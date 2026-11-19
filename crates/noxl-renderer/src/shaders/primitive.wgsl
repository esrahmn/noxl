// Unified primitive shader.
//
// Every shape is one instance and is drawn by the same pipeline, so draw order
// is simply instance order. Shapes are described by signed distance functions:
//
//   kind 0  rounded rectangle with one radius per corner (a circle is a
//           rounded rect whose radii equal its half size)
//   kind 1  line segment, with round or butt caps
//
// Colors arrive as linear, non-premultiplied RGBA. The render target is an
// sRGB format, so the hardware encodes the output on write.
// Output is premultiplied alpha.

struct Globals {
    viewport: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

const KIND_LINE: f32 = 1.0;
const CAP_BUTT: f32 = 1.0;

struct InstanceInput {
    // x, y, width, height of the area to rasterize, in pixels (top-left origin).
    @location(0) bounds: vec4<f32>,
    // Rect/circle: corner radii (top-left, top-right, bottom-right, bottom-left).
    // Line: a.xy, b.xy.
    @location(1) shape: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    // x = kind, y = half thickness (line), z = border width (rect),
    // w = cap style (line): 0 round, 1 butt.
    @location(4) params: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // Absolute pixel position of this fragment.
    @location(0) pixel: vec2<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) border_color: vec4<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
    // Rect/circle: center.xy, half_size.xy.
    @location(5) @interpolate(flat) center_half: vec4<f32>,
};

// The quad is expanded by this many pixels so anti-aliased edges are not clipped.
const AA_PAD: f32 = 1.0;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );

    let corner = corners[vertex_index];

    let origin = instance.bounds.xy;
    let size = instance.bounds.zw;
    let half_size = size * 0.5;

    let expanded = size + vec2<f32>(2.0 * AA_PAD);
    let pixel = origin - vec2<f32>(AA_PAD) + corner * expanded;

    let ndc = vec2<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
    );

    var result: VertexOutput;
    result.position = vec4<f32>(ndc, 0.0, 1.0);
    result.pixel = pixel;
    result.shape = instance.shape;
    result.color = instance.color;
    result.border_color = instance.border_color;
    result.params = instance.params;
    result.center_half = vec4<f32>(origin + half_size, half_size);

    return result;
}

// Signed distance to a rounded rectangle centered on the origin, with one
// radius per corner: radii = (top-left, top-right, bottom-right, bottom-left).
// Negative inside, positive outside, measured in pixels. Y points down.
fn sd_round_rect(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    var radius: f32;

    if (p.x > 0.0) {
        if (p.y > 0.0) {
            radius = radii.z;
        } else {
            radius = radii.y;
        }
    } else {
        if (p.y > 0.0) {
            radius = radii.w;
        } else {
            radius = radii.x;
        }
    }

    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// Distance from `p` to the segment a-b. Safe for zero-length segments.
// Subtract the half thickness to get a line with round caps.
fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

// Signed distance to the rectangle that covers segment a-b with flat (butt)
// ends. A zero-length segment has no area, so it draws nothing.
fn sd_butt_line(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, half_thickness: f32) -> f32 {
    let delta = b - a;
    let line_length = length(delta);

    var direction = vec2<f32>(1.0, 0.0);
    if (line_length > 1e-6) {
        direction = delta / line_length;
    }

    let q = p - (a + b) * 0.5;
    let local = vec2<f32>(dot(q, direction), dot(q, vec2<f32>(-direction.y, direction.x)));
    let d = abs(local) - vec2<f32>(line_length * 0.5, half_thickness);

    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let border_width = max(input.params.z, 0.0);

    var dist: f32;

    if (input.params.x == KIND_LINE) {
        let half_thickness = input.params.y;

        if (input.params.w == CAP_BUTT) {
            dist = sd_butt_line(input.pixel, input.shape.xy, input.shape.zw, half_thickness);
        } else {
            dist = sd_segment(input.pixel, input.shape.xy, input.shape.zw) - half_thickness;
        }
    } else {
        let half_size = input.center_half.zw;
        let max_radius = min(half_size.x, half_size.y);
        let radii = clamp(input.shape, vec4<f32>(0.0), vec4<f32>(max_radius));

        dist = sd_round_rect(input.pixel - input.center_half.xy, half_size, radii);
    }

    // Coverage of the whole shape (fill + border).
    let outer = clamp(0.5 - dist, 0.0, 1.0);

    // Coverage of the fill area inside the border.
    var inner = 1.0;
    if (border_width > 0.0) {
        inner = clamp(0.5 - (dist + border_width), 0.0, 1.0);
    }

    let fill = vec4<f32>(input.color.rgb * input.color.a, input.color.a);
    let border = vec4<f32>(
        input.border_color.rgb * input.border_color.a,
        input.border_color.a,
    );

    return (border * (1.0 - inner) + fill * inner) * outer;
}
