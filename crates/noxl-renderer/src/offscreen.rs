use std::error::Error;
use std::fmt;
use std::sync::mpsc;

use crate::GpuContext;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Pixel format used by every `OffscreenTarget`.
pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = COLOR_FORMAT;
const TRIANGLE_SHADER: &str = include_str!("shaders/triangle.wgsl");

const COPY_BYTES_PER_PIXEL: u32 = 4;
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

// --------------------------------------------------
// Errors
// --------------------------------------------------

#[derive(Debug)]
pub enum OffscreenError {
    InvalidDimensions,
    DimensionsTooLarge,
}

impl fmt::Display for OffscreenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => {
                write!(f, "Texture dimensions must be greater than zero")
            }
            Self::DimensionsTooLarge => {
                write!(f, "Texture dimensions exceed GPU limits")
            }
        }
    }
}

impl Error for OffscreenError {}

#[derive(Debug)]
pub enum ReadbackError {
    BufferMap(String),
    Poll(wgpu::PollError),
    CallbackChannelClosed,
}

impl fmt::Display for ReadbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferMap(message) => {
                write!(f, "Buffer mapping failed: {message}")
            }
            Self::Poll(error) => {
                write!(f, "GPU polling failed: {error}")
            }
            Self::CallbackChannelClosed => {
                write!(f, "GPU readback callback channel closed")
            }
        }
    }
}

impl Error for ReadbackError {}

// --------------------------------------------------
// Pixel Buffer
// --------------------------------------------------

pub struct PixelBuffer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl PixelBuffer {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Raw RGBA bytes, row by row, without padding.
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = ((y * self.width + x) * COPY_BYTES_PER_PIXEL) as usize;

        Some([
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
            self.pixels[index + 3],
        ])
    }
}

// --------------------------------------------------
// Offscreen Target
// --------------------------------------------------

pub struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl OffscreenTarget {
    pub fn new(gpu: &GpuContext, width: u32, height: u32) -> Result<Self, OffscreenError> {
        if width == 0 || height == 0 {
            return Err(OffscreenError::InvalidDimensions);
        }

        let limit = gpu.device().limits().max_texture_dimension_2d;

        if width > limit || height > limit {
            return Err(OffscreenError::DimensionsTooLarge);
        }

        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("NOXL Offscreen Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Self {
            texture,
            view,
            width,
            height,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        COLOR_FORMAT
    }

    /// Clears the target, lets `draw` record commands into the render pass,
    /// then submits the work.
    pub fn render_with<F>(&self, gpu: &GpuContext, clear: wgpu::Color, draw: F)
    where
        F: FnOnce(&mut wgpu::RenderPass<'_>),
    {
        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("NOXL Offscreen Encoder"),
            });

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })];

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("NOXL Offscreen Pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            draw(&mut pass);
        }

        gpu.queue().submit([encoder.finish()]);
    }

    // --------------------------------------------------
    // GPU Readback
    // --------------------------------------------------

    pub fn read_pixels(&self, gpu: &GpuContext) -> Result<PixelBuffer, ReadbackError> {
        let unpadded_bytes_per_row = self.width * COPY_BYTES_PER_PIXEL;

        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT)
            * COPY_BYTES_PER_ROW_ALIGNMENT;

        let buffer_size = u64::from(padded_bytes_per_row) * u64::from(self.height);

        let readback_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("NOXL Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("NOXL Readback Encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        gpu.queue().submit([encoder.finish()]);

        let slice = readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();

        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        gpu.device()
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(ReadbackError::Poll)?;

        receiver
            .recv()
            .map_err(|_| ReadbackError::CallbackChannelClosed)?
            .map_err(|error| ReadbackError::BufferMap(format!("{error:?}")))?;

        let mapped_range = slice
            .get_mapped_range()
            .map_err(|error| ReadbackError::BufferMap(format!("{error:?}")))?;

        let mut pixels = vec![0; (self.width * self.height * COPY_BYTES_PER_PIXEL) as usize];

        for y in 0..self.height as usize {
            let source_start = y * padded_bytes_per_row as usize;

            let source_end = source_start + unpadded_bytes_per_row as usize;

            let destination_start = y * unpadded_bytes_per_row as usize;

            let destination_end = destination_start + unpadded_bytes_per_row as usize;

            pixels[destination_start..destination_end]
                .copy_from_slice(&mapped_range[source_start..source_end]);
        }

        drop(mapped_range);
        readback_buffer.unmap();

        Ok(PixelBuffer {
            width: self.width,
            height: self.height,
            pixels,
        })
    }
}

// --------------------------------------------------
// Offscreen Renderer
// --------------------------------------------------

pub struct OffscreenRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl OffscreenRenderer {
    pub fn new(gpu: &GpuContext) -> Self {
        let shader = gpu
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("NOXL Triangle Shader"),
                source: wgpu::ShaderSource::Wgsl(TRIANGLE_SHADER.into()),
            });

        let pipeline = gpu
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("NOXL Triangle Pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });

        Self { pipeline }
    }

    pub fn render(&self, gpu: &GpuContext, target: &OffscreenTarget) {
        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("NOXL Render Encoder"),
            });

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("NOXL Triangle Render Pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.draw(0..3, 0..1);
        }

        gpu.queue().submit([encoder.finish()]);
    }
}
