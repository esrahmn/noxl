use crate::color::Color;
use crate::geometry::{Position, Size};
use crate::id::{IdError, generate_component_id};

/// Unique identifier for every component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentId(pub u64);

/// Defines the type of a UI component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentType {
    Panel,
    Container,
    Text,
    Button,
    Image,
}

/// Errors produced when validating component properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropsError {
    InvalidPosition,
    InvalidWidth,
    InvalidHeight,
    InvalidOpacity,
}

/// Stores visual and behavioral properties of a component.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentProps {
    pub position: Position,
    pub size: Size,
    pub background: Color,
    pub visible: bool,
    pub enabled: bool,
    pub opacity: f32,
    /// Clips the drawing of all descendants to this component's rectangle.
    pub clip_children: bool,
}

impl Default for ComponentProps {
    fn default() -> Self {
        Self {
            position: Position::ZERO,
            size: Size::ZERO,
            background: Color::TRANSPARENT,
            visible: true,
            enabled: true,
            opacity: 1.0,
            clip_children: false,
        }
    }
}

impl ComponentProps {
    /// Validates all component properties.
    pub fn validate(&self) -> Result<(), PropsError> {
        if !self.position.is_finite() {
            return Err(PropsError::InvalidPosition);
        }

        if !self.size.width.is_finite() || self.size.width < 0.0 {
            return Err(PropsError::InvalidWidth);
        }

        if !self.size.height.is_finite() || self.size.height < 0.0 {
            return Err(PropsError::InvalidHeight);
        }

        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(PropsError::InvalidOpacity);
        }

        Ok(())
    }
}

/// Errors produced when building a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentBuildError {
    InvalidProps(PropsError),
    IdExhausted,
}

impl From<IdError> for ComponentBuildError {
    fn from(_: IdError) -> Self {
        Self::IdExhausted
    }
}

impl std::fmt::Display for ComponentBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProps(error) => {
                write!(f, "invalid component properties: {error:?}")
            }
            Self::IdExhausted => {
                write!(f, "component ID space exhausted")
            }
        }
    }
}

impl std::error::Error for ComponentBuildError {}

/// Represents a UI component in the NOXL framework.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub id: ComponentId,
    pub component_type: ComponentType,
    pub props: ComponentProps,
}

impl Component {
    /// Creates a builder for a new component.
    pub fn builder(component_type: ComponentType) -> ComponentBuilder {
        ComponentBuilder::new(component_type)
    }

    /// Validates this component's properties.
    pub fn validate(&self) -> Result<(), PropsError> {
        self.props.validate()
    }
}

/// Builder for creating UI components.
#[derive(Debug, Clone)]
pub struct ComponentBuilder {
    component_type: ComponentType,
    props: ComponentProps,
}

impl ComponentBuilder {
    /// Creates a new component builder.
    pub fn new(component_type: ComponentType) -> Self {
        Self {
            component_type,
            props: ComponentProps::default(),
        }
    }

    /// Sets the component position.
    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.props.position = Position::new(x, y);
        self
    }

    /// Sets the component size.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.props.size = Size::new(width, height);
        self
    }

    /// Sets the component background color.
    pub fn background(mut self, color: Color) -> Self {
        self.props.background = color;
        self
    }

    /// Sets component visibility.
    pub fn visible(mut self, visible: bool) -> Self {
        self.props.visible = visible;
        self
    }

    /// Sets whether the component is enabled.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.props.enabled = enabled;
        self
    }

    /// Sets component opacity between 0.0 and 1.0.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.props.opacity = opacity;
        self
    }

    /// Sets whether descendants are clipped to this component's rectangle.
    pub fn clip_children(mut self, clip: bool) -> Self {
        self.props.clip_children = clip;
        self
    }

    /// Replaces all component properties.
    pub fn props(mut self, props: ComponentProps) -> Self {
        self.props = props;
        self
    }

    /// Builds and validates the component.
    pub fn build(self) -> Result<Component, ComponentBuildError> {
        self.props
            .validate()
            .map_err(ComponentBuildError::InvalidProps)?;

        let id = generate_component_id()?;

        Ok(Component {
            id,
            component_type: self.component_type,
            props: self.props,
        })
    }
}

#[cfg(test)]
mod clip_children_tests {
    use super::*;

    #[test]
    fn clip_children_is_off_by_default() {
        assert!(!ComponentProps::default().clip_children);

        let component = Component::builder(ComponentType::Panel).build().unwrap();

        assert!(!component.props.clip_children);
    }

    #[test]
    fn builder_sets_clip_children() {
        let component = Component::builder(ComponentType::Container)
            .clip_children(true)
            .build()
            .unwrap();

        assert!(component.props.clip_children);
    }
}
