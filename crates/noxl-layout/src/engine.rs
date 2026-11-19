use noxl_core::{
    component::ComponentId,
    geometry::{Position, Rect, Size},
};

use crate::node::{LayoutNode, NodeError};
use crate::style::{CrossAlignment, LayoutDirection, MainAlignment, SizeSpec};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutBox {
    pub id: ComponentId,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutResult {
    boxes: Vec<LayoutBox>,
}

impl LayoutResult {
    pub fn boxes(&self) -> &[LayoutBox] {
        &self.boxes
    }

    pub fn find(&self, id: ComponentId) -> Option<&LayoutBox> {
        self.boxes.iter().find(|item| item.id == id)
    }

    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    InvalidAvailableSize,
    ArithmeticOverflow,
    InvalidNode(NodeError),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAvailableSize => {
                f.write_str("available layout size must be finite and non-negative")
            }
            Self::ArithmeticOverflow => {
                f.write_str("layout calculation produced a non-finite value")
            }
            Self::InvalidNode(error) => {
                write!(f, "invalid layout node: {error}")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct LayoutEngine;

impl LayoutEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn compute(&self, root: &LayoutNode, available: Size) -> Result<LayoutResult, LayoutError> {
        if !available.is_valid() {
            return Err(LayoutError::InvalidAvailableSize);
        }

        root.validate().map_err(LayoutError::InvalidNode)?;

        let mut result = LayoutResult::default();

        self.arrange_node(root, Position::ZERO, available, &mut result)?;

        Ok(result)
    }

    fn measure(&self, node: &LayoutNode, available: MeasureAvailable) -> Result<Size, LayoutError> {
        let style = &node.style;

        let child_available = MeasureAvailable {
            width: inner_available_for_children(
                style.width,
                available.width,
                style.min_size.width,
                style.max_size.width,
                style.padding.horizontal(),
            )?,
            height: inner_available_for_children(
                style.height,
                available.height,
                style.min_size.height,
                style.max_size.height,
                style.padding.vertical(),
            )?,
        };

        let mut intrinsic = Size::ZERO;

        if !node.children.is_empty() {
            let mut main_total = 0.0_f32;
            let mut cross_max = 0.0_f32;

            for child in &node.children {
                let child_size = self.measure(child, child_available)?;
                let margin = child.style.margin;

                let (main, cross) = match style.direction {
                    LayoutDirection::Row => (
                        checked_add(child_size.width, margin.horizontal())?,
                        checked_add(child_size.height, margin.vertical())?,
                    ),
                    LayoutDirection::Column => (
                        checked_add(child_size.height, margin.vertical())?,
                        checked_add(child_size.width, margin.horizontal())?,
                    ),
                };

                main_total = checked_add(main_total, main)?;
                cross_max = cross_max.max(cross);
            }

            let gap_count = node.children.len().saturating_sub(1) as f32;
            let total_gap = checked_mul(style.gap, gap_count)?;
            main_total = checked_add(main_total, total_gap)?;

            intrinsic = match style.direction {
                LayoutDirection::Row => Size::new(main_total, cross_max),
                LayoutDirection::Column => Size::new(cross_max, main_total),
            };
        }

        intrinsic.width = checked_add(intrinsic.width, style.padding.horizontal())?;
        intrinsic.height = checked_add(intrinsic.height, style.padding.vertical())?;

        let width = resolve_size_spec(style.width, available.width, intrinsic.width)?;
        let height = resolve_size_spec(style.height, available.height, intrinsic.height)?;

        let size = Size::new(
            width.clamp(style.min_size.width, style.max_size.width),
            height.clamp(style.min_size.height, style.max_size.height),
        );

        if !size.is_valid() {
            return Err(LayoutError::ArithmeticOverflow);
        }

        Ok(size)
    }

    fn arrange_node(
        &self,
        node: &LayoutNode,
        position: Position,
        size: Size,
        result: &mut LayoutResult,
    ) -> Result<(), LayoutError> {
        if !position.is_finite() || !size.is_valid() {
            return Err(LayoutError::ArithmeticOverflow);
        }

        let right = checked_add(position.x, size.width)?;
        let bottom = checked_add(position.y, size.height)?;

        let rect = Rect::new(position, size);

        // Check that the rectangle's far edges remain finite.
        if !right.is_finite() || !bottom.is_finite() {
            return Err(LayoutError::ArithmeticOverflow);
        }

        result.boxes.push(LayoutBox { id: node.id, rect });

        if node.children.is_empty() {
            return Ok(());
        }

        let style = &node.style;
        let padding = style.padding;

        let inner_position = Position::new(
            checked_add(position.x, padding.left)?,
            checked_add(position.y, padding.top)?,
        );

        let inner_size = Size::new(
            (size.width - padding.horizontal()).max(0.0),
            (size.height - padding.vertical()).max(0.0),
        );

        let is_row = style.direction == LayoutDirection::Row;

        let main_available = if is_row {
            inner_size.width
        } else {
            inner_size.height
        };

        let cross_available = if is_row {
            inner_size.height
        } else {
            inner_size.width
        };

        let mut measurements = Vec::with_capacity(node.children.len());
        let mut bases = Vec::with_capacity(node.children.len());
        let mut minimums = Vec::with_capacity(node.children.len());
        let mut maximums = Vec::with_capacity(node.children.len());
        let mut grow_factors = Vec::with_capacity(node.children.len());
        let mut shrink_factors = Vec::with_capacity(node.children.len());

        let mut total_main = 0.0_f32;

        for child in &node.children {
            let measured = self.measure(child, MeasureAvailable::definite(inner_size))?;
            let child_style = &child.style;
            let margin = child_style.margin;

            let (
                main,
                cross,
                main_leading,
                main_trailing,
                cross_leading,
                cross_trailing,
                main_min,
                main_max,
                cross_min,
                cross_max,
            ) = if is_row {
                (
                    measured.width,
                    measured.height,
                    margin.left,
                    margin.right,
                    margin.top,
                    margin.bottom,
                    child_style.min_size.width,
                    child_style.max_size.width,
                    child_style.min_size.height,
                    child_style.max_size.height,
                )
            } else {
                (
                    measured.height,
                    measured.width,
                    margin.top,
                    margin.bottom,
                    margin.left,
                    margin.right,
                    child_style.min_size.height,
                    child_style.max_size.height,
                    child_style.min_size.width,
                    child_style.max_size.width,
                )
            };

            let base = main.clamp(main_min, main_max);

            total_main = checked_add(total_main, base)?;
            total_main = checked_add(total_main, main_leading)?;
            total_main = checked_add(total_main, main_trailing)?;

            bases.push(base);
            minimums.push(main_min);
            maximums.push(main_max);
            grow_factors.push(child_style.flex_grow);
            shrink_factors.push(child_style.flex_shrink);

            measurements.push(ChildMeasure {
                cross,
                cross_is_auto: if is_row {
                    matches!(child_style.height, SizeSpec::Auto)
                } else {
                    matches!(child_style.width, SizeSpec::Auto)
                },
                cross_leading,
                cross_trailing,
                cross_min,
                cross_max,
                main_leading,
                main_trailing,
            });
        }

        let gap_count = node.children.len().saturating_sub(1) as f32;
        let gap_total = checked_mul(style.gap, gap_count)?;
        total_main = checked_add(total_main, gap_total)?;

        let free_space = main_available - total_main;

        if !free_space.is_finite() {
            return Err(LayoutError::ArithmeticOverflow);
        }

        let main_sizes = distribute_flex(
            &bases,
            &minimums,
            &maximums,
            &grow_factors,
            &shrink_factors,
            free_space,
        )?;

        let used_main = sum_main_sizes(&main_sizes, &measurements, gap_total)?;

        let remaining = (main_available - used_main).max(0.0);

        if !remaining.is_finite() {
            return Err(LayoutError::ArithmeticOverflow);
        }

        let (initial_offset, extra_gap) =
            calculate_main_alignment(style.justify_content, remaining, node.children.len())?;

        let mut cursor = initial_offset;

        for (index, child) in node.children.iter().enumerate() {
            let measurement = measurements[index];

            cursor = checked_add(cursor, measurement.main_leading)?;

            let cross_inner =
                (cross_available - measurement.cross_leading - measurement.cross_trailing).max(0.0);

            let cross_size = match style.align_items {
                CrossAlignment::Stretch if measurement.cross_is_auto => {
                    cross_inner.clamp(measurement.cross_min, measurement.cross_max)
                }
                _ => measurement
                    .cross
                    .clamp(measurement.cross_min, measurement.cross_max),
            };

            let cross_offset = match style.align_items {
                CrossAlignment::Start | CrossAlignment::Stretch => 0.0,
                CrossAlignment::Center => (cross_inner - cross_size).max(0.0) / 2.0,
                CrossAlignment::End => (cross_inner - cross_size).max(0.0),
            };

            let cross_position = checked_add(measurement.cross_leading, cross_offset)?;

            let child_position = if is_row {
                Position::new(
                    checked_add(inner_position.x, cursor)?,
                    checked_add(inner_position.y, cross_position)?,
                )
            } else {
                Position::new(
                    checked_add(inner_position.x, cross_position)?,
                    checked_add(inner_position.y, cursor)?,
                )
            };

            let child_size = if is_row {
                Size::new(main_sizes[index], cross_size)
            } else {
                Size::new(cross_size, main_sizes[index])
            };

            self.arrange_node(child, child_position, child_size, result)?;

            cursor = checked_add(cursor, main_sizes[index])?;
            cursor = checked_add(cursor, measurement.main_trailing)?;
            cursor = checked_add(cursor, style.gap)?;
            cursor = checked_add(cursor, extra_gap)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct MeasureAvailable {
    width: Option<f32>,
    height: Option<f32>,
}

impl MeasureAvailable {
    fn definite(size: Size) -> Self {
        Self {
            width: Some(size.width),
            height: Some(size.height),
        }
    }
}

fn inner_available_for_children(
    spec: SizeSpec,
    available: Option<f32>,
    minimum: f32,
    maximum: f32,
    padding: f32,
) -> Result<Option<f32>, LayoutError> {
    let outer = match spec {
        SizeSpec::Auto => return Ok(None),
        SizeSpec::Px(value) => value,
        SizeSpec::Percent(value) => match available {
            Some(available) => checked_mul(available, value)?,
            None => return Ok(None),
        },
    };

    let inner = (outer.clamp(minimum, maximum) - padding).max(0.0);

    if inner.is_finite() {
        Ok(Some(inner))
    } else {
        Err(LayoutError::ArithmeticOverflow)
    }
}

fn resolve_size_spec(
    spec: SizeSpec,
    available: Option<f32>,
    intrinsic: f32,
) -> Result<f32, LayoutError> {
    match spec {
        SizeSpec::Auto => Ok(intrinsic),
        SizeSpec::Px(value) => Ok(value),
        SizeSpec::Percent(value) => match available {
            Some(available) => checked_mul(available, value),
            None => Ok(0.0),
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct ChildMeasure {
    cross: f32,
    cross_is_auto: bool,
    cross_leading: f32,
    cross_trailing: f32,
    cross_min: f32,
    cross_max: f32,
    main_leading: f32,
    main_trailing: f32,
}

fn distribute_flex(
    bases: &[f32],
    minimums: &[f32],
    maximums: &[f32],
    grow_factors: &[f32],
    shrink_factors: &[f32],
    free_space: f32,
) -> Result<Vec<f32>, LayoutError> {
    let count = bases.len();

    if minimums.len() != count
        || maximums.len() != count
        || grow_factors.len() != count
        || shrink_factors.len() != count
    {
        return Err(LayoutError::ArithmeticOverflow);
    }

    let mut sizes = bases.to_vec();

    if free_space > 0.0 {
        distribute_grow(&mut sizes, maximums, grow_factors, free_space)?;
    } else if free_space < 0.0 {
        distribute_shrink(&mut sizes, minimums, shrink_factors, -free_space)?;
    }

    if sizes.iter().any(|size| !size.is_finite() || *size < 0.0) {
        return Err(LayoutError::ArithmeticOverflow);
    }

    Ok(sizes)
}

fn distribute_grow(
    sizes: &mut [f32],
    maximums: &[f32],
    factors: &[f32],
    mut remaining: f32,
) -> Result<(), LayoutError> {
    let mut active: Vec<usize> = (0..sizes.len())
        .filter(|&i| factors[i] > 0.0 && sizes[i] < maximums[i])
        .collect();

    while remaining > 0.0 && !active.is_empty() {
        let total_factor = active
            .iter()
            .try_fold(0.0_f32, |sum, &i| checked_add(sum, factors[i]))?;

        if total_factor <= 0.0 {
            break;
        }

        let mut frozen = Vec::new();

        for &i in &active {
            let share = checked_mul(remaining, factors[i] / total_factor)?;
            let capacity = maximums[i] - sizes[i];

            if share >= capacity {
                frozen.push((i, capacity));
            }
        }

        if frozen.is_empty() {
            for &i in &active {
                let share = checked_mul(remaining, factors[i] / total_factor)?;
                sizes[i] = checked_add(sizes[i], share)?;
            }

            remaining = 0.0;
        } else {
            for (index, capacity) in &frozen {
                sizes[*index] = checked_add(sizes[*index], *capacity)?;
                remaining -= *capacity;
            }

            let frozen_indices: Vec<usize> = frozen.iter().map(|(index, _)| *index).collect();

            active.retain(|index| !frozen_indices.contains(index));
        }

        if !remaining.is_finite() {
            return Err(LayoutError::ArithmeticOverflow);
        }
    }

    Ok(())
}

fn distribute_shrink(
    sizes: &mut [f32],
    minimums: &[f32],
    factors: &[f32],
    mut deficit: f32,
) -> Result<(), LayoutError> {
    let bases = sizes.to_vec();

    let mut active: Vec<usize> = (0..sizes.len())
        .filter(|&i| factors[i] > 0.0 && sizes[i] > minimums[i])
        .collect();

    while deficit > 0.0 && !active.is_empty() {
        let total_weight = active.iter().try_fold(0.0_f32, |sum, &i| {
            checked_add(sum, checked_mul(factors[i], bases[i])?)
        })?;

        if total_weight <= 0.0 {
            break;
        }

        let mut frozen = Vec::new();

        for &i in &active {
            let weight = checked_mul(factors[i], bases[i])?;
            let reduction = checked_mul(deficit, weight / total_weight)?;
            let capacity = sizes[i] - minimums[i];

            if reduction >= capacity {
                frozen.push((i, capacity));
            }
        }

        if frozen.is_empty() {
            for &i in &active {
                let weight = checked_mul(factors[i], bases[i])?;
                let reduction = checked_mul(deficit, weight / total_weight)?;
                sizes[i] = (sizes[i] - reduction).max(minimums[i]);
            }

            deficit = 0.0;
        } else {
            for (index, capacity) in &frozen {
                sizes[*index] -= *capacity;
                deficit -= *capacity;
            }

            let frozen_indices: Vec<usize> = frozen.iter().map(|(index, _)| *index).collect();

            active.retain(|index| !frozen_indices.contains(index));
        }

        if !deficit.is_finite() {
            return Err(LayoutError::ArithmeticOverflow);
        }
    }

    Ok(())
}

fn sum_main_sizes(
    sizes: &[f32],
    measurements: &[ChildMeasure],
    gap_total: f32,
) -> Result<f32, LayoutError> {
    let mut total = gap_total;

    for (size, measurement) in sizes.iter().zip(measurements) {
        total = checked_add(total, *size)?;
        total = checked_add(total, measurement.main_leading)?;
        total = checked_add(total, measurement.main_trailing)?;
    }

    Ok(total)
}

fn calculate_main_alignment(
    alignment: MainAlignment,
    remaining: f32,
    child_count: usize,
) -> Result<(f32, f32), LayoutError> {
    if child_count == 0 {
        return Ok((0.0, 0.0));
    }

    let count = child_count as f32;

    let result = match alignment {
        MainAlignment::Start => (0.0, 0.0),
        MainAlignment::Center => (remaining / 2.0, 0.0),
        MainAlignment::End => (remaining, 0.0),
        MainAlignment::SpaceBetween if child_count > 1 => (0.0, remaining / (count - 1.0)),
        MainAlignment::SpaceAround => {
            let each = remaining / count;
            (each / 2.0, each)
        }
        MainAlignment::SpaceEvenly => {
            let each = remaining / (count + 1.0);
            (each, each)
        }
        _ => (0.0, 0.0),
    };

    if !result.0.is_finite() || !result.1.is_finite() {
        return Err(LayoutError::ArithmeticOverflow);
    }

    Ok(result)
}

fn checked_add(a: f32, b: f32) -> Result<f32, LayoutError> {
    let result = a + b;

    if result.is_finite() {
        Ok(result)
    } else {
        Err(LayoutError::ArithmeticOverflow)
    }
}

fn checked_mul(a: f32, b: f32) -> Result<f32, LayoutError> {
    let result = a * b;

    if result.is_finite() {
        Ok(result)
    } else {
        Err(LayoutError::ArithmeticOverflow)
    }
}
