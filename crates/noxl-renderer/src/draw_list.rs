use std::ops::Range;

use noxl_core::geometry::Rect;

use crate::primitive::PrimitiveInstance;

/// Primitives are rasterized up to this many pixels beyond their bounds
/// (anti-aliasing), so culling against a clip rectangle must allow for it.
const CULL_PADDING: f32 = 1.0;

/// One step of a frame, in the order it must be drawn.
///
/// More kinds (text, images) will be added here. Each kind is drawn by its
/// own pipeline, so the order of commands is what keeps painting correct
/// across pipelines.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DrawCommand {
    /// Draw this range of the list's primitive instances in one draw call.
    Primitives(Range<u32>),
    /// Restrict everything drawn after this command to a rectangle, in pixels.
    /// `None` removes the restriction. A rectangle with no area hides everything.
    SetClip(Option<Rect>),
}

/// An ordered recording of everything to draw in one frame.
///
/// UI code records shapes into a `DrawList` and never touches the GPU.
/// Consecutive primitives drawn under the same clip are merged into one
/// command while recording, so they cost a single draw call.
///
/// # Clipping
///
/// `push_clip` restricts the following primitives to a rectangle, intersected
/// with any clip that is already active, and `pop_clip` goes back to the
/// previous one. Clips are rectangular and snap outward to whole pixels when
/// drawn. Primitives that fall entirely outside the active clip are dropped
/// while recording.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawList {
    primitives: Vec<PrimitiveInstance>,
    commands: Vec<DrawCommand>,
    /// Effective clip after each `push_clip`, innermost last.
    clip_stack: Vec<Rect>,
    /// The clip the command stream is currently in.
    emitted_clip: Option<Rect>,
}

impl DrawList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(primitives: usize) -> Self {
        Self {
            primitives: Vec::with_capacity(primitives),
            ..Self::default()
        }
    }

    /// Removes everything, including any open clips, but keeps the allocated
    /// memory for the next frame.
    pub fn clear(&mut self) {
        self.primitives.clear();
        self.commands.clear();
        self.clip_stack.clear();
        self.emitted_clip = None;
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn primitive_count(&self) -> usize {
        self.primitives.len()
    }

    /// Commands in drawing order.
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    /// All primitive instances, indexed by the ranges in `Primitives` commands.
    pub fn primitives(&self) -> &[PrimitiveInstance] {
        &self.primitives
    }

    /// The clip that applies to primitives pushed right now.
    pub fn current_clip(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }

    /// Number of clips that have been pushed and not popped yet.
    pub fn clip_depth(&self) -> usize {
        self.clip_stack.len()
    }

    /// Restricts the primitives pushed from now on to `rect`.
    ///
    /// The result is intersected with the active clip. An invalid rectangle
    /// (not finite, or with a negative size) or one that does not overlap the
    /// active clip hides everything until the matching `pop_clip`.
    pub fn push_clip(&mut self, rect: Rect) {
        let clip = match self.current_clip() {
            Some(current) => intersect_clips(current, rect),
            None if rect.is_valid() => rect,
            None => empty_clip(rect),
        };

        self.clip_stack.push(clip);
    }

    /// Undoes the latest `push_clip`. Does nothing if no clip is active.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    /// Runs `draw` with `rect` as the active clip and pops it afterwards, so
    /// pushes and pops can never get out of balance.
    pub fn with_clip(&mut self, rect: Rect, draw: impl FnOnce(&mut Self)) {
        self.push_clip(rect);
        draw(self);
        self.pop_clip();
    }

    /// Appends a primitive. It is drawn after everything already recorded,
    /// under the active clip.
    ///
    /// Primitives entirely outside the active clip are dropped. A list holds
    /// at most `u32::MAX - 1` primitives; anything beyond that is dropped.
    pub fn push(&mut self, primitive: PrimitiveInstance) {
        let clip = self.current_clip();

        if let Some(clip) = clip
            && (has_no_area(clip) || !is_visible_through(&primitive, clip))
        {
            return;
        }

        let Some(end) = u32::try_from(self.primitives.len())
            .ok()
            .and_then(|index| index.checked_add(1))
        else {
            return;
        };

        let start = end - 1;

        // The clip command is written lazily, right before the first primitive
        // that needs it, so empty or repeated clip scopes cost nothing.
        if clip != self.emitted_clip {
            self.commands.push(DrawCommand::SetClip(clip));
            self.emitted_clip = clip;
        }

        self.primitives.push(primitive);

        match self.commands.last_mut() {
            Some(DrawCommand::Primitives(range)) if range.end == start => range.end = end,
            _ => self.commands.push(DrawCommand::Primitives(start..end)),
        }
    }
}

