use noxl_core::geometry::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutDirection {
    #[default]
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainAlignment {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossAlignment {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SizeSpec {
    #[default]
    Auto,
    Px(f32),
    /// Fraction of the matching parent content dimension: 0.0 to 1.0.
    ///
    /// For an auto-sized parent, this dimension does not contribute to the
    /// parent's intrinsic size and resolves after the parent size is known.
    Percent(f32),
}

impl SizeSpec {
    pub fn validate(self) -> Result<(), StyleError> {
        match self {
            Self::Auto => Ok(()),
            Self::Px(value) if value.is_finite() && value >= 0.0 => Ok(()),
            Self::Percent(value) if value.is_finite() && (0.0..=1.0).contains(&value) => Ok(()),
            Self::Px(_) => Err(StyleError::InvalidPixelSize),
            Self::Percent(_) => Err(StyleError::InvalidPercentage),
        }
    }

    pub fn resolve(self, available: f32, intrinsic: f32) -> Result<f32, StyleError> {
        self.validate()?;

        match self {
            Self::Auto => {
                if !intrinsic.is_finite() || intrinsic < 0.0 {
                    return Err(StyleError::InvalidIntrinsicSize);
                }

                Ok(intrinsic)
            }
            Self::Px(value) => Ok(value),
            Self::Percent(value) => {
                if !available.is_finite() || available < 0.0 {
                    return Err(StyleError::InvalidAvailableSize);
                }

                let result = available * value;

                if !result.is_finite() {
                    return Err(StyleError::ArithmeticOverflow);
                }

                Ok(result)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }

    pub fn validate(self) -> Result<(), StyleError> {
        let values = [self.top, self.right, self.bottom, self.left];

        if values.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(StyleError::InvalidEdges);
        }

        if !self.horizontal().is_finite() || !self.vertical().is_finite() {
            return Err(StyleError::ArithmeticOverflow);
        }

        Ok(())
    }
}

impl Default for Edges {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutStyle {
    pub width: SizeSpec,
    pub height: SizeSpec,
    pub min_size: Size,
    pub max_size: Size,
    pub margin: Edges,
    pub padding: Edges,
    pub gap: f32,
    pub direction: LayoutDirection,
    pub justify_content: MainAlignment,
    pub align_items: CrossAlignment,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            width: SizeSpec::Auto,
            height: SizeSpec::Auto,
            min_size: Size::ZERO,
            max_size: Size::new(f32::MAX, f32::MAX),
            margin: Edges::ZERO,
            padding: Edges::ZERO,
            gap: 0.0,
            direction: LayoutDirection::Column,
            justify_content: MainAlignment::Start,
            align_items: CrossAlignment::Stretch,
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }
    }
}

impl LayoutStyle {
    pub fn validate(&self) -> Result<(), StyleError> {
        self.width.validate()?;
        self.height.validate()?;
        self.margin.validate()?;
        self.padding.validate()?;

        if !self.min_size.is_valid() || !self.max_size.is_valid() {
            return Err(StyleError::InvalidSizeRange);
        }

        if self.min_size.width > self.max_size.width || self.min_size.height > self.max_size.height
        {
            return Err(StyleError::InvalidSizeRange);
        }

        if !self.gap.is_finite() || self.gap < 0.0 {
            return Err(StyleError::InvalidGap);
        }

        if !self.flex_grow.is_finite() || self.flex_grow < 0.0 {
            return Err(StyleError::InvalidFlexGrow);
        }

        if !self.flex_shrink.is_finite() || self.flex_shrink < 0.0 {
            return Err(StyleError::InvalidFlexShrink);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleError {
    InvalidPixelSize,
    InvalidPercentage,
    InvalidEdges,
    InvalidSizeRange,
    InvalidGap,
    InvalidFlexGrow,
    InvalidFlexShrink,
    ArithmeticOverflow,
    InvalidAvailableSize,
    InvalidIntrinsicSize,
}

impl std::fmt::Display for StyleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidPixelSize => "pixel size must be finite and non-negative",
            Self::InvalidPercentage => "percentage must be between 0.0 and 1.0",
            Self::InvalidEdges => "edge values must be finite and non-negative",
            Self::InvalidSizeRange => "minimum size must not exceed maximum size",
            Self::InvalidGap => "gap must be finite and non-negative",
            Self::InvalidFlexGrow => "flex grow must be finite and non-negative",
            Self::InvalidFlexShrink => "flex shrink must be finite and non-negative",
            Self::ArithmeticOverflow => "edge arithmetic overflowed",
            Self::InvalidAvailableSize => "available size must be finite and non-negative",
            Self::InvalidIntrinsicSize => "intrinsic size must be finite and non-negative",
        };

        f.write_str(message)
    }
}

impl std::error::Error for StyleError {}
