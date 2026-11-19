use noxl_core::component::ComponentId;
use noxl_core::geometry::Rect;
use noxl_core::tree::ComponentNode;

use crate::draw_list::DrawList;
use crate::primitive::PrimitiveInstance;

/// Converts a component tree into a `DrawList`.
///
/// `rect_of` supplies the final, absolute rectangle of each component, for
/// example from a layout result. Parents are recorded before their children,
/// so children paint on top.
///
/// Rules:
/// - Invisible components, and everything below them, are skipped.
/// - Opacity is multiplied down the tree.
/// - Components with a fully transparent background draw nothing, but their
///   children are still visited.
/// - Components without a rectangle, or with an empty or invalid one, draw nothing.
/// - A component with `clip_children` set clips its descendants to its own
///   rectangle, nested clips intersect, and the component's own background is
///   not affected. Without a rectangle there is nothing to clip to, so its
///   descendants are drawn unclipped.
///
/// This is the entry point the runtime should use: it is where text and
/// images will be interleaved with shapes later.
pub fn build_draw_list<F>(root: &ComponentNode, mut rect_of: F) -> DrawList
where
    F: FnMut(ComponentId) -> Option<Rect>,
{
    let mut list = DrawList::with_capacity(root.node_count());

    build_node(root, 1.0, &mut rect_of, &mut list);

    list
}

/// The primitives of `build_draw_list`, without its commands.
pub fn collect_primitives<F>(root: &ComponentNode, rect_of: F) -> Vec<PrimitiveInstance>
where
    F: FnMut(ComponentId) -> Option<Rect>,
{
    build_draw_list(root, rect_of).primitives().to_vec()
}

