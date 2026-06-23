use std::path::PathBuf;

use serde_json::Value;
use sqlx::{Row, SqlitePool};
use tokio::fs;

use crate::error::error::AppError;
use crate::model::ai_book::AiBookMemoryV3;
use crate::service::ai_book_memory_v3::{
    create_empty_ai_book_memory_v3, validate_ai_book_memory_v3,
};
use crate::util::hash::md5_hex;
use crate::util::time::now_ts;

#[derive(Clone)]
pub struct AiBookService {
    pool: SqlitePool,
    storage_dir: PathBuf,
}

enum StoredAiBookMemory {
    Database(String),
    LegacyFile { data: String, path: PathBuf },
}

impl AiBookService {
    pub fn new(pool: SqlitePool, storage_dir: &str) -> Self {
        Self {
            pool,
            storage_dir: PathBuf::from(storage_dir),
        }
    }

    pub async fn get_value(
        &self,
        user_ns: &str,
        book_url: &str,
    ) -> Result<Option<Value>, AppError> {
        let key = md5_hex(book_url);
        if let Some(row) =
            sqlx::query("SELECT json FROM ai_book_memories WHERE user_ns=?1 AND book_key=?2")
                .bind(user_ns)
                .bind(&key)
                .fetch_optional(&self.pool)
                .await?
        {
            let json: String = row.get("json");
            let memory = serde_json::from_str::<Value>(&json)
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            return Ok(Some(memory));
        }

        let path = self.memory_path(user_ns, book_url);
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&path)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let memory = serde_json::from_str::<Value>(&data)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let updated_at = memory
            .get("updatedAt")
            .and_then(Value::as_i64)
            .unwrap_or_else(|| now_ts() * 1000);
        self.save_memory_value_row(user_ns, book_url, &memory, updated_at)
            .await?;
        let _ = fs::remove_file(path).await;
        Ok(Some(memory))
    }

    pub async fn get_or_create_v3(
        &self,
        user_ns: &str,
        book_url: &str,
        book_name: Option<String>,
        author: Option<String>,
    ) -> Result<AiBookMemoryV3, AppError> {
        let book_name = normalize_optional_string(book_name);
        let author = normalize_optional_string(author);
        let Some(stored) = self.load_stored_memory(user_ns, book_url).await? else {
            return self.reset_v3(user_ns, book_url, book_name, author).await;
        };

        let (raw, legacy_path) = match stored {
            StoredAiBookMemory::Database(data) => (data, None),
            StoredAiBookMemory::LegacyFile { data, path } => (data, Some(path)),
        };

        let value = match serde_json::from_str::<Value>(&raw) {
            Ok(value) => value,
            Err(_) => {
                return self
                    .reset_v3_internal(
                        user_ns,
                        book_url,
                        book_name,
                        author,
                        legacy_path.as_ref(),
                    )
                    .await
            }
        };
        if value.get("schemaVersion").and_then(Value::as_i64) != Some(3) {
            return self
                .reset_v3_internal(user_ns, book_url, book_name, author, legacy_path.as_ref())
                .await;
        }

        let mut memory = match serde_json::from_value::<AiBookMemoryV3>(value) {
            Ok(memory) => memory,
            Err(_) => {
                return self
                    .reset_v3_internal(
                        user_ns,
                        book_url,
                        book_name,
                        author,
                        legacy_path.as_ref(),
                    )
                    .await
            }
        };
        if validate_ai_book_memory_v3(&memory).is_err() || memory.book_url != book_url {
            return self
                .reset_v3_internal(user_ns, book_url, book_name, author, legacy_path.as_ref())
                .await;
        }

        let mut should_persist = legacy_path.is_some();
        should_persist |= apply_missing_metadata(&mut memory, book_name.clone(), author.clone());
        if should_persist {
            let saved = self.save_v3(user_ns, book_url, memory).await?;
            self.remove_legacy_file_if_exists(legacy_path.as_ref()).await?;
            return Ok(saved);
        }

        Ok(memory)
    }

    pub async fn save_v3(
        &self,
        user_ns: &str,
        book_url: &str,
        mut memory: AiBookMemoryV3,
    ) -> Result<AiBookMemoryV3, AppError> {
        self.prepare_v3_for_save(book_url, &mut memory)?;
        memory.updated_at = now_ts();
        validate_ai_book_memory_v3(&memory)?;
        let value =
            serde_json::to_value(&memory).map_err(|e| AppError::BadRequest(e.to_string()))?;
        self.save_memory_value_row(user_ns, book_url, &value, memory.updated_at)
            .await?;
        Ok(memory)
    }

    pub async fn save_value_as_v3(
        &self,
        user_ns: &str,
        book_url: &str,
        value: Value,
    ) -> Result<Value, AppError> {
        let memory = serde_json::from_value::<AiBookMemoryV3>(value)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let saved = self.save_v3(user_ns, book_url, memory).await?;
        serde_json::to_value(saved).map_err(|e| AppError::BadRequest(e.to_string()))
    }

    pub async fn reset_v3(
        &self,
        user_ns: &str,
        book_url: &str,
        book_name: Option<String>,
        author: Option<String>,
    ) -> Result<AiBookMemoryV3, AppError> {
        self.reset_v3_internal(user_ns, book_url, book_name, author, None)
            .await
    }

    pub async fn set_enabled(
        &self,
        user_ns: &str,
        book_url: &str,
        enabled: bool,
    ) -> Result<AiBookMemoryV3, AppError> {
        let mut memory = self.get_or_create_v3(user_ns, book_url, None, None).await?;
        memory.enabled = enabled;
        self.save_v3(user_ns, book_url, memory).await
    }

    pub async fn delete(&self, user_ns: &str, book_url: &str) -> Result<bool, AppError> {
        let key = md5_hex(book_url);
        let result = sqlx::query("DELETE FROM ai_book_memories WHERE user_ns=?1 AND book_key=?2")
            .bind(user_ns)
            .bind(&key)
            .execute(&self.pool)
            .await?;

        let path = self.memory_path(user_ns, book_url);
        let removed_file = if path.exists() {
            fs::remove_file(path)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            true
        } else {
            false
        };

        Ok(result.rows_affected() > 0 || removed_file)
    }

    async fn save_memory_value_row(
        &self,
        user_ns: &str,
        book_url: &str,
        memory: &Value,
        updated_at: i64,
    ) -> Result<(), AppError> {
        let key = md5_hex(book_url);
        let data =
            serde_json::to_string(memory).map_err(|e| AppError::BadRequest(e.to_string()))?;
        sqlx::query(
            "INSERT INTO ai_book_memories (user_ns, book_key, book_url, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(user_ns, book_key) DO UPDATE SET book_url=excluded.book_url, json=excluded.json, updated_at=excluded.updated_at",
        )
        .bind(user_ns)
        .bind(&key)
        .bind(book_url)
        .bind(data)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    fn memory_path(&self, user_ns: &str, book_url: &str) -> PathBuf {
        self.storage_dir
            .join("data")
            .join(user_ns)
            .join("ai-books")
            .join(format!("{}.json", md5_hex(book_url)))
    }

    async fn load_stored_memory(
        &self,
        user_ns: &str,
        book_url: &str,
    ) -> Result<Option<StoredAiBookMemory>, AppError> {
        let key = md5_hex(book_url);
        if let Some(row) =
            sqlx::query("SELECT json FROM ai_book_memories WHERE user_ns=?1 AND book_key=?2")
                .bind(user_ns)
                .bind(&key)
                .fetch_optional(&self.pool)
                .await?
        {
            let json: String = row.get("json");
            return Ok(Some(StoredAiBookMemory::Database(json)));
        }

        let path = self.memory_path(user_ns, book_url);
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&path)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        Ok(Some(StoredAiBookMemory::LegacyFile { data, path }))
    }

    async fn reset_v3_internal(
        &self,
        user_ns: &str,
        book_url: &str,
        book_name: Option<String>,
        author: Option<String>,
        legacy_path: Option<&PathBuf>,
    ) -> Result<AiBookMemoryV3, AppError> {
        let memory = create_empty_ai_book_memory_v3(book_url, book_name, author);
        let saved = self.save_v3(user_ns, book_url, memory).await?;
        self.remove_legacy_file_if_exists(legacy_path).await?;
        Ok(saved)
    }

    async fn remove_legacy_file_if_exists(
        &self,
        legacy_path: Option<&PathBuf>,
    ) -> Result<(), AppError> {
        let Some(path) = legacy_path else {
            return Ok(());
        };
        if path.exists() {
            fs::remove_file(path)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
        }
        Ok(())
    }

    fn prepare_v3_for_save(
        &self,
        book_url: &str,
        memory: &mut AiBookMemoryV3,
    ) -> Result<(), AppError> {
        if book_url.trim().is_empty() {
            return Err(AppError::BadRequest("bookUrl required".to_string()));
        }
        if memory.book_url.trim().is_empty() {
            memory.book_url = book_url.to_string();
        } else if memory.book_url != book_url {
            return Err(AppError::BadRequest("bookUrl mismatch".to_string()));
        }
        memory.book_name = normalize_optional_string(memory.book_name.take());
        memory.author = normalize_optional_string(memory.author.take());
        Ok(())
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn apply_missing_metadata(
    memory: &mut AiBookMemoryV3,
    book_name: Option<String>,
    author: Option<String>,
) -> bool {
    let mut changed = false;
    if memory.book_name.is_none() && book_name.is_some() {
        memory.book_name = book_name;
        changed = true;
    }
    if memory.author.is_none() && author.is_some() {
        memory.author = author;
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ai_book_memory_v3::{
        create_empty_ai_book_memory_v3, select_ai_book_display_memory_v3,
    };
    use crate::storage::db;
    use crate::util::crypto::random_string;
    use serde_json::json;
    use tokio::fs;

    async fn create_service() -> (AiBookService, PathBuf) {
        let dir = std::env::temp_dir().join(format!("reader-ai-book-service-{}", random_string(8)));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        (AiBookService::new(pool, dir.to_str().unwrap()), dir)
    }

    async fn load_memory_json(
        service: &AiBookService,
        user_ns: &str,
        book_url: &str,
    ) -> serde_json::Value {
        let key = md5_hex(book_url);
        let row =
            sqlx::query("SELECT json FROM ai_book_memories WHERE user_ns=?1 AND book_key=?2")
                .bind(user_ns)
                .bind(&key)
                .fetch_one(&service.pool)
                .await
                .unwrap();
        let json: String = row.get("json");
        serde_json::from_str(&json).unwrap()
    }

    #[tokio::test]
    async fn ai_book_v3_resets_invalid_or_non_v3_without_backup() {
        let (service, dir) = create_service().await;
        let user_ns = "reader1";

        sqlx::query(
            "INSERT INTO ai_book_memories (user_ns, book_key, book_url, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(user_ns)
        .bind(md5_hex("book://legacy"))
        .bind("book://legacy")
        .bind(json!({
            "schemaVersion": 2,
            "bookUrl": "book://legacy",
            "summary": "legacy"
        }).to_string())
        .bind(now_ts())
        .execute(&service.pool)
        .await
        .unwrap();

        let legacy = service
            .get_or_create_v3(
                user_ns,
                "book://legacy",
                Some("旧书".to_string()),
                Some("旧作者".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(legacy.schema_version, 3);
        assert_eq!(legacy.book_url, "book://legacy");
        assert_eq!(legacy.book_name.as_deref(), Some("旧书"));
        assert_eq!(legacy.author.as_deref(), Some("旧作者"));
        assert!(legacy.summary.current.is_empty());
        assert!(!legacy.enabled);

        let legacy_json = load_memory_json(&service, user_ns, "book://legacy").await;
        assert_eq!(legacy_json["schemaVersion"], json!(3));
        assert_eq!(legacy_json["bookUrl"], json!("book://legacy"));
        assert_eq!(legacy_json["bookName"], json!("旧书"));
        assert_eq!(legacy_json["author"], json!("旧作者"));
        assert!(service.memory_path(user_ns, "book://legacy").exists() == false);

        sqlx::query(
            "INSERT INTO ai_book_memories (user_ns, book_key, book_url, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(user_ns, book_key) DO UPDATE SET json=excluded.json, updated_at=excluded.updated_at",
        )
        .bind(user_ns)
        .bind(md5_hex("book://invalid"))
        .bind("book://invalid")
        .bind("{not-json")
        .bind(now_ts())
        .execute(&service.pool)
        .await
        .unwrap();

        let invalid = service
            .get_or_create_v3(user_ns, "book://invalid", None, None)
            .await
            .unwrap();
        assert_eq!(invalid.schema_version, 3);
        assert_eq!(invalid.book_url, "book://invalid");
        assert!(invalid.summary.current.is_empty());

        let invalid_json = load_memory_json(&service, user_ns, "book://invalid").await;
        assert_eq!(invalid_json["schemaVersion"], json!(3));
        assert_eq!(invalid_json["bookUrl"], json!("book://invalid"));
        assert!(service.memory_path(user_ns, "book://invalid").exists() == false);

        for (book_url, stored_json) in [
            (
                "book://missing-url",
                json!({
                    "schemaVersion": 3,
                    "summary": { "current": "旧摘要" },
                    "knowledgeFacts": [{
                        "title": "规则",
                        "content": "旧内容"
                    }]
                }),
            ),
            (
                "book://blank-url",
                json!({
                    "schemaVersion": 3,
                    "bookUrl": "   ",
                    "summary": { "current": "旧摘要" },
                    "characters": [{
                        "name": "张羽"
                    }]
                }),
            ),
        ] {
            sqlx::query(
                "INSERT INTO ai_book_memories (user_ns, book_key, book_url, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(user_ns, book_key) DO UPDATE SET json=excluded.json, updated_at=excluded.updated_at",
            )
            .bind(user_ns)
            .bind(md5_hex(book_url))
            .bind(book_url)
            .bind(stored_json.to_string())
            .bind(now_ts())
            .execute(&service.pool)
            .await
            .unwrap();

            let reset = service
                .get_or_create_v3(user_ns, book_url, None, None)
                .await
                .unwrap();
            assert_eq!(reset.schema_version, 3);
            assert_eq!(reset.book_url, book_url);
            assert!(reset.summary.current.is_empty());
            assert!(reset.knowledge_facts.is_empty());
            assert!(reset.characters.is_empty());

            let reset_json = load_memory_json(&service, user_ns, book_url).await;
            assert_eq!(reset_json["schemaVersion"], json!(3));
            assert_eq!(reset_json["bookUrl"], json!(book_url));
            assert_eq!(reset_json["summary"]["current"], json!(""));
            assert_eq!(reset_json["knowledgeFacts"], json!([]));
            assert_eq!(reset_json["characters"], json!([]));
        }

        let _ = fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn ai_book_v3_rejects_non_v3_save() {
        let (service, dir) = create_service().await;
        let mut memory = create_empty_ai_book_memory_v3("book://bad", None, None);
        memory.schema_version = 2;

        let err = service
            .save_v3("reader1", "book://bad", memory)
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("unsupported ai book schema version")
                || err.to_string().contains("invalid type")
        );

        let row = sqlx::query("SELECT COUNT(*) AS count FROM ai_book_memories")
            .fetch_one(&service.pool)
            .await
            .unwrap();
        let count: i64 = row.get("count");
        assert_eq!(count, 0);

        let _ = fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn ai_book_v3_typed_value_save_round_trips_v3_only() {
        let (service, dir) = create_service().await;
        let user_ns = "reader1";
        let book_url = "book://typed-value";
        let mut memory = create_empty_ai_book_memory_v3(
            book_url,
            Some("类型化".to_string()),
            Some("作者".to_string()),
        );
        memory.summary.current = "已保存".to_string();

        let saved = service
            .save_value_as_v3(user_ns, book_url, serde_json::to_value(memory.clone()).unwrap())
            .await
            .unwrap();
        let saved_memory: AiBookMemoryV3 = serde_json::from_value(saved).unwrap();
        assert_eq!(saved_memory.schema_version, 3);
        assert_eq!(saved_memory.summary.current, "已保存");

        let mut bad = serde_json::to_value(memory).unwrap();
        bad["schemaVersion"] = json!(2);
        let err = service
            .save_value_as_v3(user_ns, book_url, bad)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unsupported ai book schema version")
                || err.to_string().contains("invalid type")
        );

        let _ = fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn ai_book_v3_get_memory_always_returns_renderable_view() {
        let (service, dir) = create_service().await;
        let user_ns = "reader1";

        sqlx::query(
            "INSERT INTO ai_book_memories (user_ns, book_key, book_url, json, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(user_ns)
        .bind(md5_hex("book://broken"))
        .bind("book://broken")
        .bind(json!({
            "schemaVersion": 3,
            "bookUrl": "book://broken",
            "characters": [{ "name": "" }]
        }).to_string())
        .bind(now_ts())
        .execute(&service.pool)
        .await
        .unwrap();

        let memory = service
            .get_or_create_v3(
                user_ns,
                "book://broken",
                Some("可渲染".to_string()),
                Some("作者".to_string()),
            )
            .await
            .unwrap();
        let view = select_ai_book_display_memory_v3(&memory);

        assert_eq!(view.book_url, "book://broken");
        assert_eq!(view.book_name.as_deref(), Some("可渲染"));
        assert_eq!(view.author.as_deref(), Some("作者"));
        assert!(view.summary.current.is_empty());
        assert!(view.characters.is_empty());
        assert!(view.relationships.is_empty());
        assert!(view.knowledge_facts.is_empty());
        assert!(view.locations.is_empty());
        assert!(view.map.is_none());
        assert_eq!(view.cleanup.dropped_facts_count, 0);

        let _ = fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn ai_book_v3_set_enabled_only_toggles_enabled() {
        let (service, dir) = create_service().await;
        let user_ns = "reader1";
        let mut memory = create_empty_ai_book_memory_v3(
            "book://toggle",
            Some("切换书".to_string()),
            Some("作者".to_string()),
        );
        memory.summary.current = "已有摘要".to_string();
        memory.characters.push(crate::model::ai_book::AiBookCharacterV3 {
            name: "张羽".to_string(),
            ..Default::default()
        });

        service.save_v3(user_ns, "book://toggle", memory).await.unwrap();

        let enabled = service
            .set_enabled(user_ns, "book://toggle", true)
            .await
            .unwrap();
        assert!(enabled.enabled);
        assert_eq!(enabled.summary.current, "已有摘要");
        assert_eq!(enabled.characters.len(), 1);
        assert_eq!(enabled.book_name.as_deref(), Some("切换书"));

        let loaded = service
            .get_or_create_v3(user_ns, "book://toggle", None, None)
            .await
            .unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.summary.current, "已有摘要");
        assert_eq!(loaded.characters.len(), 1);
        assert_eq!(loaded.book_name.as_deref(), Some("切换书"));

        let disabled = service
            .set_enabled(user_ns, "book://toggle", false)
            .await
            .unwrap();
        assert!(!disabled.enabled);
        assert_eq!(disabled.summary.current, "已有摘要");
        assert_eq!(disabled.characters.len(), 1);

        let stored = load_memory_json(&service, user_ns, "book://toggle").await;
        assert_eq!(stored["enabled"], json!(false));
        assert_eq!(stored["summary"]["current"], json!("已有摘要"));
        assert_eq!(stored["bookName"], json!("切换书"));
        assert_eq!(stored["author"], json!("作者"));
        assert_eq!(stored["characters"][0]["name"], json!("张羽"));

        let _ = fs::remove_dir_all(dir).await;
    }
}
