//! In-memory preset backend. Presets live only for the lifetime of the process.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::model::{Preset, PresetInput};
use crate::store::{PresetStore, StoreError};

/// A non-persistent [`PresetStore`] backed by an in-process map.
#[derive(Default)]
pub struct MemoryStore {
    presets: RwLock<HashMap<Uuid, Preset>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PresetStore for MemoryStore {
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
        self.presets.write().await.insert(preset.id, preset.clone());
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
            // Either absent or owned by another user: treat as not found.
            _ => return Ok(None),
        };
        let mut preset = build_preset(user, id, input)?;
        preset.created_at = created_at;
        guard.insert(id, preset.clone());
        Ok(Some(preset))
    }

    async fn delete(&self, user: &str, id: Uuid) -> Result<bool, StoreError> {
        let mut guard = self.presets.write().await;
        if matches!(guard.get(&id), Some(preset) if preset.user == user) {
            guard.remove(&id);
            return Ok(true);
        }
        Ok(false)
    }
}

/// Validates `input` and assembles a [`Preset`] with fresh timestamps.
pub(crate) fn build_preset(user: &str, id: Uuid, input: PresetInput) -> Result<Preset, StoreError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(StoreError::EmptyName);
    }
    let now = Utc::now();
    Ok(Preset {
        id,
        user: user.to_owned(),
        name: name.to_owned(),
        content: input.content,
        created_at: now,
        updated_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PresetContent;

    fn input(name: &str) -> PresetInput {
        PresetInput {
            name: name.to_owned(),
            content: PresetContent {
                prompt: Some("a cat".to_owned()),
                ..PresetContent::default()
            },
        }
    }

    #[tokio::test]
    async fn create_and_list_are_user_scoped() {
        let store = MemoryStore::new();
        let alice = store.create("alice", input("portrait")).await.unwrap();
        store.create("bob", input("landscape")).await.unwrap();

        let alice_presets = store.list("alice").await.unwrap();
        assert_eq!(alice_presets.len(), 1);
        assert_eq!(alice_presets[0].id, alice.id);

        // Bob cannot fetch Alice's preset.
        assert!(store.get("bob", alice.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_preserves_created_at_and_rejects_other_users() {
        let store = MemoryStore::new();
        let created = store.create("alice", input("portrait")).await.unwrap();

        let updated = store
            .update("alice", created.id, input("renamed"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "renamed");
        assert_eq!(updated.created_at, created.created_at);

        assert!(
            store
                .update("bob", created.id, input("hijack"))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn delete_is_user_scoped() {
        let store = MemoryStore::new();
        let created = store.create("alice", input("portrait")).await.unwrap();
        assert!(!store.delete("bob", created.id).await.unwrap());
        assert!(store.delete("alice", created.id).await.unwrap());
        assert!(store.get("alice", created.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn create_rejects_blank_name() {
        let store = MemoryStore::new();
        assert!(matches!(
            store.create("alice", input("   ")).await,
            Err(StoreError::EmptyName)
        ));
    }
}
