//! GPU integration tests for the primitive renderer.
//!
//! Each test skips itself when no GPU adapter is available (for example in CI).

use noxl_core::color::Color;
use noxl_core::geometry::{Position, Rect};
use noxl_renderer::{
    GpuContext, LineCap, OffscreenTarget, PixelBuffer, PrimitiveInstance, PrimitiveRenderer,
};

const SIZE: u32 = 64;
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn render(instances: &[PrimitiveInstance]) -> Option<PixelBuffer> {
    let gpu = match GpuContext::new_blocking() {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("skipping GPU test: {error}");
            return None;
        }
    };

    let target = OffscreenTarget::new(&gpu, SIZE, SIZE).expect("target should be created");
    let mut renderer =
        PrimitiveRenderer::new(&gpu, target.format()).expect("sRGB format is supported");

    renderer.prepare(&gpu, SIZE, SIZE, instances);
    target.render_with(&gpu, wgpu::Color::BLACK, |pass| renderer.draw(pass));

    Some(target.read_pixels(&gpu).expect("readback should work"))
}

fn px(pixels: &PixelBuffer, x: u32, y: u32) -> [u8; 4] {
    pixels.pixel(x, y).expect("pixel should exist")
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_xywh(x, y, width, height)
}

// --------------------------------------------------
// Rounded rectangle
// --------------------------------------------------

#[test]
fn solid_rect_fills_exactly_its_bounds() {
    let instance = PrimitiveInstance::rect(rect(8.0, 8.0, 32.0, 32.0), Color::RED);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 20, 20), [255, 0, 0, 255]);
    assert_eq!(px(&pixels, 8, 8), [255, 0, 0, 255]);
    assert_eq!(px(&pixels, 39, 39), [255, 0, 0, 255]);

    assert_eq!(px(&pixels, 7, 20), BLACK);
    assert_eq!(px(&pixels, 40, 20), BLACK);
    assert_eq!(px(&pixels, 2, 2), BLACK);
}

#[test]
fn corner_radius_cuts_the_corners() {
    let instance =
        PrimitiveInstance::rect(rect(0.0, 0.0, 32.0, 32.0), Color::GREEN).with_radius(16.0);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 0, 0), BLACK);
    assert_eq!(px(&pixels, 16, 16), [0, 255, 0, 255]);
    assert_eq!(px(&pixels, 16, 1), [0, 255, 0, 255]);
}

#[test]
fn border_is_drawn_inside_the_bounds() {
    let instance = PrimitiveInstance::rect(rect(4.0, 4.0, 40.0, 40.0), Color::RED)
        .with_border(4.0, Color::BLUE);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    // Inside the 4px border band.
    assert_eq!(px(&pixels, 6, 24), [0, 0, 255, 255]);
    // Inside the fill.
    assert_eq!(px(&pixels, 24, 24), [255, 0, 0, 255]);
    // Outside the shape.
    assert_eq!(px(&pixels, 2, 24), BLACK);
}

// --------------------------------------------------
// Circle
// --------------------------------------------------

#[test]
fn circle_fills_its_disc_only() {
    let instance = PrimitiveInstance::circle(Position::new(32.0, 32.0), 16.0, Color::GREEN);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 32, 32), [0, 255, 0, 255]);
    assert_eq!(px(&pixels, 40, 40), [0, 255, 0, 255]);

    // Corner of the bounding square is outside the disc.
    assert_eq!(px(&pixels, 17, 17), BLACK);
    // Just beyond the radius on the axis.
    assert_eq!(px(&pixels, 49, 32), BLACK);
    assert_eq!(px(&pixels, 32, 49), BLACK);
}

#[test]
fn circle_edge_is_anti_aliased() {
    let instance = PrimitiveInstance::circle(Position::new(32.0, 32.0), 16.0, Color::WHITE);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    // At 45 degrees the edge cuts through pixels, so some must be partially covered.
    let partial = (17..32)
        .map(|offset| px(&pixels, offset, offset))
        .filter(|[r, ..]| *r > 0 && *r < 255)
        .count();

    assert!(partial > 0, "expected anti-aliased edge pixels");
}

