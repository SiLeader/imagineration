//! The backend-agnostic preset storage interface.

use async_trait::async_trait;
use uuid::Uuid;

use crate::model::{Preset, PresetInput};

/// Storage operations for user-defined presets.
///
/// Every operation is scoped to a `user` (the authenticated subject) so that presets created by
/// one user are never visible to another. Backends must enforce this scoping themselves.
#[async_trait]
pub trait PresetStore: Send + Sync {
    /// Lists all presets owned by `user`, most recently updated first.
    async fn list(&self, user: &str) -> Result<Vec<Preset>, StoreError>;

    /// Fetches a single preset owned by `user`, or `None` when it does not exist.
    async fn get(&self, user: &str, id: Uuid) -> Result<Option<Preset>, StoreError>;

    /// Creates a new preset owned by `user`.
    async fn create(&self, user: &str, input: PresetInput) -> Result<Preset, StoreError>;

    /// Replaces an existing preset owned by `user`, returning `None` when it does not exist.
    async fn update(
        &self,
        user: &str,
        id: Uuid,
        input: PresetInput,
    ) -> Result<Option<Preset>, StoreError>;

    /// Deletes a preset owned by `user`, returning whether a preset was removed.
    async fn delete(&self, user: &str, id: Uuid) -> Result<bool, StoreError>;
}

/// Errors surfaced by a [`PresetStore`] backend.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("preset name must not be empty")]
    EmptyName,
    #[error("storage I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to (de)serialize preset data: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("preset storage backend error: {0}")]
    Backend(String),
}

impl StoreError {
    pub(crate) fn backend(message: impl Into<String>) -> Self {
        StoreError::Backend(message.into())
    }
}
