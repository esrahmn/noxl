use crate::engine::{LayoutEngine, LayoutError};
use crate::node::{LayoutNode, NodeError};
use crate::style::{CrossAlignment, Edges, LayoutDirection, LayoutStyle, MainAlignment, SizeSpec};
use noxl_core::{
    component::ComponentId,
    geometry::{Position, Size},
};

#[test]
fn percentage_size_respects_minimum_size() {
    let mut root = root(LayoutDirection::Row);

    root.add_child(LayoutNode::new(ComponentId(2)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Percent(0.5),
        min_size: Size::new(250.0, 60.0),
        align_items: CrossAlignment::Start,
        ..Default::default()
    }));

    let result = engine().compute(&root, Size::new(400.0, 100.0)).unwrap();

    assert_size(&result, 2, 250.0, 60.0);
}

#[test]
fn percentage_size_respects_maximum_size() {
    let mut root = root(LayoutDirection::Row);

    root.add_child(LayoutNode::new(ComponentId(2)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Percent(0.5),
        max_size: Size::new(150.0, 30.0),
        align_items: CrossAlignment::Start,
        ..Default::default()
    }));

    let result = engine().compute(&root, Size::new(400.0, 100.0)).unwrap();

    assert_size(&result, 2, 150.0, 30.0);
}

#[test]
fn flex_grow_works_with_percentage_sized_children() {
    let mut root = root(LayoutDirection::Row);

    let mut first = LayoutNode::new(ComponentId(2)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Px(20.0),
        flex_grow: 1.0,
        align_items: CrossAlignment::Start,
        ..Default::default()
    });

    first.style.max_size.width = 400.0;

    root.add_child(first);
    root.add_child(node(3, 100.0, 20.0));

    let result = engine().compute(&root, Size::new(400.0, 100.0)).unwrap();

    assert_size(&result, 2, 300.0, 20.0);
    assert_size(&result, 3, 100.0, 20.0);
}

#[test]
fn percentage_size_respects_parent_padding_and_child_margin() {
    let mut root = root(LayoutDirection::Column);

    let mut parent = node(2, 400.0, 100.0);
    parent.style.direction = LayoutDirection::Row;
    parent.style.padding = Edges::all(20.0);

    let mut child = LayoutNode::new(ComponentId(3)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Px(20.0),
        margin: Edges::new(0.0, 10.0, 0.0, 10.0),
        align_items: CrossAlignment::Start,
        ..Default::default()
    });

    child.style.min_size.width = 0.0;

    parent.add_child(child);
    root.add_child(parent);

    let result = engine().compute(&root, Size::new(800.0, 600.0)).unwrap();

    assert_size(&result, 3, 180.0, 20.0);
    assert_position(&result, 3, 30.0, 20.0);
}

#[test]
fn resolves_percentage_child_from_parent_content_size() {
    let mut root = root(LayoutDirection::Column);

    let mut parent = node(2, 400.0, 100.0);
    parent.style.direction = LayoutDirection::Row;
    parent.style.padding = Edges::all(20.0);

    let child = node(3, 0.0, 20.0);
    let mut child = child;
    child.style.width = SizeSpec::Percent(0.5);

    parent.add_child(child);
    root.add_child(parent);

    let result = engine().compute(&root, Size::new(800.0, 600.0)).unwrap();

    assert_size(&result, 3, 180.0, 20.0);
    assert_position(&result, 3, 20.0, 20.0);
}

#[test]
fn percentage_child_does_not_determine_auto_parent_size() {
    let mut root = root(LayoutDirection::Column);

    let mut parent = node(2, 0.0, 100.0);
    parent.style.width = SizeSpec::Auto;
    parent.style.direction = LayoutDirection::Row;

    let mut child = node(3, 0.0, 20.0);
    child.style.width = SizeSpec::Percent(0.5);

    parent.add_child(child);
    root.add_child(parent);

    let result = engine().compute(&root, Size::new(800.0, 600.0)).unwrap();

    assert_size(&result, 2, 0.0, 100.0);
    assert_size(&result, 3, 0.0, 20.0);
}

