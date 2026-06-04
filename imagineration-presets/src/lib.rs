//! User-defined generation presets: domain types, a backend-agnostic [`PresetStore`], several
//! storage backends (in-memory, JSON file, and an optional SQLite/sea-orm backend), and an axum
//! router exposing per-user CRUD endpoints.
//!
//! The crate is intentionally self-contained: the host application authenticates the request,
//! inserts an [`AuthenticatedUser`] extension, and mounts [`router`] under a prefix of its choice.

mod backend;
mod file;
mod memory;
mod model;
mod router;
mod store;

#[cfg(feature = "sqlite")]
mod sqlite;

pub use backend::{StoreBackend, build_store};
pub use memory::MemoryStore;
pub use model::{LoraSpec, Preset, PresetContent, PresetInput};
pub use router::{AuthenticatedUser, PresetApiError, router};
pub use store::{PresetStore, StoreError};

pub use file::FileStore;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStore;
