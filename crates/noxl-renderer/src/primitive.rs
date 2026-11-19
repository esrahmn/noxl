use std::error::Error;
use std::fmt;
use std::mem;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use noxl_core::color::Color;
use noxl_core::geometry::{Position, Rect};

use crate::GpuContext;

const PRIMITIVE_SHADER: &str = include_str!("shaders/primitive.wgsl");

const MIN_INSTANCE_CAPACITY: u32 = 64;
const VERTICES_PER_PRIMITIVE: u32 = 6;

const KIND_ROUND_RECT: f32 = 0.0;
const KIND_LINE: f32 = 1.0;

const CAP_ROUND: f32 = 0.0;
const CAP_BUTT: f32 = 1.0;

const INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x4,
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x4,
];

// --------------------------------------------------
// Color helpers
// --------------------------------------------------

/// Converts one sRGB-encoded channel (0.0..=1.0) to linear light.
fn srgb_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a NOXL color (sRGB bytes, straight alpha) to linear RGBA.
///
/// Alpha is already linear and is passed through unchanged.
fn linear_rgba(color: Color) -> [f32; 4] {
    let [r, g, b, a] = color.to_rgba_f32();

    [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), a]
}

/// Clamps a size-like value to a finite, non-negative number.
fn sanitize_length(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

// --------------------------------------------------
// Line caps
// --------------------------------------------------

/// How the ends of a line are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Half circle at each end. The line extends past its endpoints by half its thickness.
    #[default]
    Round,
    /// Flat ends exactly at the endpoints.
    Butt,
}

// --------------------------------------------------
// Instance data
// --------------------------------------------------

/// GPU data for one primitive shape.
///
/// All primitives share this layout and are drawn by the same pipeline.
/// Positions and sizes are in pixels with a top-left origin.
///
/// Inputs that are not finite produce an instance that draws nothing.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct PrimitiveInstance {
    /// x, y, width, height of the area to rasterize.
    pub(crate) bounds: [f32; 4],
    /// Rect and circle: corner radii (top-left, top-right, bottom-right, bottom-left).
    /// Line: start x, start y, end x, end y.
    pub(crate) shape: [f32; 4],
    /// Linear RGBA fill color, straight alpha.
    pub(crate) color: [f32; 4],
    /// Linear RGBA border color, straight alpha.
    pub(crate) border_color: [f32; 4],
    /// kind, half thickness (line), border width (rect), cap style (line)
    pub(crate) params: [f32; 4],
}

impl PrimitiveInstance {
    /// An instance that draws nothing.
    fn empty() -> Self {
        Self::zeroed()
    }

    /// Creates a filled rectangle with square corners and no border.
    pub fn rect(rect: Rect, color: Color) -> Self {
        let values = [
            rect.position.x,
            rect.position.y,
            rect.size.width,
            rect.size.height,
        ];

        if !values.iter().all(|value| value.is_finite()) || !rect.size.is_valid() {
            return Self::empty();
        }

        Self {
            bounds: values,
            shape: [0.0; 4],
            color: linear_rgba(color),
            border_color: [0.0; 4],
            params: [KIND_ROUND_RECT, 0.0, 0.0, CAP_ROUND],
        }
    }

    /// Creates a filled circle.
    ///
    /// A circle is a rounded rectangle whose corner radii equal its half size.
    pub fn circle(center: Position, radius: f32, color: Color) -> Self {
        if !center.is_finite() || !radius.is_finite() || radius < 0.0 {
            return Self::empty();
        }

        let mut instance = Self::rect(
            Rect::from_xywh(
                center.x - radius,
                center.y - radius,
                radius * 2.0,
                radius * 2.0,
            ),
            color,
        );

        instance.shape = [radius; 4];

        instance
    }

    /// Creates a line segment with round caps.
    ///
    /// `thickness` is the full width of the line in pixels. Use `with_cap` to
    /// change the end style.
    pub fn line(from: Position, to: Position, thickness: f32, color: Color) -> Self {
        if !from.is_finite() || !to.is_finite() || !thickness.is_finite() || thickness < 0.0 {
            return Self::empty();
        }

        let half = thickness * 0.5;

        Self {
            bounds: [
                from.x.min(to.x) - half,
                from.y.min(to.y) - half,
                (to.x - from.x).abs() + thickness,
                (to.y - from.y).abs() + thickness,
            ],
            shape: [from.x, from.y, to.x, to.y],
            color: linear_rgba(color),
            border_color: [0.0; 4],
            params: [KIND_LINE, half, 0.0, CAP_ROUND],
        }
    }

