use std::sync::Arc;

use md5::{Digest, Md5};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::error::AppError;
use crate::model::ai_model::{AiModelConfig, AiModelKind};
use crate::model::ai_proxy::build_ai_proxy_url;
use crate::model::chapter_summary::{ChapterSummaryConfig, ChapterSummaryRecord, GenerateChapterSummaryRequest};
use crate::service::json_document_service::JsonDocumentService;
use crate::util::time::now_ts;

const APP_NAMESPACE: &str = "__app__";
const CONFIG_NAME: &str = "chapter-summary-config.json";
const SUMMARY_PREFIX: &str = "chapter-summary";

#[derive(Clone)]
pub struct ChapterSummaryService {
    docs: Arc<JsonDocumentService>,
}

impl ChapterSummaryService {
    pub fn new(docs: Arc<JsonDocumentService>) -> Self {
        Self { docs }
    }

    pub async fn get_config(&self) -> Result<ChapterSummaryConfig, AppError> {
        if let Some(value) = self.docs.get_value(APP_NAMESPACE, CONFIG_NAME).await? {
            return serde_json::from_value::<ChapterSummaryConfig>(value)
                .map(|config| config.sanitized())
                .map_err(|e| AppError::BadRequest(e.to_string()));
        }
        Ok(ChapterSummaryConfig::default().sanitized())
    }

    pub async fn save_config(&self, config: ChapterSummaryConfig) -> Result<ChapterSummaryConfig, AppError> {
        let config = config.sanitized();
        self.docs.set_value(APP_NAMESPACE, CONFIG_NAME, &config).await?;
        Ok(config)
    }

    pub async fn get_summary(
        &self,
        user_ns: &str,
        book_url: &str,
        chapter_url: &str,
    ) -> Result<Option<ChapterSummaryRecord>, AppError> {
        let name = summary_name(book_url, chapter_url);
        let Some(value) = self.docs.get_value(user_ns, &name).await? else {
            return Ok(None);
        };
        serde_json::from_value::<ChapterSummaryRecord>(value)
            .map(Some)
            .map_err(|e| AppError::BadRequest(e.to_string()))
    }

    pub async fn save_summary(
        &self,
        user_ns: &str,
        record: ChapterSummaryRecord,
    ) -> Result<ChapterSummaryRecord, AppError> {
        let name = summary_name(&record.book_url, &record.chapter_url);
        self.docs.set_value(user_ns, &name, &record).await?;
        Ok(record)
    }

    pub fn validate_generation_input(
        &self,
        config: &ChapterSummaryConfig,
        content: &str,
    ) -> Result<(), AppError> {
        if !config.enabled {
            return Err(AppError::BadRequest("本章摘要功能未启用".to_string()));
        }
        if content.trim().is_empty() {
            return Err(AppError::BadRequest("正文内容为空".to_string()));
        }
        if content.chars().count() < config.min_content_chars {
            return Err(AppError::BadRequest("正文内容不足，未达到生成摘要的最短长度".to_string()));
        }
        Ok(())
    }

    pub async fn get_cached_if_allowed(
        &self,
        user_ns: &str,
        book_url: &str,
        chapter_url: &str,
        force: bool,
    ) -> Result<Option<ChapterSummaryRecord>, AppError> {
        if force {
            return Ok(None);
        }
        self.get_summary(user_ns, book_url, chapter_url).await
    }