#[test]
fn circle_with_border_has_a_ring() {
    let instance = PrimitiveInstance::circle(Position::new(32.0, 32.0), 20.0, Color::RED)
        .with_border(4.0, Color::BLUE);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 32, 32), [255, 0, 0, 255]);
    // 18 px from the center is inside the 4 px ring (radius 16..20).
    assert_eq!(px(&pixels, 50, 32), [0, 0, 255, 255]);
}

// --------------------------------------------------
// Line
// --------------------------------------------------

#[test]
fn horizontal_line_has_the_requested_thickness() {
    let instance = PrimitiveInstance::line(
        Position::new(8.0, 32.0),
        Position::new(56.0, 32.0),
        4.0,
        Color::WHITE,
    );

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    // Rows 30..=33 are fully covered, rows 29 and 34 are not.
    for y in 30..=33 {
        assert_eq!(px(&pixels, 32, y), [255, 255, 255, 255], "row {y}");
    }

    assert_eq!(px(&pixels, 32, 29), BLACK);
    assert_eq!(px(&pixels, 32, 34), BLACK);
}

#[test]
fn line_has_round_caps() {
    let instance = PrimitiveInstance::line(
        Position::new(8.0, 32.0),
        Position::new(56.0, 32.0),
        4.0,
        Color::WHITE,
    );

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    // The cap extends half the thickness (2 px) beyond the endpoint.
    assert_eq!(px(&pixels, 7, 32), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 4, 32), BLACK);
    assert_eq!(px(&pixels, 59, 32), BLACK);
}

#[test]
fn diagonal_line_covers_its_path_only() {
    let instance = PrimitiveInstance::line(
        Position::new(8.0, 8.0),
        Position::new(56.0, 56.0),
        6.0,
        Color::WHITE,
    );

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 32, 32), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 20, 20), [255, 255, 255, 255]);

    assert_eq!(px(&pixels, 32, 8), BLACK);
    assert_eq!(px(&pixels, 8, 32), BLACK);
}

#[test]
fn zero_length_line_draws_a_dot() {
    let point = Position::new(32.0, 32.0);
    let instance = PrimitiveInstance::line(point, point, 8.0, Color::WHITE);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 32, 32), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 40, 32), BLACK);
}

// --------------------------------------------------
// Mixed shapes and ordering
// --------------------------------------------------

#[test]
fn translucent_color_blends_in_linear_light() {
    let instance =
        PrimitiveInstance::rect(rect(0.0, 0.0, 32.0, 32.0), Color::rgba(255, 255, 255, 128));

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    let [r, g, b, a] = px(&pixels, 16, 16);

    // 50% white over black in linear light encodes to about 188 in sRGB.
    // Blending the encoded values directly would give about 128 instead.
    assert!((180..=196).contains(&r), "red was {r}");
    assert_eq!(r, g);
    assert_eq!(g, b);
    assert_eq!(a, 255);
}

#[test]
fn mixed_shapes_paint_in_instance_order() {
    let back = PrimitiveInstance::rect(rect(0.0, 0.0, 40.0, 40.0), Color::RED);
    let circle = PrimitiveInstance::circle(Position::new(30.0, 30.0), 12.0, Color::BLUE);
    let line = PrimitiveInstance::line(
        Position::new(0.0, 30.0),
        Position::new(64.0, 30.0),
        4.0,
        Color::GREEN,
    );

    let Some(pixels) = render(&[back, circle, line]) else {
        return;
    };

    // Red rect only.
    assert_eq!(px(&pixels, 5, 5), [255, 0, 0, 255]);
    // Circle over the rect.
    assert_eq!(px(&pixels, 30, 22), [0, 0, 255, 255]);
    // Line over both the circle and the rect.
    assert_eq!(px(&pixels, 30, 30), [0, 255, 0, 255]);
    assert_eq!(px(&pixels, 5, 30), [0, 255, 0, 255]);
    // Line extends past the rect on the black background.
    assert_eq!(px(&pixels, 55, 30), [0, 255, 0, 255]);
}