    fn is_line(&self) -> bool {
        self.params[0] == KIND_LINE
    }

    /// Largest radius a corner can have: half of the shorter side.
    fn max_corner_radius(&self) -> f32 {
        self.bounds[2].min(self.bounds[3]) * 0.5
    }

    /// Sets the same corner radius, in pixels, on all four corners.
    ///
    /// Negative and non-finite values are treated as zero, and the radius is
    /// limited to half of the shorter side. Lines ignore this.
    pub fn with_radius(self, radius: f32) -> Self {
        self.with_corner_radii(radius, radius, radius, radius)
    }

    /// Sets one radius per corner, in pixels, clockwise from the top-left.
    ///
    /// Negative and non-finite values are treated as zero, and each radius is
    /// limited to half of the shorter side so neighbouring corners never
    /// overlap. Lines ignore this.
    pub fn with_corner_radii(
        mut self,
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    ) -> Self {
        if self.is_line() {
            return self;
        }

        let max = self.max_corner_radius();

        self.shape =
            [top_left, top_right, bottom_right, bottom_left].map(|r| sanitize_length(r).min(max));

        self
    }

    /// Sets how the ends of a line are drawn. Other shapes ignore this.
    pub fn with_cap(mut self, cap: LineCap) -> Self {
        if self.is_line() {
            self.params[3] = match cap {
                LineCap::Round => CAP_ROUND,
                LineCap::Butt => CAP_BUTT,
            };
        }

        self
    }

    /// Adds a border drawn inside the shape bounds.
    ///
    /// Negative and non-finite widths are treated as zero. Lines ignore this.
    pub fn with_border(mut self, width: f32, color: Color) -> Self {
        if !self.is_line() {
            self.params[2] = sanitize_length(width);
            self.border_color = linear_rgba(color);
        }

        self
    }

    /// Multiplies fill and border alpha by `opacity` (clamped to 0.0..=1.0).
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.color[3] *= opacity;
        self.border_color[3] *= opacity;

        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    pad: [f32; 2],
}

// --------------------------------------------------
// Errors
// --------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveRendererError {
    /// Colors are converted to linear light, so the target must be an sRGB
    /// format that encodes on write.
    UnsupportedFormat(wgpu::TextureFormat),
}

impl fmt::Display for PrimitiveRendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat(format) => {
                write!(f, "render target format {format:?} is not an sRGB format")
            }
        }
    }
}

impl Error for PrimitiveRendererError {}

// --------------------------------------------------
// Renderer
// --------------------------------------------------

/// Draws many primitives (rounded rectangles, circles, lines) in a single
/// instanced draw call, in slice order.
pub struct PrimitiveRenderer {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    instance_count: u32,
}

impl PrimitiveRenderer {
    /// Creates a renderer for targets of the given sRGB `format`.
    pub fn new(
        gpu: &GpuContext,
        format: wgpu::TextureFormat,
    ) -> Result<Self, PrimitiveRendererError> {
        if !format.is_srgb() {
            return Err(PrimitiveRendererError::UnsupportedFormat(format));
        }

        let device = gpu.device();

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("NOXL Primitive Shader"),
            source: wgpu::ShaderSource::Wgsl(PRIMITIVE_SHADER.into()),
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<PrimitiveInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &INSTANCE_ATTRIBUTES,
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NOXL Primitive Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(instance_layout)],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("NOXL Primitive Globals"),
            size: mem::size_of::<Globals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = pipeline.get_bind_group_layout(0);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("NOXL Primitive Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let instance_buffer = create_instance_buffer(device, MIN_INSTANCE_CAPACITY);

        Ok(Self {
            pipeline,
            globals_buffer,
            bind_group,
            instance_buffer,
            instance_capacity: MIN_INSTANCE_CAPACITY,
            instance_count: 0,
        })
    }

