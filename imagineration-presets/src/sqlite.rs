//! SQLite (sea-orm) preset backend.
//!
//! Timestamps and ids are stored as TEXT (RFC 3339 / UUID strings) and the preset body as a JSON
//! TEXT column, which keeps the sea-orm feature surface small and the schema portable to other
//! relational backends.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, Schema,
};
use uuid::Uuid;

use crate::memory::build_preset;
use crate::model::{Preset, PresetInput};
use crate::store::{PresetStore, StoreError};

mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "presets")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        #[sea_orm(indexed)]
        pub owner: String,
        pub name: String,
        pub content: String,
        pub created_at: String,
        pub updated_at: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use entity::{ActiveModel, Column, Entity};

/// A [`PresetStore`] backed by a SQLite database via sea-orm.
pub struct SqliteStore {
    db: DatabaseConnection,
}

impl SqliteStore {
    /// Connects to (and, when missing, creates) a SQLite database at `path`, ensuring the schema
    /// exists. Pass `":memory:"` for an ephemeral database.
    pub async fn connect(path: &str) -> Result<Self, StoreError> {
        let url = connection_url(path);
        let db = Database::connect(&url)
            .await
            .map_err(|error| StoreError::backend(error.to_string()))?;
        ensure_schema(&db).await?;
        Ok(Self { db })
    }
}

fn connection_url(path: &str) -> String {
    if path == ":memory:" || path.starts_with("sqlite:") {
        // Already a DSN, or the in-memory sentinel.
        if path == ":memory:" {
            "sqlite::memory:".to_owned()
        } else {
            path.to_owned()
        }
    } else {
        // `mode=rwc` opens read-write and creates the file if it is absent.
        format!("sqlite://{path}?mode=rwc")
    }
}

async fn ensure_schema(db: &DatabaseConnection) -> Result<(), StoreError> {
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    let mut statement = schema.create_table_from_entity(Entity);
    statement.if_not_exists();
    db.execute(backend.build(&statement))
        .await
        .map_err(|error| StoreError::backend(error.to_string()))?;
    Ok(())
}

fn to_preset(model: entity::Model) -> Result<Preset, StoreError> {
    Ok(Preset {
        id: Uuid::parse_str(&model.id).map_err(|error| StoreError::backend(error.to_string()))?,
        user: model.owner,
        name: model.name,
        content: serde_json::from_str(&model.content)?,
        created_at: parse_time(&model.created_at)?,
        updated_at: parse_time(&model.updated_at)?,
    })
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| StoreError::backend(error.to_string()))
}

fn active_model(preset: &Preset) -> Result<ActiveModel, StoreError> {
    Ok(ActiveModel {
        id: Set(preset.id.to_string()),
        owner: Set(preset.user.clone()),
        name: Set(preset.name.clone()),
        content: Set(serde_json::to_string(&preset.content)?),
        created_at: Set(preset.created_at.to_rfc3339()),
        updated_at: Set(preset.updated_at.to_rfc3339()),
    })
}

#[async_trait]
impl PresetStore for SqliteStore {
    async fn list(&self, user: &str) -> Result<Vec<Preset>, StoreError> {
        let rows = Entity::find()
            .filter(Column::Owner.eq(user))
            .order_by_desc(Column::UpdatedAt)
            .all(&self.db)
            .await
            .map_err(|error| StoreError::backend(error.to_string()))?;
        rows.into_iter().map(to_preset).collect()
    }

    async fn get(&self, user: &str, id: Uuid) -> Result<Option<Preset>, StoreError> {
        let row = Entity::find_by_id(id.to_string())
            .filter(Column::Owner.eq(user))
            .one(&self.db)
            .await
            .map_err(|error| StoreError::backend(error.to_string()))?;
        row.map(to_preset).transpose()
    }

    async fn create(&self, user: &str, input: PresetInput) -> Result<Preset, StoreError> {
        let preset = build_preset(user, Uuid::now_v7(), input)?;
        active_model(&preset)?
            .insert(&self.db)
            .await
            .map_err(|error| StoreError::backend(error.to_string()))?;
        Ok(preset)
    }

    async fn update(
        &self,
        user: &str,
        id: Uuid,
        input: PresetInput,
    ) -> Result<Option<Preset>, StoreError> {
        let Some(existing) = self.get(user, id).await? else {
            return Ok(None);
        };
        let mut preset = build_preset(user, id, input)?;
        preset.created_at = existing.created_at;
        active_model(&preset)?
            .update(&self.db)
            .await
            .map_err(|error| StoreError::backend(error.to_string()))?;
        Ok(Some(preset))
    }

    async fn delete(&self, user: &str, id: Uuid) -> Result<bool, StoreError> {
        let result = Entity::delete_many()
            .filter(Column::Id.eq(id.to_string()))
            .filter(Column::Owner.eq(user))
            .exec(&self.db)
            .await
            .map_err(|error| StoreError::backend(error.to_string()))?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PresetContent, PresetInput};

    fn input(name: &str, prompt: &str) -> PresetInput {
        PresetInput {
            name: name.to_owned(),
            content: PresetContent {
                prompt: Some(prompt.to_owned()),
                steps: Some(24),
                ..PresetContent::default()
            },
        }
    }

    #[tokio::test]
    async fn crud_round_trip_in_memory() {
        let store = SqliteStore::connect(":memory:").await.unwrap();
        let created = store
            .create("alice", input("portrait", "a cat"))
            .await
            .unwrap();

        let fetched = store.get("alice", created.id).await.unwrap().unwrap();
        assert_eq!(fetched.content.prompt.as_deref(), Some("a cat"));
        assert_eq!(fetched.content.steps, Some(24));

        let updated = store
            .update("alice", created.id, input("portrait", "a dog"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.content.prompt.as_deref(), Some("a dog"));
        assert_eq!(updated.created_at, created.created_at);

        assert!(!store.delete("bob", created.id).await.unwrap());
        assert!(store.delete("alice", created.id).await.unwrap());
        assert!(store.list("alice").await.unwrap().is_empty());
    }
}
