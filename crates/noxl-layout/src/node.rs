use std::collections::HashSet;

use crate::style::{LayoutStyle, StyleError};
use noxl_core::component::ComponentId;

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutNode {
    pub id: ComponentId,
    pub style: LayoutStyle,
    pub children: Vec<LayoutNode>,
}

impl LayoutNode {
    pub fn new(id: ComponentId) -> Self {
        Self {
            id,
            style: LayoutStyle::default(),
            children: Vec::new(),
        }
    }

    pub fn with_style(mut self, style: LayoutStyle) -> Self {
        self.style = style;
        self
    }

    pub fn add_child(&mut self, child: LayoutNode) {
        self.children.push(child);
    }

    pub fn with_child(mut self, child: LayoutNode) -> Self {
        self.add_child(child);
        self
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn validate(&self) -> Result<(), NodeError> {
        self.validate_recursive(&mut HashSet::new())
    }

    fn validate_recursive(&self, ids: &mut HashSet<ComponentId>) -> Result<(), NodeError> {
        if !ids.insert(self.id) {
            return Err(NodeError::DuplicateId { id: self.id });
        }

        self.style
            .validate()
            .map_err(|error| NodeError::InvalidStyle { id: self.id, error })?;

        for child in &self.children {
            child.validate_recursive(ids)?;
        }

        Ok(())
    }

    pub fn find(&self, id: ComponentId) -> Option<&LayoutNode> {
        if self.id == id {
            return Some(self);
        }

        self.children.iter().find_map(|child| child.find(id))
    }

    pub fn find_mut(&mut self, id: ComponentId) -> Option<&mut LayoutNode> {
        if self.id == id {
            return Some(self);
        }

        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }

    pub fn node_count(&self) -> usize {
        self.children.iter().fold(1usize, |count, child| {
            count.saturating_add(child.node_count())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeError {
    DuplicateId { id: ComponentId },
    InvalidStyle { id: ComponentId, error: StyleError },
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId { id } => {
                write!(f, "duplicate component ID in layout tree: {id:?}")
            }
            Self::InvalidStyle { id, error } => {
                write!(f, "invalid layout style on component {id:?}: {error}")
            }
        }
    }
}

impl std::error::Error for NodeError {}