    /// Uploads the viewport size and instances for the next `draw`.
    ///
    /// Instances are drawn in slice order, so later entries paint over earlier ones.
    pub fn prepare(
        &mut self,
        gpu: &GpuContext,
        width: u32,
        height: u32,
        instances: &[PrimitiveInstance],
    ) {
        let globals = Globals {
            viewport: [width.max(1) as f32, height.max(1) as f32],
            pad: [0.0; 2],
        };

        gpu.queue()
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        let count = u32::try_from(instances.len()).unwrap_or(u32::MAX);
        let instances = &instances[..count as usize];

        if count > self.instance_capacity {
            let capacity = count
                .checked_next_power_of_two()
                .unwrap_or(count)
                .max(MIN_INSTANCE_CAPACITY);

            self.instance_buffer = create_instance_buffer(gpu.device(), capacity);
            self.instance_capacity = capacity;
        }

        if count > 0 {
            gpu.queue()
                .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
        }

        self.instance_count = count;
    }

    /// Records one draw call for all instances uploaded by `prepare`.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw_range(pass, 0..self.instance_count);
    }

    /// Records one draw call for `range` of the instances uploaded by `prepare`.
    ///
    /// The range is clamped to the uploaded instances.
    pub fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, range: Range<u32>) {
        let range = range.start..range.end.min(self.instance_count);

        if range.is_empty() {
            return;
        }

        let byte_len = u64::from(self.instance_count)
            * mem::size_of::<PrimitiveInstance>() as wgpu::BufferAddress;

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..byte_len));
        pass.draw(0..VERTICES_PER_PRIMITIVE, range);
    }

    /// Number of instances that will be drawn.
    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }
}