#[test]
fn invalid_primitives_draw_nothing() {
    let invalid = [
        PrimitiveInstance::rect(rect(f32::NAN, 0.0, 10.0, 10.0), Color::WHITE),
        PrimitiveInstance::circle(Position::new(32.0, 32.0), -5.0, Color::WHITE),
        PrimitiveInstance::line(
            Position::new(0.0, 0.0),
            Position::new(f32::INFINITY, 10.0),
            4.0,
            Color::WHITE,
        ),
    ];

    let Some(pixels) = render(&invalid) else {
        return;
    };

    for y in 0..SIZE {
        for x in 0..SIZE {
            assert_eq!(px(&pixels, x, y), BLACK, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn renders_more_instances_than_the_initial_capacity() {
    let instances: Vec<PrimitiveInstance> = (0..200)
        .map(|index| {
            let x = (index % 8) as f32 * 8.0;
            let y = (index / 8 % 8) as f32 * 8.0;

            PrimitiveInstance::rect(rect(x, y, 8.0, 8.0), Color::WHITE)
        })
        .collect();

    let Some(pixels) = render(&instances) else {
        return;
    };

    assert_eq!(px(&pixels, 4, 4), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 60, 60), [255, 255, 255, 255]);
}

// --------------------------------------------------
// Per-corner radii
// --------------------------------------------------

/// Corner pixels of the 32x32 rect placed at (8, 8): top-left, top-right,
/// bottom-right, bottom-left.
const CORNER_PIXELS: [(u32, u32); 4] = [(8, 8), (39, 8), (39, 39), (8, 39)];

#[test]
fn each_corner_can_be_rounded_on_its_own() {
    for rounded in 0..4 {
        let mut radii = [0.0; 4];
        radii[rounded] = 12.0;

        let instance = PrimitiveInstance::rect(rect(8.0, 8.0, 32.0, 32.0), Color::GREEN)
            .with_corner_radii(radii[0], radii[1], radii[2], radii[3]);

        let Some(pixels) = render(&[instance]) else {
            return;
        };

        for (corner, &(x, y)) in CORNER_PIXELS.iter().enumerate() {
            let expected = if corner == rounded {
                BLACK
            } else {
                [0, 255, 0, 255]
            };

            assert_eq!(
                px(&pixels, x, y),
                expected,
                "rounded corner {rounded}, checking corner {corner}"
            );
        }

        // The middle is always filled.
        assert_eq!(px(&pixels, 24, 24), [0, 255, 0, 255]);
    }
}

#[test]
fn different_radii_on_different_corners() {
    let instance = PrimitiveInstance::rect(rect(8.0, 8.0, 32.0, 32.0), Color::GREEN)
        .with_corner_radii(16.0, 0.0, 16.0, 0.0);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 8, 8), BLACK);
    assert_eq!(px(&pixels, 39, 39), BLACK);
    assert_eq!(px(&pixels, 39, 8), [0, 255, 0, 255]);
    assert_eq!(px(&pixels, 8, 39), [0, 255, 0, 255]);
}

#[test]
fn oversized_corner_radii_become_a_pill() {
    // 40x16 with huge radii is limited to a radius of 8: a pill shape.
    let instance =
        PrimitiveInstance::rect(rect(8.0, 24.0, 40.0, 16.0), Color::GREEN).with_radius(1000.0);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 8, 24), BLACK);
    assert_eq!(px(&pixels, 47, 39), BLACK);
    assert_eq!(px(&pixels, 28, 32), [0, 255, 0, 255]);
    // Middle of the flat top edge is filled.
    assert_eq!(px(&pixels, 28, 24), [0, 255, 0, 255]);
    // Middle of the rounded left end is filled.
    assert_eq!(px(&pixels, 9, 32), [0, 255, 0, 255]);
}