#[test]
fn resolves_nested_percentage_sizes() {
    let mut root = root(LayoutDirection::Column);

    let mut parent = node(2, 0.0, 100.0);
    parent.style.width = SizeSpec::Percent(0.5);
    parent.style.direction = LayoutDirection::Row;

    let mut child = node(3, 0.0, 20.0);
    child.style.width = SizeSpec::Percent(0.5);

    parent.add_child(child);
    root.add_child(parent);

    let result = engine().compute(&root, Size::new(800.0, 600.0)).unwrap();

    assert_size(&result, 2, 400.0, 100.0);
    assert_size(&result, 3, 200.0, 20.0);
}

fn node(id: u64, width: f32, height: f32) -> LayoutNode {
    LayoutNode::new(ComponentId(id)).with_style(LayoutStyle {
        width: SizeSpec::Px(width),
        height: SizeSpec::Px(height),
        align_items: CrossAlignment::Start,
        ..Default::default()
    })
}

fn root(direction: LayoutDirection) -> LayoutNode {
    LayoutNode::new(ComponentId(1)).with_style(LayoutStyle {
        direction,
        align_items: CrossAlignment::Start,
        ..Default::default()
    })
}

fn engine() -> LayoutEngine {
    LayoutEngine::new()
}

fn assert_position(result: &crate::LayoutResult, id: u64, x: f32, y: f32) {
    let item = result.find(ComponentId(id)).unwrap();

    assert_eq!(item.rect.position, Position::new(x, y));
}

fn assert_size(result: &crate::LayoutResult, id: u64, width: f32, height: f32) {
    let item = result.find(ComponentId(id)).unwrap();

    assert_eq!(item.rect.size, Size::new(width, height));
}