impl Extend<PrimitiveInstance> for DrawList {
    fn extend<T: IntoIterator<Item = PrimitiveInstance>>(&mut self, iter: T) {
        for primitive in iter {
            self.push(primitive);
        }
    }
}

impl FromIterator<PrimitiveInstance> for DrawList {
    fn from_iter<T: IntoIterator<Item = PrimitiveInstance>>(iter: T) -> Self {
        let mut list = Self::new();

        list.extend(iter);

        list
    }
}

/// Whether `clip` covers no pixels at all. Also true for NaN sizes.
fn has_no_area(clip: Rect) -> bool {
    !(clip.size.width > 0.0 && clip.size.height > 0.0)
}

/// A clip that shows nothing, positioned where the offending rectangle starts.
fn empty_clip(rect: Rect) -> Rect {
    let x = if rect.position.x.is_finite() {
        rect.position.x
    } else {
        0.0
    };
    let y = if rect.position.y.is_finite() {
        rect.position.y
    } else {
        0.0
    };

    Rect::from_xywh(x, y, 0.0, 0.0)
}

fn intersect_clips(current: Rect, rect: Rect) -> Rect {
    current
        .intersection(rect)
        .unwrap_or_else(|| empty_clip(current))
}

/// Whether any pixel of `primitive` can show inside `clip`.
fn is_visible_through(primitive: &PrimitiveInstance, clip: Rect) -> bool {
    let [x, y, width, height] = primitive.bounds;

    let bounds = Rect::from_xywh(
        x - CULL_PADDING,
        y - CULL_PADDING,
        width + 2.0 * CULL_PADDING,
        height + 2.0 * CULL_PADDING,
    );

    bounds.intersects(clip)
}

#[cfg(test)]
mod tests {
    use super::*;

    use noxl_core::color::Color;

    fn primitive(x: f32) -> PrimitiveInstance {
        primitive_at(x, 0.0, 10.0, 10.0)
    }

    fn primitive_at(x: f32, y: f32, width: f32, height: f32) -> PrimitiveInstance {
        PrimitiveInstance::rect(Rect::from_xywh(x, y, width, height), Color::WHITE)
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect::from_xywh(x, y, width, height)
    }

    // ----- recording -----

    #[test]
    fn new_list_is_empty() {
        let list = DrawList::new();

        assert!(list.is_empty());
        assert_eq!(list.primitive_count(), 0);
        assert!(list.commands().is_empty());
        assert_eq!(list.current_clip(), None);
        assert_eq!(list.clip_depth(), 0);
    }

