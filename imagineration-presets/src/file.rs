//! JSON-file preset backend. The full preset set is persisted to a single JSON document and
//! mirrored in memory for fast reads.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::memory::build_preset;
use crate::model::{Preset, PresetInput};
use crate::store::{PresetStore, StoreError};

/// A file-backed [`PresetStore`] that serializes every preset to a JSON document.
pub struct FileStore {
    path: PathBuf,
    presets: RwLock<HashMap<Uuid, Preset>>,
}

impl FileStore {
    /// Opens (or initializes) a store at `path`, loading any presets already persisted there.
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let path = path.into();
        let presets = load(&path).await?;
        Ok(Self {
            path,
            presets: RwLock::new(presets),
        })
    }

    /// Writes the current in-memory state to disk atomically (temp file + rename).
    async fn persist(&self, presets: &HashMap<Uuid, Preset>) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut records: Vec<&Preset> = presets.values().collect();
        records.sort_by_key(|preset| preset.created_at);
        let json = serde_json::to_vec_pretty(&records)?;

        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &json).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
    }
}

async fn load(path: &Path) -> Result<HashMap<Uuid, Preset>, StoreError> {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let records: Vec<Preset> = serde_json::from_slice(&bytes)?;
            Ok(records
                .into_iter()
                .map(|preset| (preset.id, preset))
                .collect())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(StoreError::Io(error)),
    }
}

#[async_trait]
impl PresetStore for FileStore {
    async fn list(&self, user: &str) -> Result<Vec<Preset>, StoreError> {
        let guard = self.presets.read().await;
        let mut owned: Vec<Preset> = guard
            .values()
            .filter(|preset| preset.user == user)
            .cloned()
            .collect();
        owned.sort_by_key(|preset| std::cmp::Reverse(preset.updated_at));
        Ok(owned)
    }

    async fn get(&self, user: &str, id: Uuid) -> Result<Option<Preset>, StoreError> {
        let guard = self.presets.read().await;
        Ok(guard.get(&id).filter(|preset| preset.user == user).cloned())
    }

    async fn create(&self, user: &str, input: PresetInput) -> Result<Preset, StoreError> {
        let preset = build_preset(user, Uuid::now_v7(), input)?;
        let mut guard = self.presets.write().await;
        guard.insert(preset.id, preset.clone());
        self.persist(&guard).await?;
        Ok(preset)
    }

    async fn update(
        &self,
        user: &str,
        id: Uuid,
        input: PresetInput,
    ) -> Result<Option<Preset>, StoreError> {
        let mut guard = self.presets.write().await;
        let created_at = match guard.get(&id) {
            Some(existing) if existing.user == user => existing.created_at,
            _ => return Ok(None),
        };
        let mut preset = build_preset(user, id, input)?;
        preset.created_at = created_at;
        guard.insert(id, preset.clone());
        self.persist(&guard).await?;
        Ok(Some(preset))
    }

    async fn delete(&self, user: &str, id: Uuid) -> Result<bool, StoreError> {
        let mut guard = self.presets.write().await;
        if !matches!(guard.get(&id), Some(preset) if preset.user == user) {
            return Ok(false);
        }
        guard.remove(&id);
        self.persist(&guard).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PresetContent;

    fn temp_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "imagineration-presets-file-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn input(name: &str) -> PresetInput {
        PresetInput {
            name: name.to_owned(),
            content: PresetContent::default(),
        }
    }

    #[tokio::test]
    async fn persists_across_reopen() {
        let path = temp_path();
        let created = {
            let store = FileStore::open(&path).await.unwrap();
            store.create("alice", input("portrait")).await.unwrap()
        };

        let reopened = FileStore::open(&path).await.unwrap();
        let presets = reopened.list("alice").await.unwrap();
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].id, created.id);
        assert_eq!(presets[0].name, "portrait");

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn missing_file_starts_empty() {
        let path = temp_path();
        let store = FileStore::open(&path).await.unwrap();
        assert!(store.list("alice").await.unwrap().is_empty());
    }
}
