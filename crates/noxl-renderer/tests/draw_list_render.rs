//! GPU integration tests for `DrawList` and `Renderer`.
//!
//! Each test skips itself when no GPU adapter is available (for example in CI).

use noxl_core::color::Color;
use noxl_core::geometry::{Position, Rect};
use noxl_renderer::{
    DrawList, GpuContext, OffscreenTarget, PixelBuffer, PrimitiveInstance, PrimitiveRenderer,
    Renderer,
};

const SIZE: u32 = 64;
const BLACK: [u8; 4] = [0, 0, 0, 255];

struct Harness {
    gpu: GpuContext,
    target: OffscreenTarget,
    renderer: Renderer,
}

impl Harness {
    fn new() -> Option<Self> {
        let gpu = match GpuContext::new_blocking() {
            Ok(gpu) => gpu,
            Err(error) => {
                eprintln!("skipping GPU test: {error}");
                return None;
            }
        };

        let target = OffscreenTarget::new(&gpu, SIZE, SIZE).expect("target should be created");
        let renderer = Renderer::new(&gpu, target.format()).expect("sRGB format is supported");

        Some(Self {
            gpu,
            target,
            renderer,
        })
    }

    fn render(&mut self, list: &DrawList) -> PixelBuffer {
        self.renderer.prepare(&self.gpu, SIZE, SIZE, list);

        self.target
            .render_with(&self.gpu, wgpu::Color::BLACK, |pass| {
                self.renderer.draw(pass)
            });

        self.target
            .read_pixels(&self.gpu)
            .expect("readback should work")
    }
}

fn px(pixels: &PixelBuffer, x: u32, y: u32) -> [u8; 4] {
    pixels.pixel(x, y).expect("pixel should exist")
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_xywh(x, y, width, height)
}

fn sample_primitives() -> Vec<PrimitiveInstance> {
    vec![
        PrimitiveInstance::rect(rect(4.0, 4.0, 40.0, 40.0), Color::RED)
            .with_radius(8.0)
            .with_border(2.0, Color::BLUE),
        PrimitiveInstance::circle(Position::new(40.0, 40.0), 14.0, Color::rgba(0, 255, 0, 160)),
        PrimitiveInstance::line(
            Position::new(2.0, 60.0),
            Position::new(62.0, 20.0),
            3.0,
            Color::WHITE,
        ),
    ]
}

#[test]
fn draw_list_matches_direct_primitive_rendering() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let primitives = sample_primitives();

    // Direct path: raw instances through PrimitiveRenderer.
    let mut direct = PrimitiveRenderer::new(&harness.gpu, harness.target.format()).unwrap();

    direct.prepare(&harness.gpu, SIZE, SIZE, &primitives);
    harness
        .target
        .render_with(&harness.gpu, wgpu::Color::BLACK, |pass| direct.draw(pass));

    let expected = harness.target.read_pixels(&harness.gpu).unwrap();

    // DrawList path.
    let list: DrawList = primitives.into_iter().collect();
    let actual = harness.render(&list);

    assert_eq!(actual.as_bytes(), expected.as_bytes());
}

#[test]
fn consecutive_primitives_use_a_single_batch() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let list: DrawList = (0..200)
        .map(|index| {
            let x = (index % 8) as f32 * 8.0;
            let y = (index / 8 % 8) as f32 * 8.0;

            PrimitiveInstance::rect(rect(x, y, 8.0, 8.0), Color::WHITE)
        })
        .collect();

    let pixels = harness.render(&list);

    assert_eq!(harness.renderer.batch_count(), 1);
    assert_eq!(harness.renderer.primitive_count(), 200);
    assert_eq!(px(&pixels, 4, 4), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 60, 60), [255, 255, 255, 255]);
}

