use crate::geometry::{Position, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Start,
    Center,
    End,
}

impl Alignment {
    /// Calculates a non-negative alignment offset.
    ///
    /// If the element is larger than the available space, it starts at zero
    /// instead of producing a negative offset.
    pub fn calculate_offset(&self, container_size: f32, element_size: f32) -> f32 {
        let remaining = (container_size - element_size).max(0.0);

        match self {
            Self::Start => 0.0,
            Self::Center => remaining / 2.0,
            Self::End => remaining,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutConfig {
    pub direction: LayoutDirection,
    pub spacing: f32,
    pub padding: Padding,
    pub alignment: Alignment,
}

impl LayoutConfig {
    pub fn new(
        direction: LayoutDirection,
        spacing: f32,
        padding: Padding,
        alignment: Alignment,
    ) -> Self {
        Self {
            direction,
            spacing,
            padding,
            alignment,
        }
    }

    /// Calculates the position of each element inside a container.
    ///
    /// Elements are arranged along the configured direction.
    /// Alignment is applied on the cross axis.
    pub fn calculate_layout(
        &self,
        container_size: Size,
        element_sizes: &[Size],
    ) -> Result<Vec<Position>, LayoutError> {
        self.validate()?;
        Self::validate_container_size(container_size)?;
        Self::validate_element_sizes(element_sizes)?;

        let content_size = self.padding.apply(container_size);

        let start = Position::new(self.padding.left, self.padding.top);

        let layout = Layout::new(self.direction, self.spacing);

        let mut positions = layout.try_calculate_positions(element_sizes, start)?;

        for (position, size) in positions.iter_mut().zip(element_sizes) {
            let offset = match self.direction {
                LayoutDirection::Horizontal => self
                    .alignment
                    .calculate_offset(content_size.height, size.height),
                LayoutDirection::Vertical => self
                    .alignment
                    .calculate_offset(content_size.width, size.width),
            };

            match self.direction {
                LayoutDirection::Horizontal => position.y += offset,
                LayoutDirection::Vertical => position.x += offset,
            }

            if !position.is_finite() {
                return Err(LayoutError::ArithmeticOverflow);
            }
        }

        Ok(positions)
    }

    /// Calculates layout after applying per-element size constraints.
    pub fn calculate_layout_with_constraints(
        &self,
        container_size: Size,
        element_sizes: &[Size],
        constraints: &[Constraints],
    ) -> Result<Vec<Position>, LayoutError> {
        if element_sizes.len() != constraints.len() {
            return Err(LayoutError::ConstraintCountMismatch);
        }

        let constrained_sizes: Result<Vec<Size>, ConstraintError> = element_sizes
            .iter()
            .zip(constraints.iter())
            .map(|(size, constraint)| constraint.constrain(*size))
            .collect();

        self.calculate_layout(
            container_size,
            &constrained_sizes.map_err(LayoutError::InvalidElementSize)?,
        )
    }

    pub fn validate(&self) -> Result<(), LayoutError> {
        if !self.spacing.is_finite() || self.spacing < 0.0 {
            return Err(LayoutError::InvalidSpacing);
        }

        self.padding
            .validate()
            .map_err(LayoutError::InvalidPadding)?;

        Ok(())
    }

    fn validate_container_size(size: Size) -> Result<(), LayoutError> {
        if !size.width.is_finite() || size.width < 0.0 {
            return Err(LayoutError::InvalidContainerWidth);
        }

        if !size.height.is_finite() || size.height < 0.0 {
            return Err(LayoutError::InvalidContainerHeight);
        }

        Ok(())
    }

    fn validate_element_sizes(sizes: &[Size]) -> Result<(), LayoutError> {
        for size in sizes {
            if !size.width.is_finite() || size.width < 0.0 {
                return Err(LayoutError::InvalidElementSize(
                    ConstraintError::InvalidWidth,
                ));
            }

            if !size.height.is_finite() || size.height < 0.0 {
                return Err(LayoutError::InvalidElementSize(
                    ConstraintError::InvalidHeight,
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    ConstraintCountMismatch,
    InvalidElementSize(ConstraintError),
    InvalidPadding(PaddingError),
    InvalidSpacing,
    InvalidContainerWidth,
    InvalidContainerHeight,
    InvalidStartPosition,
    ArithmeticOverflow,
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConstraintCountMismatch => {
                write!(f, "element and constraint counts do not match")
            }
            Self::InvalidElementSize(error) => {
                write!(f, "invalid element size: {error}")
            }
            Self::InvalidPadding(error) => {
                write!(f, "invalid padding: {error}")
            }
            Self::InvalidSpacing => write!(f, "spacing must be finite and non-negative"),
            Self::InvalidContainerWidth => {
                write!(f, "container width must be finite and non-negative")
            }
            Self::InvalidContainerHeight => {
                write!(f, "container height must be finite and non-negative")
            }
            Self::InvalidStartPosition => {
                write!(f, "start position must contain finite coordinates")
            }
            Self::ArithmeticOverflow => {
                write!(f, "layout calculation produced an invalid result")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaddingError {
    InvalidTop,
    InvalidRight,
    InvalidBottom,
    InvalidLeft,
}

impl std::fmt::Display for PaddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTop => write!(f, "top padding must be finite and non-negative"),
            Self::InvalidRight => write!(f, "right padding must be finite and non-negative"),
            Self::InvalidBottom => write!(f, "bottom padding must be finite and non-negative"),
            Self::InvalidLeft => write!(f, "left padding must be finite and non-negative"),
        }
    }
}

impl std::error::Error for PaddingError {}

impl Default for Padding {
    fn default() -> Self {
        Self::all(0.0)
    }
}

impl Padding {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Returns the remaining content area after padding.
    ///
    /// The resulting dimensions are clamped to zero.
    pub fn apply(&self, size: Size) -> Size {
        Size::new(
            (size.width - self.left - self.right).max(0.0),
            (size.height - self.top - self.bottom).max(0.0),
        )
    }

    pub fn validate(&self) -> Result<(), PaddingError> {
        if !self.top.is_finite() || self.top < 0.0 {
            return Err(PaddingError::InvalidTop);
        }

        if !self.right.is_finite() || self.right < 0.0 {
            return Err(PaddingError::InvalidRight);
        }

        if !self.bottom.is_finite() || self.bottom < 0.0 {
            return Err(PaddingError::InvalidBottom);
        }

        if !self.left.is_finite() || self.left < 0.0 {
            return Err(PaddingError::InvalidLeft);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintError {
    InvalidWidthRange,
    InvalidHeightRange,
    InvalidWidth,
    InvalidHeight,
}

impl std::fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidWidthRange => write!(f, "invalid width constraint range"),
            Self::InvalidHeightRange => write!(f, "invalid height constraint range"),
            Self::InvalidWidth => write!(f, "width must be finite and non-negative"),
            Self::InvalidHeight => write!(f, "height must be finite and non-negative"),
        }
    }
}

impl std::error::Error for ConstraintError {}

impl Constraints {
    pub fn new(
        min_width: f32,
        max_width: f32,
        min_height: f32,
        max_height: f32,
    ) -> Result<Self, ConstraintError> {
        if !min_width.is_finite()
            || !max_width.is_finite()
            || min_width < 0.0
            || min_width > max_width
        {
            return Err(ConstraintError::InvalidWidthRange);
        }

        if !min_height.is_finite()
            || !max_height.is_finite()
            || min_height < 0.0
            || min_height > max_height
        {
            return Err(ConstraintError::InvalidHeightRange);
        }

        Ok(Self {
            min_width,
            max_width,
            min_height,
            max_height,
        })
    }

    /// Creates constraints that allow any finite non-negative size.
    pub const fn unbounded() -> Self {
        Self {
            min_width: 0.0,
            max_width: f32::MAX,
            min_height: 0.0,
            max_height: f32::MAX,
        }
    }

    pub fn constrain(&self, size: Size) -> Result<Size, ConstraintError> {
        if !size.width.is_finite() || size.width < 0.0 {
            return Err(ConstraintError::InvalidWidth);
        }

        if !size.height.is_finite() || size.height < 0.0 {
            return Err(ConstraintError::InvalidHeight);
        }

        Ok(Size::new(
            size.width.clamp(self.min_width, self.max_width),
            size.height.clamp(self.min_height, self.max_height),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub direction: LayoutDirection,
    pub spacing: f32,
}

impl Layout {
    pub fn new(direction: LayoutDirection, spacing: f32) -> Self {
        Self { direction, spacing }
    }

    /// Calculates positions without validation.
    ///
    /// Prefer `try_calculate_positions` when input may be untrusted.
    pub fn calculate_positions(&self, sizes: &[Size], start: Position) -> Vec<Position> {
        let mut positions = Vec::with_capacity(sizes.len());
        let mut offset = 0.0;

        for size in sizes {
            let position = match self.direction {
                LayoutDirection::Horizontal => Position::new(start.x + offset, start.y),
                LayoutDirection::Vertical => Position::new(start.x, start.y + offset),
            };

            positions.push(position);

            offset += match self.direction {
                LayoutDirection::Horizontal => size.width + self.spacing,
                LayoutDirection::Vertical => size.height + self.spacing,
            };
        }

        positions
    }

    /// Validates inputs and rejects arithmetic overflow.
    pub fn try_calculate_positions(
        &self,
        sizes: &[Size],
        start: Position,
    ) -> Result<Vec<Position>, LayoutError> {
        if !self.spacing.is_finite() || self.spacing < 0.0 {
            return Err(LayoutError::InvalidSpacing);
        }

        if !start.is_finite() {
            return Err(LayoutError::InvalidStartPosition);
        }

        for size in sizes {
            if !size.width.is_finite() || size.width < 0.0 {
                return Err(LayoutError::InvalidElementSize(
                    ConstraintError::InvalidWidth,
                ));
            }

            if !size.height.is_finite() || size.height < 0.0 {
                return Err(LayoutError::InvalidElementSize(
                    ConstraintError::InvalidHeight,
                ));
            }
        }

        let mut positions = Vec::with_capacity(sizes.len());
        let mut offset = 0.0_f32;

        for (index, size) in sizes.iter().enumerate() {
            let position = match self.direction {
                LayoutDirection::Horizontal => Position::new(start.x + offset, start.y),
                LayoutDirection::Vertical => Position::new(start.x, start.y + offset),
            };

            if !position.is_finite() {
                return Err(LayoutError::ArithmeticOverflow);
            }

            positions.push(position);

            if index + 1 < sizes.len() {
                let main_axis_size = match self.direction {
                    LayoutDirection::Horizontal => size.width,
                    LayoutDirection::Vertical => size.height,
                };

                offset += main_axis_size + self.spacing;

                if !offset.is_finite() {
                    return Err(LayoutError::ArithmeticOverflow);
                }
            }
        }

        Ok(positions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn horizontal_config() -> LayoutConfig {
        LayoutConfig::new(
            LayoutDirection::Horizontal,
            10.0,
            Padding::all(10.0),
            Alignment::Center,
        )
    }

    #[test]
    fn horizontal_layout_calculates_positions() {
        let layout = Layout::new(LayoutDirection::Horizontal, 10.0);
        let sizes = [Size::new(100.0, 50.0), Size::new(150.0, 50.0)];

        let positions = layout
            .try_calculate_positions(&sizes, Position::ZERO)
            .unwrap();

        assert_eq!(positions[0], Position::new(0.0, 0.0));
        assert_eq!(positions[1], Position::new(110.0, 0.0));
    }

    #[test]
    fn vertical_layout_calculates_positions() {
        let layout = Layout::new(LayoutDirection::Vertical, 10.0);
        let sizes = [Size::new(100.0, 40.0), Size::new(100.0, 60.0)];

        let positions = layout
            .try_calculate_positions(&sizes, Position::ZERO)
            .unwrap();

        assert_eq!(positions[0], Position::new(0.0, 0.0));
        assert_eq!(positions[1], Position::new(0.0, 50.0));
    }

    #[test]
    fn padding_all_applies_to_every_side() {
        assert_eq!(
            Padding::all(10.0).apply(Size::new(300.0, 200.0)),
            Size::new(280.0, 180.0)
        );
    }

    #[test]
    fn padding_symmetric_applies_correctly() {
        assert_eq!(
            Padding::symmetric(20.0, 10.0).apply(Size::new(300.0, 200.0)),
            Size::new(260.0, 180.0)
        );
    }

    #[test]
    fn padding_larger_than_container_clamps_to_zero() {
        assert_eq!(
            Padding::all(100.0).apply(Size::new(100.0, 50.0)),
            Size::ZERO
        );
    }

    #[test]
    fn alignment_calculates_offsets() {
        assert_eq!(Alignment::Start.calculate_offset(300.0, 100.0), 0.0);
        assert_eq!(Alignment::Center.calculate_offset(300.0, 100.0), 100.0);
        assert_eq!(Alignment::End.calculate_offset(300.0, 100.0), 200.0);
    }

    #[test]
    fn alignment_does_not_produce_negative_offsets() {
        assert_eq!(Alignment::Center.calculate_offset(50.0, 100.0), 0.0);
        assert_eq!(Alignment::End.calculate_offset(50.0, 100.0), 0.0);
    }

    #[test]
    fn layout_config_combines_padding_spacing_and_alignment() {
        let config = horizontal_config();
        let sizes = [Size::new(100.0, 40.0), Size::new(150.0, 80.0)];

        let positions = config
            .calculate_layout(Size::new(400.0, 120.0), &sizes)
            .unwrap();

        assert_eq!(positions[0], Position::new(10.0, 40.0));
        assert_eq!(positions[1], Position::new(120.0, 20.0));
    }

    #[test]
    fn constraints_increase_size_below_minimum() {
        let constraints = Constraints::new(100.0, 300.0, 50.0, 150.0).unwrap();

        assert_eq!(
            constraints.constrain(Size::new(50.0, 20.0)).unwrap(),
            Size::new(100.0, 50.0)
        );
    }

    #[test]
    fn constraints_reduce_size_above_maximum() {
        let constraints = Constraints::new(100.0, 300.0, 50.0, 150.0).unwrap();

        assert_eq!(
            constraints.constrain(Size::new(400.0, 200.0)).unwrap(),
            Size::new(300.0, 150.0)
        );
    }

    #[test]
    fn constraints_preserve_valid_size() {
        let constraints = Constraints::new(100.0, 300.0, 50.0, 150.0).unwrap();

        assert_eq!(
            constraints.constrain(Size::new(200.0, 100.0)).unwrap(),
            Size::new(200.0, 100.0)
        );
    }

    #[test]
    fn unbounded_constraints_accept_large_finite_sizes() {
        let result = Constraints::unbounded()
            .constrain(Size::new(100_000.0, 200_000.0))
            .unwrap();

        assert_eq!(result, Size::new(100_000.0, 200_000.0));
    }

    #[test]
    fn constraints_reject_invalid_width_range() {
        assert_eq!(
            Constraints::new(300.0, 100.0, 50.0, 150.0),
            Err(ConstraintError::InvalidWidthRange)
        );
    }

    #[test]
    fn constraints_reject_invalid_height_range() {
        assert_eq!(
            Constraints::new(100.0, 300.0, 150.0, 50.0),
            Err(ConstraintError::InvalidHeightRange)
        );
    }

    #[test]
    fn constraints_reject_nan_width() {
        assert_eq!(
            Constraints::new(f32::NAN, 300.0, 50.0, 150.0),
            Err(ConstraintError::InvalidWidthRange)
        );
    }

    #[test]
    fn constraints_reject_infinite_height() {
        assert_eq!(
            Constraints::new(100.0, 300.0, 50.0, f32::INFINITY),
            Err(ConstraintError::InvalidHeightRange)
        );
    }

    #[test]
    fn constraints_reject_nan_input_width() {
        let constraints = Constraints::unbounded();

        assert_eq!(
            constraints.constrain(Size::new(f32::NAN, 100.0)),
            Err(ConstraintError::InvalidWidth)
        );
    }

    #[test]
    fn constraints_reject_infinite_input_height() {
        let constraints = Constraints::unbounded();

        assert_eq!(
            constraints.constrain(Size::new(100.0, f32::INFINITY)),
            Err(ConstraintError::InvalidHeight)
        );
    }

    #[test]
    fn layout_with_constraints_applies_size_limits() {
        let config = horizontal_config();
        let sizes = [Size::new(50.0, 20.0), Size::new(400.0, 100.0)];

        let constraints = [
            Constraints::new(100.0, 200.0, 40.0, 80.0).unwrap(),
            Constraints::new(100.0, 300.0, 40.0, 80.0).unwrap(),
        ];

        let result = config
            .calculate_layout_with_constraints(Size::new(400.0, 120.0), &sizes, &constraints)
            .unwrap();

        assert_eq!(result[0], Position::new(10.0, 40.0));
        assert_eq!(result[1], Position::new(120.0, 20.0));
    }

    #[test]
    fn layout_with_constraints_rejects_mismatched_counts() {
        let config = horizontal_config();
        let sizes = [Size::new(100.0, 40.0)];
        let constraints = [];

        assert_eq!(
            config
                .calculate_layout_with_constraints(Size::new(400.0, 120.0), &sizes, &constraints,),
            Err(LayoutError::ConstraintCountMismatch)
        );
    }

    #[test]
    fn layout_accepts_empty_elements() {
        assert_eq!(
            horizontal_config().calculate_layout(Size::new(400.0, 120.0), &[]),
            Ok(vec![])
        );
    }

    #[test]
    fn layout_rejects_invalid_padding() {
        let config = LayoutConfig::new(
            LayoutDirection::Horizontal,
            10.0,
            Padding {
                top: -10.0,
                right: 10.0,
                bottom: 10.0,
                left: 10.0,
            },
            Alignment::Center,
        );

        assert_eq!(
            config.calculate_layout(Size::new(400.0, 120.0), &[]),
            Err(LayoutError::InvalidPadding(PaddingError::InvalidTop))
        );
    }

    #[test]
    fn layout_rejects_invalid_spacing() {
        let config = LayoutConfig::new(
            LayoutDirection::Horizontal,
            f32::NAN,
            Padding::default(),
            Alignment::Start,
        );

        assert_eq!(
            config.calculate_layout(Size::new(400.0, 120.0), &[]),
            Err(LayoutError::InvalidSpacing)
        );
    }

    #[test]
    fn layout_rejects_invalid_container_width() {
        assert_eq!(
            horizontal_config().calculate_layout(Size::new(-1.0, 120.0), &[]),
            Err(LayoutError::InvalidContainerWidth)
        );
    }

    #[test]
    fn layout_rejects_invalid_container_height() {
        assert_eq!(
            horizontal_config().calculate_layout(Size::new(400.0, f32::NAN), &[]),
            Err(LayoutError::InvalidContainerHeight)
        );
    }

    #[test]
    fn layout_rejects_invalid_element_size() {
        let sizes = [Size::new(-100.0, 40.0)];

        assert_eq!(
            horizontal_config().calculate_layout(Size::new(400.0, 120.0), &sizes),
            Err(LayoutError::InvalidElementSize(
                ConstraintError::InvalidWidth
            ))
        );
    }

    #[test]
    fn layout_rejects_element_height_nan() {
        let sizes = [Size::new(100.0, f32::NAN)];

        assert_eq!(
            horizontal_config().calculate_layout(Size::new(400.0, 120.0), &sizes),
            Err(LayoutError::InvalidElementSize(
                ConstraintError::InvalidHeight
            ))
        );
    }

    #[test]
    fn layout_rejects_position_overflow() {
        let layout = Layout::new(LayoutDirection::Horizontal, f32::MAX);
        let sizes = [Size::new(f32::MAX, 10.0), Size::new(10.0, 10.0)];

        assert_eq!(
            layout.try_calculate_positions(&sizes, Position::ZERO),
            Err(LayoutError::ArithmeticOverflow)
        );
    }

    #[test]
    fn layout_rejects_invalid_start_position() {
        let layout = Layout::new(LayoutDirection::Horizontal, 10.0);

        assert_eq!(
            layout.try_calculate_positions(&[], Position::new(f32::INFINITY, 0.0)),
            Err(LayoutError::InvalidStartPosition)
        );
    }

    #[test]
    fn layout_rejects_negative_spacing() {
        let layout = Layout::new(LayoutDirection::Horizontal, -1.0);

        assert_eq!(
            layout.try_calculate_positions(&[], Position::ZERO),
            Err(LayoutError::InvalidSpacing)
        );
    }

    #[test]
    fn padding_rejects_negative_value() {
        let padding = Padding {
            top: -10.0,
            right: 10.0,
            bottom: 10.0,
            left: 10.0,
        };

        assert_eq!(padding.validate(), Err(PaddingError::InvalidTop));
    }

    #[test]
    fn padding_rejects_infinite_value() {
        let padding = Padding {
            top: 10.0,
            right: f32::INFINITY,
            bottom: 10.0,
            left: 10.0,
        };

        assert_eq!(padding.validate(), Err(PaddingError::InvalidRight));
    }
}
