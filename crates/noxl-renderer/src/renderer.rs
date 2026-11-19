use std::error::Error;
use std::fmt;

use noxl_core::geometry::Rect;

use crate::GpuContext;
use crate::draw_list::{DrawCommand, DrawList};
use crate::primitive::{PrimitiveRenderer, PrimitiveRendererError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererError {
    Primitive(PrimitiveRendererError),
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(error) => write!(f, "primitive renderer: {error}"),
        }
    }
}

impl Error for RendererError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Primitive(error) => Some(error),
        }
    }
}

impl From<PrimitiveRendererError> for RendererError {
    fn from(error: PrimitiveRendererError) -> Self {
        Self::Primitive(error)
    }
}

/// Executes a `DrawList`.
///
/// Usage per frame: `prepare` uploads the list to the GPU, then `draw` records
/// its commands into a render pass in order.
pub struct Renderer {
    primitives: PrimitiveRenderer,
    commands: Vec<DrawCommand>,
    viewport: (u32, u32),
}

impl Renderer {
    /// Creates a renderer for targets of the given sRGB `format`.
    pub fn new(gpu: &GpuContext, format: wgpu::TextureFormat) -> Result<Self, RendererError> {
        Ok(Self {
            primitives: PrimitiveRenderer::new(gpu, format)?,
            commands: Vec::new(),
            viewport: (0, 0),
        })
    }

    /// Uploads `list` and remembers its commands for the next `draw`.
    ///
    /// `width` and `height` must be the size in pixels of the render target
    /// that `draw` will record into.
    pub fn prepare(&mut self, gpu: &GpuContext, width: u32, height: u32, list: &DrawList) {
        self.primitives
            .prepare(gpu, width, height, list.primitives());

        self.commands.clear();
        self.commands.extend_from_slice(list.commands());
        self.viewport = (width, height);
    }

    /// Records the prepared commands, in order, into `pass`.
    ///
    /// The scissor rectangle is restored to the whole target afterwards.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let (width, height) = self.viewport;

        if width == 0 || height == 0 {
            return;
        }

        // False while the active clip shows no pixel of the target.
        let mut visible = true;
        let mut scissor_changed = false;

        for command in &self.commands {
            match command {
                DrawCommand::SetClip(clip) => {
                    scissor_changed = true;

                    match scissor_rect(clip.as_ref(), width, height) {
                        Some([x, y, w, h]) => {
                            pass.set_scissor_rect(x, y, w, h);
                            visible = true;
                        }
                        None => visible = false,
                    }
                }
                DrawCommand::Primitives(range) => {
                    if visible {
                        self.primitives.draw_range(pass, range.clone());
                    }
                }
            }
        }

        if scissor_changed {
            pass.set_scissor_rect(0, 0, width, height);
        }
    }

    /// Number of primitives that will be drawn.
    pub fn primitive_count(&self) -> u32 {
        self.primitives.instance_count()
    }

    /// Number of draw calls the prepared list needs.
    ///
    /// Counts primitive batches only; clip changes are free state changes.
    pub fn batch_count(&self) -> usize {
        self.commands
            .iter()
            .filter(|command| matches!(command, DrawCommand::Primitives(_)))
            .count()
    }
}

/// Converts a clip rectangle to a scissor rectangle `[x, y, width, height]`
/// in whole pixels, or `None` if it covers no pixel of the target.
///
/// Edges snap outward, so a partly covered pixel is drawn whole, and the
/// result is limited to the target.
fn scissor_rect(clip: Option<&Rect>, width: u32, height: u32) -> Option<[u32; 4]> {
    let Some(clip) = clip else {
        return Some([0, 0, width, height]);
    };

    if !clip.is_valid() {
        return None;
    }

    let left = clip.left().floor().max(0.0);
    let top = clip.top().floor().max(0.0);
    let right = clip.right().ceil().min(width as f32);
    let bottom = clip.bottom().ceil().min(height as f32);

    if !(right > left && bottom > top) {
        return None;
    }

    Some([
        left as u32,
        top as u32,
        (right - left) as u32,
        (bottom - top) as u32,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect::from_xywh(x, y, width, height)
    }

    #[test]
    fn no_clip_means_the_whole_target() {
        assert_eq!(scissor_rect(None, 800, 600), Some([0, 0, 800, 600]));
    }

    #[test]
    fn whole_pixel_clips_are_used_as_is() {
        let clip = rect(10.0, 20.0, 30.0, 40.0);

        assert_eq!(scissor_rect(Some(&clip), 800, 600), Some([10, 20, 30, 40]));
    }

    #[test]
    fn fractional_edges_snap_outward() {
        let clip = rect(16.3, 16.7, 10.0, 10.0);

        // Left 16.3 -> 16, top 16.7 -> 16, right 26.3 -> 27, bottom 26.7 -> 27.
        assert_eq!(scissor_rect(Some(&clip), 64, 64), Some([16, 16, 11, 11]));
    }

    #[test]
    fn clips_are_limited_to_the_target() {
        let clip = rect(-10.0, -10.0, 30.0, 30.0);

        assert_eq!(scissor_rect(Some(&clip), 64, 64), Some([0, 0, 20, 20]));

        let clip = rect(50.0, 50.0, 100.0, 100.0);

        assert_eq!(scissor_rect(Some(&clip), 64, 64), Some([50, 50, 14, 14]));
    }

    #[test]
    fn clips_outside_the_target_show_nothing() {
        assert_eq!(
            scissor_rect(Some(&rect(100.0, 100.0, 10.0, 10.0)), 64, 64),
            None
        );
        assert_eq!(
            scissor_rect(Some(&rect(-50.0, 0.0, 40.0, 10.0)), 64, 64),
            None
        );
        assert_eq!(
            scissor_rect(Some(&rect(0.0, 64.0, 10.0, 10.0)), 64, 64),
            None
        );
    }

    #[test]
    fn clips_without_area_show_nothing() {
        assert_eq!(
            scissor_rect(Some(&rect(10.0, 10.0, 0.0, 20.0)), 64, 64),
            None
        );
        assert_eq!(
            scissor_rect(Some(&rect(10.0, 10.0, 20.0, 0.0)), 64, 64),
            None
        );
    }

    #[test]
    fn invalid_clips_show_nothing() {
        assert_eq!(
            scissor_rect(Some(&rect(f32::NAN, 0.0, 10.0, 10.0)), 64, 64),
            None
        );
        assert_eq!(
            scissor_rect(Some(&rect(0.0, 0.0, -5.0, 10.0)), 64, 64),
            None
        );
    }

    #[test]
    fn a_sliver_thinner_than_a_pixel_still_covers_one_pixel() {
        let clip = rect(10.4, 10.0, 0.2, 5.0);

        assert_eq!(scissor_rect(Some(&clip), 64, 64), Some([10, 10, 1, 5]));
    }
}