#[test]
fn empty_list_draws_nothing() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let pixels = harness.render(&DrawList::new());

    assert_eq!(harness.renderer.batch_count(), 0);

    for y in 0..SIZE {
        for x in 0..SIZE {
            assert_eq!(px(&pixels, x, y), BLACK, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn renderer_can_be_reused_with_a_smaller_list() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.push(PrimitiveInstance::rect(
        rect(0.0, 0.0, 30.0, 30.0),
        Color::RED,
    ));
    list.push(PrimitiveInstance::rect(
        rect(32.0, 0.0, 30.0, 30.0),
        Color::GREEN,
    ));
    list.push(PrimitiveInstance::rect(
        rect(0.0, 32.0, 30.0, 30.0),
        Color::BLUE,
    ));

    let first = harness.render(&list);

    assert_eq!(px(&first, 10, 10), [255, 0, 0, 255]);
    assert_eq!(px(&first, 40, 10), [0, 255, 0, 255]);
    assert_eq!(px(&first, 10, 40), [0, 0, 255, 255]);

    // Next frame draws only one shape. Leftovers from the first frame must not appear.
    list.clear();
    list.push(PrimitiveInstance::rect(
        rect(32.0, 32.0, 30.0, 30.0),
        Color::WHITE,
    ));

    let second = harness.render(&list);

    assert_eq!(px(&second, 40, 40), [255, 255, 255, 255]);
    assert_eq!(px(&second, 10, 10), BLACK);
    assert_eq!(px(&second, 40, 10), BLACK);
    assert_eq!(px(&second, 10, 40), BLACK);
}

// --------------------------------------------------
// Clipping
// --------------------------------------------------

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

fn full_rect(color: Color) -> PrimitiveInstance {
    PrimitiveInstance::rect(rect(0.0, 0.0, 64.0, 64.0), color)
}

#[test]
fn clip_restricts_drawing_to_its_rectangle() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.push_clip(rect(16.0, 16.0, 32.0, 32.0));
    list.push(full_rect(Color::RED));

    let pixels = harness.render(&list);

    // Inside the clip, edges included.
    assert_eq!(px(&pixels, 16, 16), RED);
    assert_eq!(px(&pixels, 47, 47), RED);
    assert_eq!(px(&pixels, 30, 30), RED);

    // One pixel outside on every side.
    assert_eq!(px(&pixels, 15, 30), BLACK);
    assert_eq!(px(&pixels, 48, 30), BLACK);
    assert_eq!(px(&pixels, 30, 15), BLACK);
    assert_eq!(px(&pixels, 30, 48), BLACK);
    assert_eq!(px(&pixels, 2, 2), BLACK);
}

#[test]
fn fractional_clip_edges_snap_outward_to_whole_pixels() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.push_clip(rect(16.3, 16.3, 10.0, 10.0));
    list.push(full_rect(Color::RED));

    let pixels = harness.render(&list);

    // The clip spans 16.3..26.3, so pixels 16 through 26 are drawn whole.
    assert_eq!(px(&pixels, 16, 20), RED);
    assert_eq!(px(&pixels, 26, 20), RED);
    assert_eq!(px(&pixels, 15, 20), BLACK);
    assert_eq!(px(&pixels, 27, 20), BLACK);
}

#[test]
fn drawing_after_pop_clip_is_unclipped_again() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.with_clip(rect(0.0, 0.0, 32.0, 32.0), |list| {
        list.push(full_rect(Color::RED));
    });
    list.push(PrimitiveInstance::circle(
        Position::new(48.0, 48.0),
        8.0,
        Color::WHITE,
    ));

    let pixels = harness.render(&list);

    assert_eq!(px(&pixels, 10, 10), RED);
    // The red rectangle stayed inside its clip.
    assert_eq!(px(&pixels, 40, 10), BLACK);
    // The circle was drawn after the clip ended.
    assert_eq!(px(&pixels, 48, 48), [255, 255, 255, 255]);
}

#[test]
fn nested_clips_draw_only_where_they_overlap() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.push_clip(rect(0.0, 0.0, 48.0, 48.0));
    list.push_clip(rect(16.0, 16.0, 48.0, 48.0));
    list.push(full_rect(Color::RED));

    let pixels = harness.render(&list);

    // The overlap is 16..48 on both axes.
    assert_eq!(px(&pixels, 20, 20), RED);
    assert_eq!(px(&pixels, 47, 47), RED);
    assert_eq!(px(&pixels, 10, 10), BLACK);
    assert_eq!(px(&pixels, 50, 50), BLACK);
    assert_eq!(px(&pixels, 10, 30), BLACK);
    assert_eq!(px(&pixels, 30, 10), BLACK);
}

