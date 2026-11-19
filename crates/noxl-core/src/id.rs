use std::sync::atomic::{AtomicU64, Ordering};

use crate::component::ComponentId;

/// Errors produced by the component ID generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    Exhausted,
}

impl std::fmt::Display for IdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exhausted => write!(f, "component ID space exhausted"),
        }
    }
}

impl std::error::Error for IdError {}

/// Zero is reserved as the exhausted state.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Generates a globally unique component ID.
pub fn generate_component_id() -> Result<ComponentId, IdError> {
    NEXT_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            if current == 0 {
                None
            } else {
                Some(current.wrapping_add(1))
            }
        })
        .map(ComponentId)
        .map_err(|_| IdError::Exhausted)
}