    pub async fn generate_summary(
        &self,
        user_ns: &str,
        req: GenerateChapterSummaryRequest,
        ai_config: AiModelConfig,
        client: &Client,
    ) -> Result<ChapterSummaryRecord, AppError> {
        let config = self.get_config().await?;
        self.validate_generation_input(&config, &req.content)?;

        if let Some(cached) = self
            .get_cached_if_allowed(user_ns, &req.book_url, &req.chapter_url, req.force)
            .await?
        {
            return Ok(cached);
        }

        let endpoint = ai_config.resolve(AiModelKind::Text);
        if !endpoint.enabled || endpoint.base_url.trim().is_empty() || endpoint.model.trim().is_empty() {
            return Err(AppError::BadRequest("后端文本模型未启用或配置不完整".to_string()));
        }

        let path = if endpoint.path.trim().is_empty() {
            "/v1/chat/completions"
        } else {
            endpoint.path.trim()
        };
        let target = build_ai_proxy_url(&endpoint.base_url, path, endpoint.use_full_url)
            .map_err(AppError::BadRequest)?;
        let body = build_summary_model_body(path, &endpoint.model, &config, &req);

        let mut builder = client
            .post(target)
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&body);
        if !endpoint.api_key.trim().is_empty() {
            builder = builder.bearer_auth(endpoint.api_key.trim());
        }
        let response = builder.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AppError::BadRequest(format!(
                "摘要模型请求失败: {} {}",
                status,
                text.chars().take(200).collect::<String>()
            )));
        }

        let value: Value = response.json().await?;
        let content = extract_model_content(path, &value)?;
        let parsed = parse_summary_payload(&content)?;
        let now = now_ts();
        let old = self.get_summary(user_ns, &req.book_url, &req.chapter_url).await?;
        let created_at = old.as_ref().map(|v| v.created_at).unwrap_or(now);
        let record = ChapterSummaryRecord {
            book_url: req.book_url,
            chapter_url: req.chapter_url,
            chapter_index: req.chapter_index,
            chapter_title: req.chapter_title,
            summary: parsed.summary,
            key_points: parsed.key_points,
            questions: parsed.questions,
            prompt_version: "default-v1".to_string(),
            model: endpoint.model,
            created_at,
            updated_at: now,
        };
        self.save_summary(user_ns, record).await
    }
}

fn summary_name(book_url: &str, chapter_url: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(book_url.as_bytes());
    hasher.update(b"\n");
    hasher.update(chapter_url.as_bytes());
    format!("{}-{:x}.json", SUMMARY_PREFIX, hasher.finalize())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelSummaryPayload {
    summary: String,
    #[serde(default)]
    key_points: Vec<String>,
    #[serde(default)]
    questions: Vec<String>,
}

fn build_chapter_summary_user_prompt(
    config: &ChapterSummaryConfig,
    req: &GenerateChapterSummaryRequest,
) -> String {
    format!(
        "书籍URL：{}\n章节：{}\n详细程度：{}\n最多{}字\n\n正文：\n{}",
        req.book_url,
        req.chapter_title.as_deref().unwrap_or("未命名章节"),
        config.detail_level,
        config.max_words,
        trim_content_for_summary(&req.content)
    )
}

fn trim_content_for_summary(content: &str) -> String {
    const MAX_CHARS: usize = 12_000;
    let count = content.chars().count();
    if count <= MAX_CHARS {
        return content.to_string();
    }
    let head: String = content.chars().take(8_000).collect();
    let tail: String = content.chars().skip(count.saturating_sub(4_000)).collect();
    format!("{}\n\n……中间内容已省略……\n\n{}", head, tail)
}

fn build_summary_model_body(
    path: &str,
    model: &str,
    config: &ChapterSummaryConfig,
    req: &GenerateChapterSummaryRequest,
) -> Value {
    let user_prompt = build_chapter_summary_user_prompt(config, req);
    if path.ends_with("/responses") {
        return json!({
            "model": model,
            "temperature": config.temperature,
            "input": [
                { "role": "system", "content": config.prompt },
                { "role": "user", "content": user_prompt }
            ]
        });
    }
    json!({
        "model": model,
        "temperature": config.temperature,
        "messages": [
            { "role": "system", "content": config.prompt },
            { "role": "user", "content": user_prompt }
        ]
    })
}

fn extract_model_content(path: &str, value: &Value) -> Result<String, AppError> {
    if path.ends_with("/responses") {
        if let Some(text) = value.get("output_text").and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                return Ok(text.to_string());
            }
        }
        let text = value
            .get("output")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("content").and_then(Value::as_array))
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Ok(text);
        }
    }

    value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::BadRequest("摘要模型返回内容为空".to_string()))
}