#[test]
fn computes_horizontal_layout() {
    let mut root = root(LayoutDirection::Row);

    root.add_child(node(2, 20.0, 10.0));
    root.add_child(node(3, 30.0, 10.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_eq!(result.len(), 3);

    assert_position(&result, 2, 0.0, 0.0);
    assert_position(&result, 3, 20.0, 0.0);

    assert_size(&result, 2, 20.0, 10.0);
    assert_size(&result, 3, 30.0, 10.0);
}

#[test]
fn computes_vertical_layout() {
    let mut root = root(LayoutDirection::Column);

    root.add_child(node(2, 20.0, 10.0));
    root.add_child(node(3, 20.0, 30.0));

    let result = engine().compute(&root, Size::new(100.0, 100.0)).unwrap();

    assert_position(&result, 2, 0.0, 0.0);
    assert_position(&result, 3, 0.0, 10.0);
}

#[test]
fn applies_gap_between_children() {
    let mut root = root(LayoutDirection::Row);

    root.style.gap = 10.0;

    root.add_child(node(2, 20.0, 10.0));
    root.add_child(node(3, 30.0, 10.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_position(&result, 3, 30.0, 0.0);
}

#[test]
fn applies_padding_to_children() {
    let mut root = root(LayoutDirection::Row);

    root.style.padding = crate::Edges::all(10.0);

    root.add_child(node(2, 20.0, 10.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_position(&result, 2, 10.0, 10.0);
}

#[test]
fn distributes_flex_grow() {
    let mut root = root(LayoutDirection::Row);

    let mut first = node(2, 20.0, 10.0);
    first.style.flex_grow = 1.0;

    let mut second = node(3, 20.0, 10.0);
    second.style.flex_grow = 1.0;

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_size(&result, 2, 50.0, 10.0);
    assert_size(&result, 3, 50.0, 10.0);
}

#[test]
fn distributes_flex_shrink() {
    let mut root = root(LayoutDirection::Row);

    let mut first = node(2, 80.0, 10.0);
    first.style.flex_shrink = 1.0;

    let mut second = node(3, 80.0, 10.0);
    second.style.flex_shrink = 1.0;

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_size(&result, 2, 50.0, 10.0);
    assert_size(&result, 3, 50.0, 10.0);
}

#[test]
fn centers_children_on_main_axis() {
    let mut root = root(LayoutDirection::Row);

    root.style.justify_content = MainAlignment::Center;

    root.add_child(node(2, 20.0, 10.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_position(&result, 2, 40.0, 0.0);
}

#[test]
fn rejects_invalid_available_size() {
    let root = root(LayoutDirection::Row);

    let result = engine().compute(&root, Size::new(-1.0, 50.0));

    assert!(result.is_err());
}

#[test]
fn calculates_nested_layout() {
    let mut root = root(LayoutDirection::Column);

    let mut container = node(2, 100.0, 50.0);
    container.style.direction = LayoutDirection::Row;

    container.add_child(node(3, 20.0, 10.0));
    container.add_child(node(4, 30.0, 10.0));

    root.add_child(container);

    let result = engine().compute(&root, Size::new(100.0, 100.0)).unwrap();

    assert_position(&result, 2, 0.0, 0.0);
    assert_position(&result, 3, 0.0, 0.0);
    assert_position(&result, 4, 20.0, 0.0);
}

#[test]
fn flex_grow_redistributes_space_after_max_size() {
    let mut root = root(LayoutDirection::Row);

    let mut first = node(2, 20.0, 10.0);
    first.style.flex_grow = 1.0;
    first.style.max_size.width = 30.0;

    let mut second = node(3, 20.0, 10.0);
    second.style.flex_grow = 1.0;

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_size(&result, 2, 30.0, 10.0);
    assert_size(&result, 3, 70.0, 10.0);
}

#[test]
fn flex_shrink_redistributes_space_after_min_size() {
    let mut root = root(LayoutDirection::Row);

    let mut first = node(2, 80.0, 10.0);
    first.style.flex_shrink = 1.0;
    first.style.min_size.width = 60.0;

    let mut second = node(3, 80.0, 10.0);
    second.style.flex_shrink = 1.0;

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_size(&result, 2, 60.0, 10.0);
    assert_size(&result, 3, 40.0, 10.0);
}

#[test]
fn applies_asymmetric_horizontal_margins() {
    let mut root = root(LayoutDirection::Row);

    let mut first = node(2, 20.0, 10.0);
    first.style.margin.left = 5.0;
    first.style.margin.right = 15.0;

    let second = node(3, 20.0, 10.0);

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_position(&result, 2, 5.0, 0.0);
    assert_position(&result, 3, 40.0, 0.0);
}

#[test]
fn centers_child_on_cross_axis() {
    let mut root = root(LayoutDirection::Row);
    root.style.align_items = CrossAlignment::Center;

    root.add_child(node(2, 20.0, 10.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_position(&result, 2, 0.0, 20.0);
}

#[test]
fn lays_out_a_leaf_child_without_children() {
    let root = root(LayoutDirection::Row).with_child(LayoutNode::new(ComponentId(2)));

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_eq!(result.len(), 2);
    assert_position(&result, 2, 0.0, 0.0);
    assert_size(&result, 2, 0.0, 0.0);
}

#[test]
fn handles_empty_children_with_space_evenly() {
    let mut root = root(LayoutDirection::Row);
    root.style.justify_content = MainAlignment::SpaceEvenly;

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_eq!(result.len(), 1);
    assert!(result.find(ComponentId(1)).is_some());
}

#[test]
fn applies_asymmetric_vertical_margins() {
    let mut root = root(LayoutDirection::Column);

    let mut first = node(2, 20.0, 10.0);
    first.style.margin.top = 5.0;
    first.style.margin.bottom = 15.0;

    let second = node(3, 20.0, 10.0);

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 100.0)).unwrap();

    assert_position(&result, 2, 0.0, 5.0);
    assert_position(&result, 3, 0.0, 30.0);
}

#[test]
fn rejects_layout_when_gap_arithmetic_overflows() {
    let mut root = root(LayoutDirection::Row);
    root.style.gap = f32::MAX;
    root.add_child(node(2, 0.0, 0.0));
    root.add_child(node(3, 0.0, 0.0));
    root.add_child(node(4, 0.0, 0.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0));

    assert_eq!(result, Err(LayoutError::ArithmeticOverflow));
}

#[test]
fn rejects_arithmetic_overflow_from_child_margin() {
    let mut root = root(LayoutDirection::Row);

    let mut child = node(2, f32::MAX, 10.0);
    child.style.margin.left = f32::MAX;
    root.add_child(child);

    let result = engine().compute(&root, Size::new(100.0, 50.0));

    assert_eq!(result, Err(LayoutError::ArithmeticOverflow));
}

#[test]
fn rejects_layout_when_flex_grow_factor_sum_overflows() {
    let mut root = root(LayoutDirection::Row);

    let mut first = node(2, 0.0, 10.0);
    first.style.flex_grow = f32::MAX;
    let mut second = node(3, 0.0, 10.0);
    second.style.flex_grow = f32::MAX;

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 50.0));

    assert_eq!(result, Err(LayoutError::ArithmeticOverflow));
}

#[test]
fn rejects_layout_when_child_far_edge_overflows() {
    let mut root = root(LayoutDirection::Row);
    root.style.padding.left = f32::MAX / 2.0;

    let mut child = node(2, f32::MAX, 10.0);
    child.style.min_size.width = f32::MAX;
    root.add_child(child);

    let result = engine().compute(&root, Size::new(f32::MAX, 50.0));

    assert_eq!(result, Err(LayoutError::ArithmeticOverflow));
}

#[test]
fn rejects_duplicate_component_ids_in_layout_tree() {
    let mut root = root(LayoutDirection::Row);
    root.add_child(node(2, 10.0, 10.0));
    root.add_child(node(2, 20.0, 20.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0));

    assert_eq!(
        result,
        Err(LayoutError::InvalidNode(NodeError::DuplicateId {
            id: ComponentId(2)
        }))
    );
}

#[test]
fn rejects_duplicate_component_ids_between_root_and_child() {
    let mut root = root(LayoutDirection::Row);
    root.add_child(node(1, 20.0, 10.0));

    let result = engine().compute(&root, Size::new(100.0, 50.0));

    assert!(matches!(
        result,
        Err(LayoutError::InvalidNode(NodeError::DuplicateId {
            id: ComponentId(1)
        }))
    ));
}

#[test]
fn preserves_minimum_sizes_when_available_space_is_insufficient() {
    let mut root = root(LayoutDirection::Row);

    let mut first = node(2, 80.0, 10.0);
    first.style.flex_shrink = 1.0;
    first.style.min_size.width = 60.0;
    let mut second = node(3, 80.0, 10.0);
    second.style.flex_shrink = 1.0;
    second.style.min_size.width = 60.0;

    root.add_child(first);
    root.add_child(second);

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_size(&result, 2, 60.0, 10.0);
    assert_size(&result, 3, 60.0, 10.0);
    // Minimum constraints take priority over fitting the parent.
    assert_position(&result, 3, 60.0, 0.0);
}

#[test]
fn nested_percentages_use_each_parent_content_area() {
    let mut parent = node(2, 200.0, 100.0);
    parent.style.padding = crate::style::Edges::all(10.0);

    let mut child = LayoutNode::new(ComponentId(3)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Percent(0.5),
        align_items: CrossAlignment::Start,
        ..Default::default()
    });
    let grandchild = LayoutNode::new(ComponentId(4)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Percent(0.5),
        align_items: CrossAlignment::Start,
        ..Default::default()
    });
    child.add_child(grandchild);
    parent.add_child(child);

    let root = root(LayoutDirection::Column).with_child(parent);
    let result = engine().compute(&root, Size::new(500.0, 400.0)).unwrap();

    assert_size(&result, 2, 200.0, 100.0);
    assert_size(&result, 3, 90.0, 40.0);
    assert_size(&result, 4, 45.0, 20.0);
    assert_position(&result, 3, 10.0, 10.0);
    assert_position(&result, 4, 10.0, 10.0);
}

#[test]
fn percentage_cross_axis_size_is_not_overridden_by_stretch() {
    let mut root = root(LayoutDirection::Row);
    root.style.align_items = CrossAlignment::Stretch;
    root.add_child(LayoutNode::new(ComponentId(2)).with_style(LayoutStyle {
        width: SizeSpec::Px(30.0),
        height: SizeSpec::Percent(0.5),
        ..Default::default()
    }));

    let result = engine().compute(&root, Size::new(200.0, 100.0)).unwrap();

    assert_size(&result, 2, 30.0, 50.0);
}

#[test]
fn percentage_child_content_does_not_contribute_to_auto_parent_size() {
    let mut parent = LayoutNode::new(ComponentId(2)).with_style(LayoutStyle {
        width: SizeSpec::Auto,
        height: SizeSpec::Px(100.0),
        direction: LayoutDirection::Column,
        align_items: CrossAlignment::Start,
        ..Default::default()
    });

    let mut percentage_child = LayoutNode::new(ComponentId(3)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Px(20.0),
        align_items: CrossAlignment::Start,
        ..Default::default()
    });
    percentage_child.add_child(node(4, 80.0, 10.0));

    parent.add_child(percentage_child);
    parent.add_child(node(5, 40.0, 10.0));

    let root = root(LayoutDirection::Column).with_child(parent);
    let result = engine().compute(&root, Size::new(300.0, 200.0)).unwrap();

    assert_size(&result, 2, 40.0, 100.0);
    assert_size(&result, 3, 20.0, 20.0);
}

#[test]
fn percentages_do_not_contribute_to_auto_parent_intrinsic_size() {
    let mut parent = LayoutNode::new(ComponentId(2)).with_style(LayoutStyle {
        width: SizeSpec::Auto,
        height: SizeSpec::Px(100.0),
        min_size: Size::new(120.0, 0.0),
        padding: crate::style::Edges::all(10.0),
        align_items: CrossAlignment::Start,
        ..Default::default()
    });
    parent.add_child(LayoutNode::new(ComponentId(3)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Percent(0.5),
        align_items: CrossAlignment::Start,
        ..Default::default()
    }));

    let root = root(LayoutDirection::Column).with_child(parent);
    let result = engine().compute(&root, Size::new(500.0, 400.0)).unwrap();

    assert_size(&result, 2, 120.0, 100.0);
    assert_size(&result, 3, 50.0, 40.0);
    assert_position(&result, 3, 10.0, 10.0);
}

#[test]
fn auto_parent_without_intrinsic_content_uses_minimum_or_zero() {
    let mut parent_with_minimum = LayoutNode::new(ComponentId(2)).with_style(LayoutStyle {
        min_size: Size::new(30.0, 20.0),
        ..Default::default()
    });
    parent_with_minimum.add_child(LayoutNode::new(ComponentId(4)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Percent(0.5),
        ..Default::default()
    }));

    let mut parent_without_minimum = LayoutNode::new(ComponentId(3));
    parent_without_minimum.add_child(LayoutNode::new(ComponentId(5)).with_style(LayoutStyle {
        width: SizeSpec::Percent(0.5),
        height: SizeSpec::Percent(0.5),
        ..Default::default()
    }));

    let mut root = root(LayoutDirection::Row);
    root.add_child(parent_with_minimum);
    root.add_child(parent_without_minimum);

    let result = engine().compute(&root, Size::new(100.0, 50.0)).unwrap();

    assert_size(&result, 2, 30.0, 20.0);
    assert_size(&result, 3, 0.0, 0.0);
    assert_size(&result, 4, 15.0, 10.0);
    assert_size(&result, 5, 0.0, 0.0);
}