fn build_node<F>(node: &ComponentNode, inherited_opacity: f32, rect_of: &mut F, list: &mut DrawList)
where
    F: FnMut(ComponentId) -> Option<Rect>,
{
    let props = &node.component.props;
    let opacity = inherited_opacity * props.opacity;

    if !props.visible || opacity <= 0.0 {
        return;
    }

    let rect = rect_of(node.component.id);

    if props.background.a > 0
        && let Some(rect) = rect
        && rect.is_valid()
        && !rect.size.is_empty()
    {
        list.push(PrimitiveInstance::rect(rect, props.background).with_opacity(opacity));
    }

    let clip = rect.filter(|_| props.clip_children);

    if let Some(clip) = clip {
        list.push_clip(clip);
    }

    for child in &node.children {
        build_node(child, opacity, rect_of, list);
    }

    if clip.is_some() {
        list.pop_clip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use noxl_core::color::Color;
    use noxl_core::component::{Component, ComponentType};

    use crate::draw_list::DrawCommand;

    fn node(color: Color) -> ComponentNode {
        let component = Component::builder(ComponentType::Panel)
            .background(color)
            .build()
            .unwrap();

        ComponentNode::new(component)
    }

    fn clipping_node(color: Color) -> ComponentNode {
        let mut node = node(color);

        node.component.props.clip_children = true;

        node
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect::from_xywh(x, y, width, height)
    }

    /// Gives every component a 10x10 rect whose x equals its ID.
    fn rect_by_id(id: ComponentId) -> Option<Rect> {
        Some(rect(id.0 as f32, 0.0, 10.0, 10.0))
    }

    /// Looks rectangles up in a fixed table.
    fn lookup(table: Vec<(ComponentId, Rect)>) -> impl FnMut(ComponentId) -> Option<Rect> {
        move |id| {
            table
                .iter()
                .find(|(candidate, _)| *candidate == id)
                .map(|(_, rect)| *rect)
        }
    }

    // ----- drawing rules -----

    #[test]
    fn emits_parents_before_children() {
        let mut root = node(Color::BLACK);
        let mut first = node(Color::RED);
        let second = node(Color::BLUE);

        first.add_child(node(Color::GREEN)).unwrap();
        root.add_child(first).unwrap();
        root.add_child(second).unwrap();

        let mut expected = Vec::new();
        root.traverse(|node| expected.push(node.component.id.0 as f32));

        let instances = collect_primitives(&root, rect_by_id);
        let actual: Vec<f32> = instances
            .iter()
            .map(|instance| instance.bounds[0])
            .collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn skips_invisible_subtree() {
        let mut root = node(Color::BLACK);
        let mut hidden = node(Color::RED);

        hidden.component.props.visible = false;
        hidden.add_child(node(Color::GREEN)).unwrap();
        root.add_child(hidden).unwrap();

        assert_eq!(collect_primitives(&root, rect_by_id).len(), 1);
    }

    #[test]
    fn transparent_background_draws_nothing_but_children_remain() {
        let mut root = node(Color::TRANSPARENT);

        root.add_child(node(Color::RED)).unwrap();

        assert_eq!(collect_primitives(&root, rect_by_id).len(), 1);
    }

    #[test]
    fn multiplies_opacity_down_the_tree() {
        let mut root = node(Color::WHITE);
        let mut child = node(Color::WHITE);

        root.component.props.opacity = 0.5;
        child.component.props.opacity = 0.5;
        root.add_child(child).unwrap();

        let instances = collect_primitives(&root, rect_by_id);

        assert!((instances[0].color[3] - 0.5).abs() < 1e-6);
        assert!((instances[1].color[3] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn zero_opacity_skips_subtree() {
        let mut root = node(Color::WHITE);

        root.component.props.opacity = 0.0;
        root.add_child(node(Color::RED)).unwrap();

        assert!(collect_primitives(&root, rect_by_id).is_empty());
    }

    #[test]
    fn skips_components_without_a_rect() {
        let mut root = node(Color::BLACK);
        let child = node(Color::RED);
        let child_id = child.component.id;

        root.add_child(child).unwrap();

        let instances =
            collect_primitives(
                &root,
                |id| {
                    if id == child_id { None } else { rect_by_id(id) }
                },
            );

        assert_eq!(instances.len(), 1);
    }

    #[test]
    fn build_draw_list_batches_the_whole_tree_into_one_command() {
        let mut root = node(Color::BLACK);

        root.add_child(node(Color::RED)).unwrap();
        root.add_child(node(Color::BLUE)).unwrap();

        let list = build_draw_list(&root, rect_by_id);

        assert_eq!(list.primitive_count(), 3);
        assert_eq!(list.commands().len(), 1);
    }

    #[test]
    fn skips_empty_rects() {
        let root = node(Color::BLACK);

        let instances = collect_primitives(&root, |_| Some(rect(0.0, 0.0, 0.0, 10.0)));

        assert!(instances.is_empty());
    }

    // ----- clipping -----

    #[test]
    fn clip_children_clips_descendants_to_the_component_rect() {
        let mut root = clipping_node(Color::BLACK);
        let inside = node(Color::RED);
        let outside = node(Color::BLUE);

        let ids = [root.component.id, inside.component.id, outside.component.id];
        let root_rect = rect(0.0, 0.0, 100.0, 100.0);

        root.add_child(inside).unwrap();
        root.add_child(outside).unwrap();

        let list = build_draw_list(
            &root,
            lookup(vec![
                (ids[0], root_rect),
                (ids[1], rect(10.0, 10.0, 20.0, 20.0)),
                (ids[2], rect(300.0, 300.0, 20.0, 20.0)),
            ]),
        );

        // The root background, then the child that is inside the clip. The child
        // outside the clip is dropped.
        assert_eq!(list.primitive_count(), 2);
        assert_eq!(
            list.commands(),
            &[
                DrawCommand::Primitives(0..1),
                DrawCommand::SetClip(Some(root_rect)),
                DrawCommand::Primitives(1..2),
            ]
        );
    }

    #[test]
    fn without_clip_children_nothing_is_clipped_or_dropped() {
        let mut root = node(Color::BLACK);
        let inside = node(Color::RED);
        let outside = node(Color::BLUE);

        let ids = [root.component.id, inside.component.id, outside.component.id];

        root.add_child(inside).unwrap();
        root.add_child(outside).unwrap();

        let list = build_draw_list(
            &root,
            lookup(vec![
                (ids[0], rect(0.0, 0.0, 100.0, 100.0)),
                (ids[1], rect(10.0, 10.0, 20.0, 20.0)),
                (ids[2], rect(300.0, 300.0, 20.0, 20.0)),
            ]),
        );

        assert_eq!(list.primitive_count(), 3);
        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..3)]);
    }

    #[test]
    fn nested_clips_intersect() {
        let mut root = clipping_node(Color::TRANSPARENT);
        let mut middle = clipping_node(Color::TRANSPARENT);
        let leaf = node(Color::RED);

        let ids = [root.component.id, middle.component.id, leaf.component.id];

        middle.add_child(leaf).unwrap();
        root.add_child(middle).unwrap();

        let list = build_draw_list(
            &root,
            lookup(vec![
                (ids[0], rect(0.0, 0.0, 100.0, 100.0)),
                (ids[1], rect(50.0, 50.0, 100.0, 100.0)),
                (ids[2], rect(60.0, 60.0, 10.0, 10.0)),
            ]),
        );

        assert_eq!(
            list.commands(),
            &[
                DrawCommand::SetClip(Some(rect(50.0, 50.0, 50.0, 50.0))),
                DrawCommand::Primitives(0..1),
            ]
        );
    }

    #[test]
    fn a_clip_only_applies_inside_its_component() {
        let mut root = node(Color::TRANSPARENT);
        let mut panel = clipping_node(Color::RED);
        let inner = node(Color::BLUE);
        let sibling = node(Color::GREEN);

        let ids = [
            panel.component.id,
            inner.component.id,
            sibling.component.id,
            root.component.id,
        ];
        let panel_rect = rect(0.0, 0.0, 50.0, 50.0);

        panel.add_child(inner).unwrap();
        root.add_child(panel).unwrap();
        root.add_child(sibling).unwrap();

        let list = build_draw_list(
            &root,
            lookup(vec![
                (ids[3], rect(0.0, 0.0, 200.0, 200.0)),
                (ids[0], panel_rect),
                (ids[1], rect(10.0, 10.0, 10.0, 10.0)),
                (ids[2], rect(60.0, 60.0, 10.0, 10.0)),
            ]),
        );

        // The panel's own background is not clipped, its child is, and the
        // sibling after the panel is drawn without any clip again.
        assert_eq!(
            list.commands(),
            &[
                DrawCommand::Primitives(0..1),
                DrawCommand::SetClip(Some(panel_rect)),
                DrawCommand::Primitives(1..2),
                DrawCommand::SetClip(None),
                DrawCommand::Primitives(2..3),
            ]
        );
    }

    #[test]
    fn a_component_without_a_rect_cannot_clip() {
        let mut root = clipping_node(Color::BLACK);
        let child = node(Color::RED);

        let child_id = child.component.id;

        root.add_child(child).unwrap();

        let list = build_draw_list(
            &root,
            lookup(vec![(child_id, rect(500.0, 500.0, 10.0, 10.0))]),
        );

        assert_eq!(list.commands(), &[DrawCommand::Primitives(0..1)]);
    }

    #[test]
    fn a_clipping_component_with_no_area_hides_its_descendants() {
        let mut root = clipping_node(Color::TRANSPARENT);
        let child = node(Color::RED);

        let ids = [root.component.id, child.component.id];

        root.add_child(child).unwrap();

        let list = build_draw_list(
            &root,
            lookup(vec![
                (ids[0], rect(0.0, 0.0, 0.0, 100.0)),
                (ids[1], rect(0.0, 0.0, 10.0, 10.0)),
            ]),
        );

        assert!(list.is_empty());
    }
}
