use std::path::PathBuf;

use serde_json::Value;
use sqlx::{Row, SqlitePool};
use tokio::fs;

use crate::error::error::AppError;
use crate::model::ai_book::AiBookMemory;
use crate::util::hash::md5_hex;
use crate::util::time::now_ts;

const MAX_AI_BOOK_ARRAY_ITEMS: usize = 500;

#[derive(Clone)]
pub struct AiBookService {
    pool: SqlitePool,
    storage_dir: PathBuf,
}

impl AiBookService {
    pub fn new(pool: SqlitePool, storage_dir: &str) -> Self {
        Self {
            pool,
            storage_dir: PathBuf::from(storage_dir),
        }
    }

    pub async fn get(
        &self,
        user_ns: &str,
        book_url: &str,
    ) -> Result<Option<AiBookMemory>, AppError> {
        let Some(value) = self.get_value(user_ns, book_url).await? else {
            return Ok(None);
        };
        serde_json::from_value::<AiBookMemory>(value)
            .map(Some)
            .map_err(|e| AppError::BadRequest(e.to_string()))
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

    pub async fn save_for_book(
        &self,
        user_ns: &str,
        book_url: &str,
        memory: AiBookMemory,
    ) -> Result<AiBookMemory, AppError> {
        let value =
            serde_json::to_value(memory).map_err(|e| AppError::BadRequest(e.to_string()))?;
        let saved = self.save_value_for_book(user_ns, book_url, value).await?;
        serde_json::from_value::<AiBookMemory>(saved)
            .map_err(|e| AppError::BadRequest(e.to_string()))
    }

    pub async fn save_value_for_book(
        &self,
        user_ns: &str,
        book_url: &str,
        mut memory: Value,
    ) -> Result<Value, AppError> {
        if book_url.trim().is_empty() {
            return Err(AppError::BadRequest("bookUrl required".to_string()));
        }

        {
            let object = memory.as_object_mut().ok_or_else(|| {
                AppError::BadRequest("AI memory must be a JSON object".to_string())
            })?;
            let memory_book_url = object
                .get("bookUrl")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if memory_book_url.is_empty() {
                object.insert("bookUrl".to_string(), Value::String(book_url.to_string()));
            } else if memory_book_url != book_url {
                return Err(AppError::BadRequest("bookUrl mismatch".to_string()));
            }
        }

        validate_ai_book_memory_value(&memory)?;

        let object = memory
            .as_object_mut()
            .ok_or_else(|| AppError::BadRequest("AI memory must be a JSON object".to_string()))?;
        let updated_at = object.get("updatedAt").and_then(Value::as_i64).unwrap_or(0);
        let updated_at = if updated_at > 0 {
            updated_at
        } else {
            let now = now_ts() * 1000;
            object.insert("updatedAt".to_string(), Value::Number(now.into()));
            now
        };

        self.save_memory_value_row(user_ns, book_url, &memory, updated_at)
            .await?;
        Ok(memory)
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
}

fn validate_ai_book_memory_value(memory: &Value) -> Result<(), AppError> {
    let object = memory
        .as_object()
        .ok_or_else(|| AppError::BadRequest("AI memory must be a JSON object".to_string()))?;

    if let Some(schema_version) = object.get("schemaVersion").filter(|value| !value.is_null()) {
        match schema_version.as_i64() {
            Some(1 | 2) => {}
            _ => {
                return Err(AppError::BadRequest(
                    "schemaVersion must be 1 or 2".to_string(),
                ));
            }
        }
    }

    for key in [
        "worldview",
        "worldFacts",
        "characters",
        "relationships",
        "locations",
        "chapterDigests",
        "arcs",
    ] {
        if let Some(length) = object.get(key).and_then(Value::as_array).map(Vec::len) {
            if length > MAX_AI_BOOK_ARRAY_ITEMS {
                return Err(AppError::BadRequest(format!(
                    "{key} exceeds {MAX_AI_BOOK_ARRAY_ITEMS} items"
                )));
            }
        }
    }

    if claims_processed_chapter(object)
        && !has_ai_book_semantic_content(object)
        && !has_non_empty_string(object.get("lastError"))
    {
        return Err(AppError::BadRequest("AI memory is empty".to_string()));
    }

    Ok(())
}

fn claims_processed_chapter(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("processedChapterIndex")
        .and_then(Value::as_i64)
        .is_some()
        || has_non_empty_string(object.get("processedChapterTitle"))
}

fn has_ai_book_semantic_content(object: &serde_json::Map<String, Value>) -> bool {
    if has_non_empty_string(object.get("summary")) {
        return true;
    }
    if let Some(summary) = object.get("summary").and_then(Value::as_object) {
        if has_non_empty_string(summary.get("current"))
            || has_non_empty_array(summary.get("recentChanges"))
            || has_non_empty_array(summary.get("openQuestions"))
        {
            return true;
        }
    }
    [
        "worldview",
        "worldFacts",
        "characters",
        "relationships",
        "locations",
    ]
    .iter()
    .any(|key| has_non_empty_array(object.get(*key)))
}

fn has_non_empty_array(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

fn has_non_empty_string(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
}