#[test]
fn per_corner_radii_work_with_a_border() {
    let instance = PrimitiveInstance::rect(rect(8.0, 8.0, 32.0, 32.0), Color::RED)
        .with_corner_radii(12.0, 0.0, 12.0, 0.0)
        .with_border(3.0, Color::BLUE);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    // Square corners keep the border all the way to the tip.
    assert_eq!(px(&pixels, 39, 8), [0, 0, 255, 255]);
    assert_eq!(px(&pixels, 8, 39), [0, 0, 255, 255]);
    // Rounded corners are cut away.
    assert_eq!(px(&pixels, 8, 8), BLACK);
    assert_eq!(px(&pixels, 39, 39), BLACK);
    // Fill and border bands.
    assert_eq!(px(&pixels, 24, 24), [255, 0, 0, 255]);
    assert_eq!(px(&pixels, 24, 9), [0, 0, 255, 255]);
}

// --------------------------------------------------
// Line caps
// --------------------------------------------------

#[test]
fn butt_caps_end_exactly_at_the_endpoints() {
    let instance = PrimitiveInstance::line(
        Position::new(16.0, 32.0),
        Position::new(48.0, 32.0),
        6.0,
        Color::WHITE,
    )
    .with_cap(LineCap::Butt);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    // First and last covered pixel columns.
    assert_eq!(px(&pixels, 16, 32), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 47, 32), [255, 255, 255, 255]);
    // Nothing past either endpoint.
    assert_eq!(px(&pixels, 15, 32), BLACK);
    assert_eq!(px(&pixels, 48, 32), BLACK);
    // Thickness is unchanged: rows 29..=34 are covered.
    assert_eq!(px(&pixels, 32, 29), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 32, 34), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 32, 28), BLACK);
    assert_eq!(px(&pixels, 32, 35), BLACK);
}

#[test]
fn round_caps_extend_past_the_endpoints_but_butt_caps_do_not() {
    let line = PrimitiveInstance::line(
        Position::new(16.0, 32.0),
        Position::new(48.0, 32.0),
        6.0,
        Color::WHITE,
    );

    let Some(round) = render(&[line]) else {
        return;
    };
    let Some(butt) = render(&[line.with_cap(LineCap::Butt)]) else {
        return;
    };

    assert_eq!(px(&round, 15, 32), [255, 255, 255, 255]);
    assert_eq!(px(&butt, 15, 32), BLACK);
}

#[test]
fn diagonal_butt_line_is_flat_at_its_ends() {
    let line = PrimitiveInstance::line(
        Position::new(8.0, 8.0),
        Position::new(56.0, 56.0),
        6.0,
        Color::WHITE,
    );

    let Some(round) = render(&[line]) else {
        return;
    };
    let Some(butt) = render(&[line.with_cap(LineCap::Butt)]) else {
        return;
    };

    // Just beyond the start point along the line direction.
    assert_eq!(px(&round, 6, 6), [255, 255, 255, 255]);
    assert_eq!(px(&butt, 6, 6), BLACK);
    // Both cover the middle.
    assert_eq!(px(&round, 32, 32), [255, 255, 255, 255]);
    assert_eq!(px(&butt, 32, 32), [255, 255, 255, 255]);
}

#[test]
fn zero_length_butt_line_draws_nothing() {
    let point = Position::new(32.0, 32.0);
    let instance = PrimitiveInstance::line(point, point, 8.0, Color::WHITE).with_cap(LineCap::Butt);

    let Some(pixels) = render(&[instance]) else {
        return;
    };

    assert_eq!(px(&pixels, 32, 32), BLACK);
}
