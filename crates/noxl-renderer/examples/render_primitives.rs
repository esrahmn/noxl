use std::fs;
use std::process::exit;

use noxl_core::color::Color;
use noxl_core::geometry::{Position, Rect};
use noxl_renderer::{DrawList, GpuContext, LineCap, OffscreenTarget, PrimitiveInstance, Renderer};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 240;
const OUTPUT_PATH: &str = "/tmp/noxl_primitives.ppm";

fn hex(value: &str) -> Color {
    Color::from_hex(value).expect("valid hex color")
}

fn main() {
    let gpu = match GpuContext::new_blocking() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("GPU initialization failed: {error}");
            exit(1);
        }
    };

    println!("GPU: {}", gpu.adapter_info().name);

    let target = OffscreenTarget::new(&gpu, WIDTH, HEIGHT).expect("target should be created");

    let mut renderer = match Renderer::new(&gpu, target.format()) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("Renderer creation failed: {error}");
            exit(1);
        }
    };

    let rect = |x, y, width, height| Rect::from_xywh(x, y, width, height);
    let point = |x, y| Position::new(x, y);

    // Everything below goes through the same pipeline, in this order.
    let mut list = DrawList::new();

    list.extend([
        // Bar with a border.
        PrimitiveInstance::rect(rect(16.0, 16.0, 768.0, 56.0), hex("#1E1E2E"))
            .with_radius(16.0)
            .with_border(1.0, hex("#45475A")),
        // Pills with different radii.
        PrimitiveInstance::rect(rect(28.0, 28.0, 80.0, 32.0), hex("#89B4FA")).with_radius(16.0),
        PrimitiveInstance::rect(rect(120.0, 28.0, 80.0, 32.0), hex("#A6E3A1")).with_radius(8.0),
        PrimitiveInstance::rect(rect(212.0, 28.0, 80.0, 32.0), hex("#F38BA8")).with_radius(4.0),
        // Status dots on the right side of the bar.
        PrimitiveInstance::circle(point(720.0, 44.0), 8.0, hex("#A6E3A1")),
        PrimitiveInstance::circle(point(748.0, 44.0), 8.0, hex("#F9E2AF")),
        PrimitiveInstance::circle(point(776.0, 44.0), 0.0, hex("#F38BA8")),
        // Panel with a border.
        PrimitiveInstance::rect(rect(16.0, 96.0, 360.0, 128.0), hex("#313244"))
            .with_radius(20.0)
            .with_border(2.0, hex("#CBA6F7")),
        // Lines of different thickness.
        PrimitiveInstance::line(point(40.0, 128.0), point(350.0, 128.0), 1.0, hex("#6C7086")),
        PrimitiveInstance::line(point(40.0, 152.0), point(350.0, 152.0), 3.0, hex("#89B4FA")),
        PrimitiveInstance::line(point(40.0, 176.0), point(350.0, 200.0), 6.0, hex("#F5C2E7")),
        // Ring: a circle with a border.
        PrimitiveInstance::circle(point(480.0, 160.0), 48.0, hex("#1E1E2E"))
            .with_border(6.0, hex("#94E2D5")),
        // Polyline made of connected segments.
        PrimitiveInstance::line(
            point(560.0, 200.0),
            point(620.0, 120.0),
            4.0,
            hex("#FAB387"),
        ),
        PrimitiveInstance::line(
            point(620.0, 120.0),
            point(680.0, 180.0),
            4.0,
            hex("#FAB387"),
        ),
        PrimitiveInstance::line(
            point(680.0, 180.0),
            point(760.0, 110.0),
            4.0,
            hex("#FAB387"),
        ),
        // Per-corner radii: top-only, bottom-only, and a leaf shape.
        PrimitiveInstance::rect(rect(320.0, 36.0, 80.0, 24.0), hex("#45475A"))
            .with_corner_radii(10.0, 10.0, 0.0, 0.0),
        PrimitiveInstance::rect(rect(412.0, 28.0, 80.0, 24.0), hex("#74C7EC"))
            .with_corner_radii(0.0, 0.0, 10.0, 10.0),
        PrimitiveInstance::rect(rect(504.0, 28.0, 80.0, 32.0), hex("#F9E2AF"))
            .with_corner_radii(16.0, 2.0, 16.0, 2.0),
        // Line caps: round (default) on top, butt below.
        PrimitiveInstance::line(point(606.0, 36.0), point(646.0, 36.0), 8.0, hex("#CDD6F4")),
        PrimitiveInstance::line(point(606.0, 54.0), point(646.0, 54.0), 8.0, hex("#CDD6F4"))
            .with_cap(LineCap::Butt),
        // Translucent overlay on top of everything before it.
        PrimitiveInstance::circle(point(400.0, 160.0), 40.0, hex("#FFFFFF80")),
    ]);

    renderer.prepare(&gpu, WIDTH, HEIGHT, &list);

    target.render_with(
        &gpu,
        wgpu::Color {
            r: 0.004,
            g: 0.004,
            b: 0.01,
            a: 1.0,
        },
        |pass| renderer.draw(pass),
    );

    let pixels = match target.read_pixels(&gpu) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!("GPU readback failed: {error}");
            exit(1);
        }
    };

    let mut ppm = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();

    let (rgba_pixels, _) = pixels.as_bytes().as_chunks::<4>();

    for &[r, g, b, _] in rgba_pixels {
        ppm.extend_from_slice(&[r, g, b]);
    }

    if let Err(error) = fs::write(OUTPUT_PATH, ppm) {
        eprintln!("Failed to write {OUTPUT_PATH}: {error}");
        exit(1);
    }

    println!(
        "Drew {} primitives in {} draw call(s)",
        list.primitive_count(),
        renderer.batch_count()
    );
    println!("Saved {OUTPUT_PATH}");
}