fn create_instance_buffer(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("NOXL Primitive Instances"),
        size: u64::from(capacity) * mem::size_of::<PrimitiveInstance>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_rect() -> Rect {
        Rect::from_xywh(0.0, 0.0, 10.0, 10.0)
    }

    fn line() -> PrimitiveInstance {
        PrimitiveInstance::line(
            Position::new(0.0, 0.0),
            Position::new(10.0, 0.0),
            2.0,
            Color::WHITE,
        )
    }

    #[test]
    fn instance_is_tightly_packed() {
        assert_eq!(mem::size_of::<PrimitiveInstance>(), 80);
    }

    #[test]
    fn attributes_cover_the_whole_instance() {
        let last = INSTANCE_ATTRIBUTES.last().unwrap();

        assert_eq!(
            last.offset + last.format.size(),
            mem::size_of::<PrimitiveInstance>() as u64
        );
    }

    #[test]
    fn srgb_endpoints_are_preserved() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_midpoint_is_darker_in_linear_light() {
        assert!((srgb_to_linear(0.5) - 0.214).abs() < 0.001);
    }

    #[test]
    fn alpha_is_not_gamma_converted() {
        let color = linear_rgba(Color::rgba(255, 255, 255, 128));

        assert!((color[3] - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn rect_stores_bounds_and_square_corners() {
        let instance = PrimitiveInstance::rect(Rect::from_xywh(1.0, 2.0, 30.0, 40.0), Color::RED);

        assert_eq!(instance.bounds, [1.0, 2.0, 30.0, 40.0]);
        assert_eq!(instance.shape, [0.0; 4]);
        assert_eq!(instance.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(instance.border_color, [0.0; 4]);
        assert_eq!(instance.params, [KIND_ROUND_RECT, 0.0, 0.0, CAP_ROUND]);
    }

    #[test]
    fn circle_is_a_rounded_rect_with_radii_equal_to_half_size() {
        let instance = PrimitiveInstance::circle(Position::new(50.0, 40.0), 10.0, Color::BLUE);

        assert_eq!(instance.bounds, [40.0, 30.0, 20.0, 20.0]);
        assert_eq!(instance.shape, [10.0; 4]);
        assert_eq!(instance.params[0], KIND_ROUND_RECT);
    }

    #[test]
    fn line_bounds_include_thickness() {
        let instance = PrimitiveInstance::line(
            Position::new(10.0, 20.0),
            Position::new(50.0, 10.0),
            4.0,
            Color::WHITE,
        );

        assert_eq!(instance.bounds, [8.0, 8.0, 44.0, 14.0]);
        assert_eq!(instance.shape, [10.0, 20.0, 50.0, 10.0]);
        assert_eq!(instance.params, [KIND_LINE, 2.0, 0.0, CAP_ROUND]);
    }

    #[test]
    fn line_defaults_to_round_caps() {
        assert_eq!(line().params[3], CAP_ROUND);
        assert_eq!(LineCap::default(), LineCap::Round);
    }

    #[test]
    fn with_cap_switches_line_caps() {
        assert_eq!(line().with_cap(LineCap::Butt).params[3], CAP_BUTT);
        assert_eq!(
            line()
                .with_cap(LineCap::Butt)
                .with_cap(LineCap::Round)
                .params[3],
            CAP_ROUND
        );
    }

    #[test]
    fn with_cap_does_nothing_for_other_shapes() {
        let rect = PrimitiveInstance::rect(unit_rect(), Color::WHITE);

        assert_eq!(rect.with_cap(LineCap::Butt), rect);
    }

    #[test]
    fn line_ignores_corner_radii_and_border() {
        let line = line();

        let changed = line
            .with_radius(5.0)
            .with_corner_radii(1.0, 2.0, 3.0, 4.0)
            .with_border(3.0, Color::RED);

        assert_eq!(changed, line);
    }

    #[test]
    fn invalid_inputs_produce_an_empty_instance() {
        let empty = PrimitiveInstance::zeroed();

        assert_eq!(
            PrimitiveInstance::rect(Rect::from_xywh(f32::NAN, 0.0, 10.0, 10.0), Color::RED),
            empty
        );
        assert_eq!(
            PrimitiveInstance::rect(Rect::from_xywh(0.0, 0.0, -10.0, 10.0), Color::RED),
            empty
        );
        assert_eq!(
            PrimitiveInstance::circle(Position::ZERO, -1.0, Color::RED),
            empty
        );
        assert_eq!(
            PrimitiveInstance::circle(Position::new(f32::INFINITY, 0.0), 5.0, Color::RED),
            empty
        );
        assert_eq!(
            PrimitiveInstance::line(
                Position::ZERO,
                Position::new(1.0, f32::NAN),
                2.0,
                Color::RED
            ),
            empty
        );
        assert_eq!(
            PrimitiveInstance::line(Position::ZERO, Position::new(1.0, 1.0), -2.0, Color::RED),
            empty
        );
    }

    #[test]
    fn radius_sets_all_four_corners() {
        let instance = PrimitiveInstance::rect(unit_rect(), Color::WHITE).with_radius(3.0);

        assert_eq!(instance.shape, [3.0; 4]);
    }

    #[test]
    fn corner_radii_are_stored_clockwise_from_top_left() {
        let instance = PrimitiveInstance::rect(unit_rect(), Color::WHITE)
            .with_corner_radii(1.0, 2.0, 3.0, 4.0);

        assert_eq!(instance.shape, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn corner_radii_are_limited_to_half_the_shorter_side() {
        let instance =
            PrimitiveInstance::rect(Rect::from_xywh(0.0, 0.0, 100.0, 20.0), Color::WHITE)
                .with_corner_radii(50.0, 10.0, 4.0, 999.0);

        assert_eq!(instance.shape, [10.0, 10.0, 4.0, 10.0]);
    }

    #[test]
    fn corner_radii_reject_invalid_values() {
        let instance = PrimitiveInstance::rect(unit_rect(), Color::WHITE).with_corner_radii(
            -3.0,
            f32::NAN,
            f32::INFINITY,
            2.0,
        );

        assert_eq!(instance.shape, [0.0, 0.0, 0.0, 2.0]);
    }

    #[test]
    fn circle_radius_cannot_exceed_its_size() {
        let instance = PrimitiveInstance::circle(Position::new(10.0, 10.0), 5.0, Color::WHITE)
            .with_radius(50.0);

        assert_eq!(instance.shape, [5.0; 4]);
    }

    #[test]
    fn border_sets_width_and_color() {
        let instance =
            PrimitiveInstance::rect(unit_rect(), Color::WHITE).with_border(2.0, Color::BLUE);

        assert_eq!(instance.params[2], 2.0);
        assert_eq!(instance.border_color, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn opacity_scales_fill_and_border_alpha() {
        let instance = PrimitiveInstance::rect(unit_rect(), Color::WHITE)
            .with_border(1.0, Color::BLACK)
            .with_opacity(0.5);

        assert!((instance.color[3] - 0.5).abs() < 1e-6);
        assert!((instance.border_color[3] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn opacity_is_clamped() {
        let base = PrimitiveInstance::rect(unit_rect(), Color::WHITE);

        assert_eq!(base.with_opacity(3.0).color[3], 1.0);
        assert_eq!(base.with_opacity(-1.0).color[3], 0.0);
        assert_eq!(base.with_opacity(f32::NAN).color[3], 0.0);
    }
}