#[test]
fn clips_apply_to_circles_and_lines() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.push_clip(rect(32.0, 0.0, 32.0, 64.0));
    list.push(PrimitiveInstance::circle(
        Position::new(32.0, 24.0),
        20.0,
        Color::GREEN,
    ));
    list.pop_clip();

    list.push_clip(rect(0.0, 0.0, 32.0, 64.0));
    list.push(PrimitiveInstance::line(
        Position::new(0.0, 56.0),
        Position::new(64.0, 56.0),
        4.0,
        Color::WHITE,
    ));

    let pixels = harness.render(&list);

    // Circle: only its right half remains.
    assert_eq!(px(&pixels, 40, 24), GREEN);
    assert_eq!(px(&pixels, 24, 24), BLACK);

    // Line: only its left half remains.
    assert_eq!(px(&pixels, 10, 56), [255, 255, 255, 255]);
    assert_eq!(px(&pixels, 50, 56), BLACK);
}

#[test]
fn a_clip_hanging_over_the_target_edge_is_limited_to_the_target() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.push_clip(rect(-10.0, -10.0, 30.0, 30.0));
    list.push(full_rect(Color::RED));

    let pixels = harness.render(&list);

    assert_eq!(px(&pixels, 0, 0), RED);
    assert_eq!(px(&pixels, 19, 19), RED);
    assert_eq!(px(&pixels, 20, 5), BLACK);
    assert_eq!(px(&pixels, 5, 20), BLACK);
}

#[test]
fn a_clip_outside_the_target_draws_nothing_and_drawing_recovers_after() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.with_clip(rect(100.0, 100.0, 50.0, 50.0), |list| {
        list.push(PrimitiveInstance::rect(
            rect(100.0, 100.0, 50.0, 50.0),
            Color::RED,
        ));
    });
    list.push(PrimitiveInstance::rect(
        rect(10.0, 10.0, 10.0, 10.0),
        Color::GREEN,
    ));

    let pixels = harness.render(&list);

    assert_eq!(px(&pixels, 12, 12), GREEN);
    assert_eq!(px(&pixels, 63, 63), BLACK);
}

#[test]
fn clip_changes_are_not_counted_as_draw_calls() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut list = DrawList::new();

    list.with_clip(rect(0.0, 0.0, 32.0, 32.0), |list| {
        list.push(full_rect(Color::RED));
    });
    list.with_clip(rect(32.0, 32.0, 32.0, 32.0), |list| {
        list.push(full_rect(Color::GREEN));
    });

    let pixels = harness.render(&list);

    assert_eq!(list.commands().len(), 4);
    assert_eq!(harness.renderer.batch_count(), 2);
    assert_eq!(px(&pixels, 10, 10), RED);
    assert_eq!(px(&pixels, 50, 50), GREEN);
    assert_eq!(px(&pixels, 50, 10), BLACK);
}

#[test]
fn the_scissor_is_restored_after_the_list_is_drawn() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut clipped = DrawList::new();

    clipped.with_clip(rect(0.0, 0.0, 16.0, 16.0), |list| {
        list.push(full_rect(Color::RED));
    });

    harness.renderer.prepare(&harness.gpu, SIZE, SIZE, &clipped);

    // Something else drawn into the same pass afterwards must not inherit the clip.
    let mut extra = PrimitiveRenderer::new(&harness.gpu, harness.target.format()).unwrap();

    extra.prepare(
        &harness.gpu,
        SIZE,
        SIZE,
        &[PrimitiveInstance::rect(
            rect(0.0, 48.0, 64.0, 16.0),
            Color::BLUE,
        )],
    );

    harness
        .target
        .render_with(&harness.gpu, wgpu::Color::BLACK, |pass| {
            harness.renderer.draw(pass);
            extra.draw(pass);
        });

    let pixels = harness.target.read_pixels(&harness.gpu).unwrap();

    assert_eq!(px(&pixels, 5, 5), RED);
    assert_eq!(px(&pixels, 30, 30), BLACK);
    assert_eq!(px(&pixels, 30, 55), [0, 0, 255, 255]);
}
