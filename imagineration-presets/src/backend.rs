//! Runtime selection of a [`PresetStore`] backend.

use std::path::PathBuf;
use std::sync::Arc;

use crate::memory::MemoryStore;
use crate::store::{PresetStore, StoreError};

/// Which storage backend to build, with its location.
#[derive(Debug, Clone)]
pub enum StoreBackend {
    /// Non-persistent, in-process storage.
    Memory,
    /// JSON document on disk at `path`.
    File { path: PathBuf },
    /// SQLite database at `path` (`:memory:` for an ephemeral database). Requires the `sqlite`
    /// crate feature.
    Sqlite { path: String },
}

/// Builds the configured [`PresetStore`].
///
/// Returns an error when the `Sqlite` backend is requested but the crate was compiled without the
/// `sqlite` feature.
pub async fn build_store(backend: StoreBackend) -> Result<Arc<dyn PresetStore>, StoreError> {
    match backend {
        StoreBackend::Memory => Ok(Arc::new(MemoryStore::new())),
        StoreBackend::File { path } => {
            let store = crate::file::FileStore::open(path).await?;
            Ok(Arc::new(store))
        }
        StoreBackend::Sqlite { path } => build_sqlite(path).await,
    }
}

#[cfg(feature = "sqlite")]
async fn build_sqlite(path: String) -> Result<Arc<dyn PresetStore>, StoreError> {
    let store = crate::sqlite::SqliteStore::connect(&path).await?;
    Ok(Arc::new(store))
}

#[cfg(not(feature = "sqlite"))]
async fn build_sqlite(_path: String) -> Result<Arc<dyn PresetStore>, StoreError> {
    Err(StoreError::backend(
        "the `sqlite` preset backend requires building imagineration-presets with the `sqlite` feature",
    ))
}