fn parse_summary_payload(content: &str) -> Result<ModelSummaryPayload, AppError> {
    let trimmed = content.trim();
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };
    serde_json::from_str::<ModelSummaryPayload>(json_text)
        .map_err(|_| AppError::BadRequest("摘要模型返回 JSON 格式不正确".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::service::json_document_service::JsonDocumentService;
    use crate::model::chapter_summary::{ChapterSummaryConfig, ChapterSummaryRecord};
    use uuid::Uuid;
    use std::sync::Arc;
    use tokio::fs;

    async fn create_service() -> (ChapterSummaryService, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("reader-chapter-summary-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        let docs = Arc::new(JsonDocumentService::new(pool, dir.to_str().unwrap()));
        (ChapterSummaryService::new(docs), dir)
    }

    #[tokio::test]
    async fn chapter_summary_config_defaults_are_safe() {
        let (service, dir) = create_service().await;
        let config = service.get_config().await.unwrap();

        assert!(config.enabled);
        assert!(config.auto_enabled_default);
        assert_eq!(config.detail_level, "normal");
        assert_eq!(config.max_words, 300);
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.min_content_chars, 300);
        assert!(config.prompt.contains("只总结用户提供的本章正文"));

        let _ = fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn generate_rejects_short_content() {
        let (service, dir) = create_service().await;
        let config = ChapterSummaryConfig { min_content_chars: 10, ..Default::default() };
        let err = service.validate_generation_input(&config, "太短").unwrap_err();

        assert!(err.to_string().contains("正文内容不足"));
        let _ = fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn get_or_cached_summary_uses_cache_when_not_forced() {
        let (service, dir) = create_service().await;
        let record = ChapterSummaryRecord {
            book_url: "book-a".to_string(),
            chapter_url: "chapter-1".to_string(),
            chapter_index: Some(1),
            chapter_title: Some("第一章".to_string()),
            summary: "缓存摘要".to_string(),
            key_points: vec![],
            questions: vec![],
            prompt_version: "default-v1".to_string(),
            model: "cached-model".to_string(),
            created_at: 1,
            updated_at: 1,
        };
        service.save_summary("u1", record).await.unwrap();

        let cached = service
            .get_cached_if_allowed("u1", "book-a", "chapter-1", false)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cached.summary, "缓存摘要");

        let forced = service
            .get_cached_if_allowed("u1", "book-a", "chapter-1", true)
            .await
            .unwrap();
        assert!(forced.is_none());

        let _ = fs::remove_dir_all(dir).await;
    }

    #[test]
    fn responses_endpoint_uses_responses_body_and_output_text() {
        let config = ChapterSummaryConfig::default();
        let req = GenerateChapterSummaryRequest {
            book_url: "book-a".to_string(),
            chapter_url: "chapter-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            content: "足够长的正文".repeat(80),
            ..Default::default()
        };

        let body = build_summary_model_body("/v1/responses", "test-model", &config, &req);
        assert!(body.get("messages").is_none());
        assert!(body.get("input").is_some());

        let content = extract_model_content(
            "/v1/responses",
            &json!({ "output_text": "{\"summary\":\"ok\",\"keyPoints\":[],\"questions\":[]}" }),
        )
        .unwrap();
        assert!(content.contains("\"summary\":\"ok\""));

        let nested = extract_model_content(
            "/v1/responses",
            &json!({
                "output": [{
                    "type": "message",
                    "content": [{ "type": "output_text", "text": "{\"summary\":\"nested\",\"keyPoints\":[],\"questions\":[]}" }]
                }]
            }),
        )
        .unwrap();
        assert!(nested.contains("\"summary\":\"nested\""));
    }

    #[tokio::test]
    async fn chapter_summary_cache_is_scoped_by_user_book_and_chapter() {
        let (service, dir) = create_service().await;
        let record = ChapterSummaryRecord {
            book_url: "book-a".to_string(),
            chapter_url: "chapter-1".to_string(),
            chapter_index: Some(1),
            chapter_title: Some("第一章".to_string()),
            summary: "主角醒来并发现异样。".to_string(),
            key_points: vec!["主角醒来".to_string()],
            questions: vec!["异样来源未知".to_string()],
            prompt_version: "default-v1".to_string(),
            model: "test-model".to_string(),
            created_at: 10,
            updated_at: 20,
        };

        service.save_summary("u1", record.clone()).await.unwrap();

        assert!(service.get_summary("u1", "book-a", "chapter-1").await.unwrap().is_some());
        assert!(service.get_summary("u2", "book-a", "chapter-1").await.unwrap().is_none());
        assert!(service.get_summary("u1", "book-a", "chapter-2").await.unwrap().is_none());

        let _ = fs::remove_dir_all(dir).await;
    }
}
