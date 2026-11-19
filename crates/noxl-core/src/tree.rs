use std::collections::HashSet;
use std::fmt;

use crate::component::{Component, ComponentId, PropsError};

/// Errors produced by component tree operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeError {
    DuplicateId(ComponentId),
    InvalidComponentProps { id: ComponentId, error: PropsError },
    InvalidChildIndex { index: usize, child_count: usize },
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => {
                write!(f, "duplicate component ID: {}", id.0)
            }
            Self::InvalidComponentProps { id, error } => {
                write!(f, "invalid properties for component {}: {:?}", id.0, error)
            }
            Self::InvalidChildIndex { index, child_count } => {
                write!(
                    f,
                    "child index {index} is out of bounds for {child_count} children"
                )
            }
        }
    }
}

impl std::error::Error for TreeError {}

/// A node in the component hierarchy.
///
/// Children are stored in insertion order.
#[derive(Debug, Clone)]
pub struct ComponentNode {
    pub component: Component,
    pub children: Vec<ComponentNode>,
}

impl ComponentNode {
    /// Creates a new node without children.
    pub fn new(component: Component) -> Self {
        Self {
            component,
            children: Vec::new(),
        }
    }

    // Child Management

    /// Adds a child to the end of this node's children.
    ///
    /// The current tree and incoming subtree are validated before mutation.
    pub fn add_child(&mut self, child: ComponentNode) -> Result<(), TreeError> {
        let mut ids = HashSet::new();

        self.collect_ids(&mut ids)?;
        child.collect_ids(&mut ids)?;

        self.children.push(child);

        Ok(())
    }

    /// Inserts a child at the specified index.
    ///
    /// Valid indices are `0..=child_count`.
    pub fn insert_child(&mut self, index: usize, child: ComponentNode) -> Result<(), TreeError> {
        if index > self.children.len() {
            return Err(TreeError::InvalidChildIndex {
                index,
                child_count: self.children.len(),
            });
        }

        let mut ids = HashSet::new();

        self.collect_ids(&mut ids)?;
        child.collect_ids(&mut ids)?;

        self.children.insert(index, child);

        Ok(())
    }

    /// Removes a direct or nested descendant by component ID.
    ///
    /// This method does not remove the current node itself.
    pub fn remove(&mut self, id: ComponentId) -> Option<ComponentNode> {
        if let Some(index) = self
            .children
            .iter()
            .position(|child| child.component.id == id)
        {
            return Some(self.children.remove(index));
        }

        for child in &mut self.children {
            if let Some(removed) = child.remove(id) {
                return Some(removed);
            }
        }

        None
    }

    /// Removes all direct children and returns them.
    pub fn clear_children(&mut self) -> Vec<ComponentNode> {
        std::mem::take(&mut self.children)
    }

    /// Replaces a direct child identified by ID.
    ///
    /// The replacement subtree is validated before mutation.
    /// Returns the old child, or `None` if the ID was not found.
    pub fn replace_child(
        &mut self,
        id: ComponentId,
        replacement: ComponentNode,
    ) -> Result<Option<ComponentNode>, TreeError> {
        let Some(index) = self
            .children
            .iter()
            .position(|child| child.component.id == id)
        else {
            return Ok(None);
        };

        let mut ids = HashSet::new();
        ids.insert(self.component.id);

        for (child_index, child) in self.children.iter().enumerate() {
            if child_index != index {
                child.collect_ids(&mut ids)?;
            }
        }

        replacement.collect_ids(&mut ids)?;

        let old_child = std::mem::replace(&mut self.children[index], replacement);

        Ok(Some(old_child))
    }

    // Search

    /// Finds a node by ID, including the current node.
    pub fn find(&self, id: ComponentId) -> Option<&ComponentNode> {
        if self.component.id == id {
            return Some(self);
        }

        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }

        None
    }

    /// Finds a mutable node by ID, including the current node.
    pub fn find_mut(&mut self, id: ComponentId) -> Option<&mut ComponentNode> {
        if self.component.id == id {
            return Some(self);
        }

        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }

        None
    }

    /// Checks whether a component ID exists in this tree.
    pub fn contains(&self, id: ComponentId) -> bool {
        self.find(id).is_some()
    }

    // Validation

    /// Validates component properties and ID uniqueness across the tree.
    pub fn validate(&self) -> Result<(), TreeError> {
        let mut ids = HashSet::new();
        self.collect_ids(&mut ids)
    }

    fn collect_ids(&self, ids: &mut HashSet<ComponentId>) -> Result<(), TreeError> {
        let id = self.component.id;

        if !ids.insert(id) {
            return Err(TreeError::DuplicateId(id));
        }

        self.component
            .validate()
            .map_err(|error| TreeError::InvalidComponentProps { id, error })?;

        for child in &self.children {
            child.collect_ids(ids)?;
        }

        Ok(())
    }

    // Traversal

    /// Visits every node in pre-order.
    ///
    /// The current node is visited before its children.
    pub fn traverse<F>(&self, mut visitor: F)
    where
        F: FnMut(&ComponentNode),
    {
        self.traverse_inner(&mut visitor);
    }

    fn traverse_inner<F>(&self, visitor: &mut F)
    where
        F: FnMut(&ComponentNode),
    {
        visitor(self);

        for child in &self.children {
            child.traverse_inner(visitor);
        }
    }

    /// Mutably visits every node in pre-order.
    pub fn traverse_mut<F>(&mut self, mut visitor: F)
    where
        F: FnMut(&mut ComponentNode),
    {
        self.traverse_mut_inner(&mut visitor);
    }

    fn traverse_mut_inner<F>(&mut self, visitor: &mut F)
    where
        F: FnMut(&mut ComponentNode),
    {
        visitor(self);

        for child in &mut self.children {
            child.traverse_mut_inner(visitor);
        }
    }

    // Statistics

    /// Returns the total number of nodes, including this node.
    pub fn node_count(&self) -> usize {
        self.children.iter().fold(1usize, |count, child| {
            count.saturating_add(child.node_count())
        })
    }

    /// Returns the number of direct children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Returns the tree depth.
    ///
    /// A node without children has a depth of 1.
    pub fn depth(&self) -> usize {
        let child_depth = self
            .children
            .iter()
            .map(ComponentNode::depth)
            .max()
            .unwrap_or(0);

        1usize.saturating_add(child_depth)
    }

    /// Returns true if this node has no children.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::color::Color;
    use crate::component::{ComponentProps, ComponentType};
    use crate::geometry::{Position, Size};

    fn create_test_component(id: u64) -> Component {
        Component {
            id: ComponentId(id),
            component_type: ComponentType::Container,
            props: ComponentProps {
                position: Position::ZERO,
                size: Size::new(1600.0, 40.0),
                background: Color::BLACK,
                ..ComponentProps::default()
            },
        }
    }

    fn create_test_node(id: u64) -> ComponentNode {
        ComponentNode::new(create_test_component(id))
    }

    #[test]
    fn node_starts_without_children() {
        let node = create_test_node(1);

        assert!(node.children.is_empty());
        assert!(node.is_leaf());
        assert_eq!(node.child_count(), 0);
        assert_eq!(node.node_count(), 1);
        assert_eq!(node.depth(), 1);
    }

    #[test]
    fn add_child_accepts_unique_id() {
        let mut root = create_test_node(1);

        assert!(root.add_child(create_test_node(2)).is_ok());
        assert_eq!(root.child_count(), 1);
    }

    #[test]
    fn add_child_rejects_duplicate_id() {
        let mut root = create_test_node(1);

        assert_eq!(
            root.add_child(create_test_node(1)),
            Err(TreeError::DuplicateId(ComponentId(1)))
        );

        assert!(root.children.is_empty());
    }

    #[test]
    fn add_child_rejects_duplicate_id_inside_subtree() {
        let mut root = create_test_node(1);
        let mut child = create_test_node(2);

        child.children.push(create_test_node(2));

        assert_eq!(
            root.add_child(child),
            Err(TreeError::DuplicateId(ComponentId(2)))
        );

        assert!(root.children.is_empty());
    }

    #[test]
    fn add_child_rejects_duplicate_against_existing_subtree() {
        let mut root = create_test_node(1);
        let mut child = create_test_node(2);

        child.add_child(create_test_node(3)).unwrap();
        root.add_child(child).unwrap();

        assert_eq!(
            root.add_child(create_test_node(3)),
            Err(TreeError::DuplicateId(ComponentId(3)))
        );
    }

    #[test]
    fn insert_child_places_node_at_requested_index() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();
        root.add_child(create_test_node(4)).unwrap();
        root.insert_child(1, create_test_node(3)).unwrap();

        let ids: Vec<_> = root.children.iter().map(|node| node.component.id).collect();

        assert_eq!(ids, vec![ComponentId(2), ComponentId(3), ComponentId(4)]);
    }

    #[test]
    fn insert_child_rejects_invalid_index() {
        let mut root = create_test_node(1);

        assert_eq!(
            root.insert_child(1, create_test_node(2)),
            Err(TreeError::InvalidChildIndex {
                index: 1,
                child_count: 0,
            })
        );

        assert!(root.children.is_empty());
    }

    #[test]
    fn insert_child_rejects_duplicate_id() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();

        assert_eq!(
            root.insert_child(0, create_test_node(2)),
            Err(TreeError::DuplicateId(ComponentId(2)))
        );
    }

    #[test]
    fn validate_accepts_unique_tree() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();

        assert_eq!(root.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_duplicate_id() {
        let mut root = create_test_node(1);

        root.children.push(create_test_node(1));

        assert_eq!(root.validate(), Err(TreeError::DuplicateId(ComponentId(1))));
    }

    #[test]
    fn validate_rejects_invalid_component_props() {
        let mut root = create_test_node(1);

        root.component.props.opacity = 2.0;

        assert_eq!(
            root.validate(),
            Err(TreeError::InvalidComponentProps {
                id: ComponentId(1),
                error: PropsError::InvalidOpacity,
            })
        );
    }

    #[test]
    fn find_searches_nested_components() {
        let mut root = create_test_node(1);
        let mut child = create_test_node(2);

        child.add_child(create_test_node(3)).unwrap();
        root.add_child(child).unwrap();

        assert!(root.find(ComponentId(3)).is_some());
        assert!(root.contains(ComponentId(3)));
    }

    #[test]
    fn find_includes_current_node() {
        let root = create_test_node(1);

        assert!(root.find(ComponentId(1)).is_some());
    }

    #[test]
    fn find_returns_none_for_missing_id() {
        let root = create_test_node(1);

        assert!(root.find(ComponentId(99)).is_none());
    }

    #[test]
    fn find_mut_updates_nested_component() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();

        let node = root.find_mut(ComponentId(2)).unwrap();
        node.component.props.background = Color::WHITE;

        assert_eq!(
            root.find(ComponentId(2))
                .unwrap()
                .component
                .props
                .background,
            Color::WHITE
        );
    }

    #[test]
    fn remove_deletes_nested_component() {
        let mut root = create_test_node(1);
        let mut child = create_test_node(2);

        child.add_child(create_test_node(3)).unwrap();
        root.add_child(child).unwrap();

        assert!(root.remove(ComponentId(3)).is_some());
        assert!(!root.contains(ComponentId(3)));
    }

    #[test]
    fn remove_does_not_remove_root() {
        let mut root = create_test_node(1);

        assert!(root.remove(ComponentId(1)).is_none());
        assert!(root.contains(ComponentId(1)));
    }

    #[test]
    fn clear_children_returns_removed_nodes() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();
        root.add_child(create_test_node(3)).unwrap();

        let removed = root.clear_children();

        assert_eq!(removed.len(), 2);
        assert!(root.children.is_empty());
        assert_eq!(root.node_count(), 1);
    }

    #[test]
    fn replace_child_replaces_direct_child() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();

        let old = root
            .replace_child(ComponentId(2), create_test_node(3))
            .unwrap()
            .unwrap();

        assert_eq!(old.component.id, ComponentId(2));
        assert!(root.contains(ComponentId(3)));
        assert!(!root.contains(ComponentId(2)));
    }

    #[test]
    fn replace_child_returns_none_for_missing_id() {
        let mut root = create_test_node(1);

        let result = root
            .replace_child(ComponentId(99), create_test_node(2))
            .unwrap();

        assert!(result.is_none());
        assert!(root.children.is_empty());
    }

    #[test]
    fn replace_child_rejects_duplicate_id() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();
        root.add_child(create_test_node(3)).unwrap();

        assert!(matches!(
            root.replace_child(ComponentId(2), create_test_node(3)),
            Err(TreeError::DuplicateId(ComponentId(3)))
        ));

        assert!(root.contains(ComponentId(2)));
    }

    #[test]
    fn replace_child_rejects_duplicate_inside_replacement() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();

        let mut replacement = create_test_node(3);
        replacement.children.push(create_test_node(3));

        assert!(matches!(
            root.replace_child(ComponentId(2), replacement),
            Err(TreeError::DuplicateId(ComponentId(3)))
        ));

        assert!(root.contains(ComponentId(2)));
    }

    #[test]
    fn traverse_visits_parent_before_children() {
        let mut root = create_test_node(1);
        let mut child = create_test_node(2);

        child.add_child(create_test_node(4)).unwrap();

        root.add_child(child).unwrap();
        root.add_child(create_test_node(3)).unwrap();

        let mut visited = Vec::new();

        root.traverse(|node| visited.push(node.component.id));

        assert_eq!(
            visited,
            vec![
                ComponentId(1),
                ComponentId(2),
                ComponentId(4),
                ComponentId(3),
            ]
        );
    }

    #[test]
    fn traverse_mut_updates_all_components() {
        let mut root = create_test_node(1);

        root.add_child(create_test_node(2)).unwrap();
        root.add_child(create_test_node(3)).unwrap();

        root.traverse_mut(|node| {
            node.component.props.background = Color::WHITE;
        });

        root.traverse(|node| {
            assert_eq!(node.component.props.background, Color::WHITE);
        });
    }

    #[test]
    fn statistics_calculate_tree_structure() {
        let mut root = create_test_node(1);
        let mut child = create_test_node(2);

        child.add_child(create_test_node(3)).unwrap();
        root.add_child(child).unwrap();
        root.add_child(create_test_node(4)).unwrap();

        assert_eq!(root.node_count(), 4);
        assert_eq!(root.child_count(), 2);
        assert_eq!(root.depth(), 3);
        assert!(!root.is_leaf());
    }
}