    #[test]
    fn one_primitive_makes_one_command() {
        let mut list = DrawList::new();

        list.push(primitive(0.0));

        assert!(!list.is_empty());
        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..1)]);
    }

    #[test]
    fn consecutive_primitives_merge_into_one_command() {
        let mut list = DrawList::new();

        for index in 0..5 {
            list.push(primitive(index as f32));
        }

        assert_eq!(list.primitive_count(), 5);
        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..5)]);
    }

    #[test]
    fn primitives_keep_insertion_order() {
        let list: DrawList = (0..4).map(|index| primitive(index as f32)).collect();

        let xs: Vec<f32> = list
            .primitives()
            .iter()
            .map(|instance| instance.bounds[0])
            .collect();

        assert_eq!(xs, [0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn extend_appends_after_existing_primitives() {
        let mut list = DrawList::new();

        list.push(primitive(0.0));
        list.extend([primitive(1.0), primitive(2.0)]);

        assert_eq!(list.primitive_count(), 3);
        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..3)]);
    }

    #[test]
    fn clear_resets_the_list_and_recording_starts_over() {
        let mut list = DrawList::new();

        list.extend([primitive(0.0), primitive(1.0)]);
        list.clear();

        assert!(list.is_empty());
        assert_eq!(list.primitive_count(), 0);

        list.push(primitive(5.0));

        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..1)]);
    }

    #[test]
    fn with_capacity_starts_empty() {
        assert!(DrawList::with_capacity(128).is_empty());
    }

    // ----- clipping: commands -----

    #[test]
    fn a_clip_is_written_before_the_first_primitive_that_uses_it() {
        let mut list = DrawList::new();
        let clip = rect(0.0, 0.0, 100.0, 100.0);

        list.push_clip(clip);
        list.push(primitive(10.0));

        assert_eq!(
            list.commands(),
            &[
                DrawCommand::SetClip(Some(clip)),
                DrawCommand::Primitives(0..1),
            ]
        );
    }

    #[test]
    fn popping_the_clip_restores_the_unclipped_state() {
        let mut list = DrawList::new();
        let clip = rect(0.0, 0.0, 100.0, 100.0);

        list.push_clip(clip);
        list.push(primitive(10.0));
        list.pop_clip();
        list.push(primitive(20.0));

        assert_eq!(
            list.commands(),
            &[
                DrawCommand::SetClip(Some(clip)),
                DrawCommand::Primitives(0..1),
                DrawCommand::SetClip(None),
                DrawCommand::Primitives(1..2),
            ]
        );
    }

    #[test]
    fn a_clip_scope_without_primitives_writes_nothing() {
        let mut list = DrawList::new();

        list.push_clip(rect(0.0, 0.0, 100.0, 100.0));
        list.pop_clip();
        list.push(primitive(0.0));

        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..1)]);
    }

    #[test]
    fn repeating_the_same_clip_keeps_primitives_in_one_batch() {
        let mut list = DrawList::new();
        let clip = rect(0.0, 0.0, 100.0, 100.0);

        list.push_clip(clip);
        list.push(primitive(10.0));
        list.pop_clip();
        list.push_clip(clip);
        list.push(primitive(20.0));

        assert_eq!(
            list.commands(),
            &[
                DrawCommand::SetClip(Some(clip)),
                DrawCommand::Primitives(0..2),
            ]
        );
    }

    #[test]
    fn a_new_clip_starts_a_new_batch() {
        let mut list = DrawList::new();
        let first = rect(0.0, 0.0, 50.0, 50.0);
        let second = rect(20.0, 20.0, 50.0, 50.0);

        list.with_clip(first, |list| list.push(primitive(10.0)));
        list.with_clip(second, |list| {
            list.push(primitive_at(30.0, 30.0, 10.0, 10.0))
        });

        assert_eq!(
            list.commands(),
            &[
                DrawCommand::SetClip(Some(first)),
                DrawCommand::Primitives(0..1),
                DrawCommand::SetClip(Some(second)),
                DrawCommand::Primitives(1..2),
            ]
        );
    }

    // ----- clipping: state -----

    #[test]
    fn nested_clips_intersect() {
        let mut list = DrawList::new();

        list.push_clip(rect(0.0, 0.0, 100.0, 100.0));
        list.push_clip(rect(50.0, 50.0, 100.0, 100.0));

        assert_eq!(list.current_clip(), Some(rect(50.0, 50.0, 50.0, 50.0)));
        assert_eq!(list.clip_depth(), 2);

        list.pop_clip();

        assert_eq!(list.current_clip(), Some(rect(0.0, 0.0, 100.0, 100.0)));
    }

    #[test]
    fn a_clip_can_never_grow_beyond_its_parent() {
        let mut list = DrawList::new();

        list.push_clip(rect(10.0, 10.0, 20.0, 20.0));
        list.push_clip(rect(0.0, 0.0, 500.0, 500.0));

        assert_eq!(list.current_clip(), Some(rect(10.0, 10.0, 20.0, 20.0)));
    }

    #[test]
    fn a_disjoint_clip_hides_everything_until_popped() {
        let mut list = DrawList::new();

        list.push_clip(rect(0.0, 0.0, 50.0, 50.0));
        list.push_clip(rect(200.0, 200.0, 50.0, 50.0));
        list.push(primitive_at(210.0, 210.0, 10.0, 10.0));
        list.push(primitive_at(10.0, 10.0, 10.0, 10.0));

        assert_eq!(list.primitive_count(), 0);

        list.pop_clip();
        list.push(primitive_at(10.0, 10.0, 10.0, 10.0));

        assert_eq!(list.primitive_count(), 1);
    }

    #[test]
    fn an_empty_clip_hides_everything() {
        let mut list = DrawList::new();

        list.push_clip(rect(10.0, 10.0, 0.0, 50.0));
        list.push(primitive_at(0.0, 0.0, 100.0, 100.0));

        assert_eq!(list.primitive_count(), 0);
    }

    #[test]
    fn an_invalid_clip_hides_everything() {
        for clip in [
            rect(f32::NAN, 0.0, 10.0, 10.0),
            rect(0.0, 0.0, -10.0, 10.0),
            rect(0.0, 0.0, f32::INFINITY, 10.0),
        ] {
            let mut list = DrawList::new();

            list.push_clip(clip);
            list.push(primitive_at(0.0, 0.0, 10.0, 10.0));

            assert_eq!(list.primitive_count(), 0, "clip {clip:?}");
        }
    }

    #[test]
    fn popping_without_a_clip_does_nothing() {
        let mut list = DrawList::new();

        list.pop_clip();
        list.push(primitive(0.0));

        assert_eq!(list.clip_depth(), 0);
        assert_eq!(list.primitive_count(), 1);
    }

    #[test]
    fn with_clip_is_always_balanced() {
        let mut list = DrawList::new();

        list.with_clip(rect(0.0, 0.0, 100.0, 100.0), |list| {
            assert_eq!(list.clip_depth(), 1);

            list.with_clip(rect(0.0, 0.0, 50.0, 50.0), |list| {
                assert_eq!(list.clip_depth(), 2);
            });

            assert_eq!(list.clip_depth(), 1);
        });

        assert_eq!(list.clip_depth(), 0);
        assert_eq!(list.current_clip(), None);
    }

    #[test]
    fn clear_drops_open_clips() {
        let mut list = DrawList::new();

        list.push_clip(rect(0.0, 0.0, 10.0, 10.0));
        list.push(primitive_at(0.0, 0.0, 5.0, 5.0));
        list.clear();

        assert_eq!(list.clip_depth(), 0);
        assert_eq!(list.current_clip(), None);

        // The next frame starts unclipped and writes no stale clip command.
        list.push(primitive(500.0));

        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..1)]);
    }

    // ----- clipping: culling -----

    #[test]
    fn primitives_outside_the_clip_are_dropped() {
        let mut list = DrawList::new();

        list.push_clip(rect(0.0, 0.0, 100.0, 100.0));
        list.push(primitive_at(10.0, 10.0, 20.0, 20.0));
        list.push(primitive_at(300.0, 10.0, 20.0, 20.0));
        list.push(primitive_at(10.0, -80.0, 20.0, 20.0));

        assert_eq!(list.primitive_count(), 1);
    }

    #[test]
    fn primitives_partly_inside_the_clip_are_kept() {
        let mut list = DrawList::new();

        list.push_clip(rect(0.0, 0.0, 100.0, 100.0));
        list.push(primitive_at(90.0, 90.0, 50.0, 50.0));
        list.push(primitive_at(-20.0, -20.0, 40.0, 40.0));

        assert_eq!(list.primitive_count(), 2);
    }

    #[test]
    fn a_primitive_touching_the_clip_edge_is_kept_for_anti_aliasing() {
        let mut list = DrawList::new();

        list.push_clip(rect(0.0, 0.0, 100.0, 100.0));
        // Ends exactly at the clip edge; its anti-aliased fringe can reach one pixel inside.
        list.push(primitive_at(-10.0, 0.0, 10.0, 10.0));

        assert_eq!(list.primitive_count(), 1);
    }

    #[test]
    fn unclipped_primitives_are_never_dropped() {
        let mut list = DrawList::new();

        list.push(primitive_at(-5000.0, -5000.0, 10.0, 10.0));
        list.push(primitive_at(5000.0, 5000.0, 10.0, 10.0));

        assert_eq!(list.primitive_count(), 2);
    }
}
