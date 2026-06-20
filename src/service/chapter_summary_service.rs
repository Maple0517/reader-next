use std::sync::Arc;

use md5::{Digest, Md5};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::error::AppError;
use crate::model::ai_model::{AiModelConfig, AiModelKind};
use crate::model::ai_proxy::{ai_proxy_timeout, build_ai_proxy_url};
use crate::model::chapter_summary::{
    ChapterSummaryConfig, ChapterSummaryContextChapter, ChapterSummaryRecord,
    GenerateChapterSummaryRequest,
};
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

    pub async fn save_config(
        &self,
        config: ChapterSummaryConfig,
    ) -> Result<ChapterSummaryConfig, AppError> {
        let config = config.sanitized();
        self.docs
            .set_value(APP_NAMESPACE, CONFIG_NAME, &config)
            .await?;
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
            return Err(AppError::BadRequest(
                "正文内容不足，未达到生成摘要的最短长度".to_string(),
            ));
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
        _client: &Client,
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
        if !endpoint.enabled
            || endpoint.base_url.trim().is_empty()
            || endpoint.model.trim().is_empty()
        {
            return Err(AppError::BadRequest(
                "后端文本模型未启用或配置不完整".to_string(),
            ));
        }

        let path = if endpoint.path.trim().is_empty() {
            "/v1/chat/completions"
        } else {
            endpoint.path.trim()
        };
        let target = build_ai_proxy_url(&endpoint.base_url, path, endpoint.use_full_url)
            .map_err(AppError::BadRequest)?;
        let use_gemini_api_key_header = is_gemini_generate_content_path(path)
            && target.host_str() == Some("generativelanguage.googleapis.com");
        let previous_context = self
            .load_previous_summary_context(
                user_ns,
                &req.book_url,
                req.chapter_index,
                &req.previous_chapters,
            )
            .await?;
        let body =
            build_summary_model_body(path, &endpoint.model, &config, &req, &previous_context);

        let model_client = Client::builder().timeout(ai_proxy_timeout()).build()?;
        let mut builder = model_client
            .post(target)
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&body);
        if !endpoint.api_key.trim().is_empty() {
            if use_gemini_api_key_header {
                builder = builder.header("x-goog-api-key", endpoint.api_key.trim());
            } else {
                builder = builder.bearer_auth(endpoint.api_key.trim());
            }
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
        let old = self
            .get_summary(user_ns, &req.book_url, &req.chapter_url)
            .await?;
        let created_at = old.as_ref().map(|v| v.created_at).unwrap_or(now);
        let record = ChapterSummaryRecord {
            book_url: req.book_url,
            chapter_url: req.chapter_url,
            chapter_index: req.chapter_index,
            chapter_title: req.chapter_title,
            summary: parsed.summary,
            key_points: parsed.key_points,
            prompt_version: "default-v1".to_string(),
            model: endpoint.model,
            created_at,
            updated_at: now,
        };
        self.save_summary(user_ns, record).await
    }

    async fn load_previous_summary_context(
        &self,
        user_ns: &str,
        book_url: &str,
        current_chapter_index: Option<i32>,
        previous_chapters: &[ChapterSummaryContextChapter],
    ) -> Result<Vec<PreviousChapterSummaryContext>, AppError> {
        let mut context = Vec::new();
        for chapter in previous_chapters
            .iter()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            if chapter.chapter_url.trim().is_empty() {
                continue;
            }
            if let (Some(current), Some(candidate)) = (current_chapter_index, chapter.chapter_index)
            {
                if candidate >= current {
                    continue;
                }
            }
            let Some(record) = self
                .get_summary(user_ns, book_url, &chapter.chapter_url)
                .await?
            else {
                continue;
            };
            if let (Some(current), Some(candidate)) = (current_chapter_index, record.chapter_index)
            {
                if candidate >= current {
                    continue;
                }
            }
            context.push(PreviousChapterSummaryContext {
                chapter_index: record.chapter_index.or(chapter.chapter_index),
                chapter_title: record
                    .chapter_title
                    .or_else(|| chapter.chapter_title.clone()),
                summary: record.summary,
                key_points: record.key_points,
            });
        }
        Ok(context)
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
}

#[derive(Debug, Clone, PartialEq)]
struct PreviousChapterSummaryContext {
    chapter_index: Option<i32>,
    chapter_title: Option<String>,
    summary: String,
    key_points: Vec<String>,
}

fn build_chapter_summary_user_prompt(
    config: &ChapterSummaryConfig,
    req: &GenerateChapterSummaryRequest,
    previous_context: &[PreviousChapterSummaryContext],
) -> String {
    let previous = format_previous_summary_context(previous_context);
    format!(
        "书籍URL：{}\n章节：{}\n{}\n最多{}字\n\n{}\n正文：\n{}",
        req.book_url,
        req.chapter_title.as_deref().unwrap_or("未命名章节"),
        chapter_summary_detail_instruction(&config.detail_level),
        config.max_words,
        previous,
        trim_content_for_summary(&req.content)
    )
}

fn chapter_summary_detail_instruction(detail_level: &str) -> &'static str {
    match detail_level {
        "short" => "详细程度：短。摘要控制在80-150字，要点2-4条，只保留主线变化。",
        "detailed" => {
            "详细程度：详细。摘要可接近字数上限，要点5-8条，保留人物动机、关系变化和关键信息。"
        }
        _ => "详细程度：正常。摘要控制在150-300字，要点3-6条，兼顾主线和后续需记住的信息。",
    }
}

fn format_previous_summary_context(previous_context: &[PreviousChapterSummaryContext]) -> String {
    if previous_context.is_empty() {
        return "前文缓存摘要：无。".to_string();
    }
    let mut total_chars = 0usize;
    let mut lines =
        vec!["前文缓存摘要（只作轻量上下文，不要重复总结；不要推断下一章）：".to_string()];
    for item in previous_context {
        if total_chars >= 1_200 {
            break;
        }
        let title = item
            .chapter_title
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("未命名章节");
        let prefix = item
            .chapter_index
            .map(|idx| format!("第{}章 {}", idx + 1, title))
            .unwrap_or_else(|| title.to_string());
        let points = item.key_points.join("；");
        let mut text = if points.is_empty() {
            format!("- {}：{}", prefix, item.summary)
        } else {
            format!("- {}：{} 读前记住：{}", prefix, item.summary, points)
        };
        text = take_chars(&text, 260);
        total_chars += text.chars().count();
        lines.push(text);
    }
    lines.join("\n")
}

fn take_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    format!("{}…", text.chars().take(max_chars).collect::<String>())
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
    previous_context: &[PreviousChapterSummaryContext],
) -> Value {
    let user_prompt = build_chapter_summary_user_prompt(config, req, previous_context);
    if is_gemini_generate_content_path(path) {
        return json!({
            "contents": [
                { "role": "user", "parts": [{ "text": user_prompt }] }
            ],
            "systemInstruction": { "parts": [{ "text": config.prompt }] },
            "generationConfig": {
                "temperature": config.temperature,
                "maxOutputTokens": 1024,
                "responseMimeType": "application/json"
            }
        });
    }
    if is_anthropic_messages_path(path) {
        return json!({
            "model": model,
            "system": config.prompt,
            "temperature": config.temperature,
            "max_tokens": 1024,
            "messages": [
                { "role": "user", "content": user_prompt }
            ]
        });
    }
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
    if is_gemini_generate_content_path(path) {
        let text = value
            .get("candidates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|candidate| {
                candidate
                    .pointer("/content/parts")
                    .and_then(Value::as_array)
            })
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

    if is_anthropic_messages_path(path) {
        let text = value
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
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

fn is_gemini_generate_content_path(path: &str) -> bool {
    path.split('?').next().is_some_and(|path| {
        path.ends_with(":generateContent") || path.ends_with(":streamGenerateContent")
    })
}

fn is_anthropic_messages_path(path: &str) -> bool {
    path.split('?')
        .next()
        .is_some_and(|path| path.ends_with("/v1/messages"))
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
    use crate::model::chapter_summary::{ChapterSummaryConfig, ChapterSummaryRecord};
    use crate::service::json_document_service::JsonDocumentService;
    use crate::storage::db;
    use std::sync::Arc;
    use tokio::fs;
    use uuid::Uuid;

    async fn create_service() -> (ChapterSummaryService, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("reader-chapter-summary-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        let docs = Arc::new(JsonDocumentService::new(pool, dir.to_str().unwrap()));
        (ChapterSummaryService::new(docs), dir)
    }

    #[test]
    fn default_prompt_outputs_only_summary_and_key_points() {
        let prompt = crate::model::chapter_summary::default_chapter_summary_prompt();

        assert!(prompt.contains("\"summary\""));
        assert!(prompt.contains("\"keyPoints\""));
        assert!(!prompt.contains("150-300字本章梗概"));
        assert!(!prompt.contains("questions"));
        assert!(!prompt.contains("疑点"));
    }

    #[test]
    fn legacy_question_prompt_is_replaced_by_new_default() {
        let config = ChapterSummaryConfig {
            prompt: "你是小说阅读助手。只总结用户提供的本章正文，不预测未读内容。使用简体中文，输出 JSON：{\"summary\":\"梗概\",\"keyPoints\":[\"关键人物或线索\"],\"questions\":[\"伏笔疑点\"]}。".to_string(),
            ..Default::default()
        }
        .sanitized();

        assert!(config.prompt.contains("\"summary\""));
        assert!(config.prompt.contains("\"keyPoints\""));
        assert!(!config.prompt.contains("questions"));
        assert!(!config.prompt.contains("伏笔疑点"));
    }

    #[test]
    fn old_fixed_length_prompt_is_replaced_by_detail_aware_default() {
        let config = ChapterSummaryConfig {
            prompt: "你是小说阅读助手。严格只输出 JSON：{\"summary\":\"150-300字本章梗概\",\"keyPoints\":[\"读者后续需要记住的关键人物、关系、目标、地点、物品或已揭示信息\"]}。".to_string(),
            ..Default::default()
        }
        .sanitized();

        assert!(config
            .prompt
            .contains("摘要长度和要点数量按用户消息里的详细程度执行"));
        assert!(!config.prompt.contains("150-300字本章梗概"));
    }

    #[test]
    fn user_prompt_includes_previous_cached_summary_context_only_when_provided() {
        let config = ChapterSummaryConfig::default();
        let req = GenerateChapterSummaryRequest {
            book_url: "book-a".to_string(),
            chapter_url: "chapter-3".to_string(),
            chapter_title: Some("第三章".to_string()),
            content: "当前正文".repeat(200),
            ..Default::default()
        };
        let previous = vec![PreviousChapterSummaryContext {
            chapter_index: Some(1),
            chapter_title: Some("第一章".to_string()),
            summary: "前文摘要".to_string(),
            key_points: vec!["前文要点".to_string()],
        }];

        let prompt = build_chapter_summary_user_prompt(&config, &req, &previous);

        assert!(prompt.contains("前文缓存摘要"));
        assert!(prompt.contains("只作轻量上下文"));
        assert!(prompt.contains("第一章"));
        assert!(prompt.contains("前文摘要"));
        assert!(prompt.contains("前文要点"));
        assert!(prompt.contains("正文："));
        assert!(prompt.contains("当前正文"));
    }

    #[test]
    fn detail_level_changes_prompt_instruction() {
        let req = GenerateChapterSummaryRequest {
            book_url: "book-a".to_string(),
            chapter_url: "chapter-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            content: "当前正文".repeat(200),
            ..Default::default()
        };
        let short = ChapterSummaryConfig {
            detail_level: "short".to_string(),
            ..Default::default()
        };
        let detailed = ChapterSummaryConfig {
            detail_level: "detailed".to_string(),
            ..Default::default()
        };

        let short_prompt = build_chapter_summary_user_prompt(&short, &req, &[]);
        let detailed_prompt = build_chapter_summary_user_prompt(&detailed, &req, &[]);

        assert!(short_prompt.contains("要点2-4条"));
        assert!(detailed_prompt.contains("要点5-8条"));
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
        let config = ChapterSummaryConfig {
            min_content_chars: 10,
            ..Default::default()
        };
        let err = service
            .validate_generation_input(&config, "太短")
            .unwrap_err();

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

    #[tokio::test]
    async fn previous_summary_context_skips_non_previous_chapters_by_index() {
        let (service, dir) = create_service().await;
        for (url, index, summary) in [
            ("chapter-1", 0, "上一章摘要"),
            ("chapter-3", 2, "下一章摘要"),
        ] {
            service
                .save_summary(
                    "u1",
                    ChapterSummaryRecord {
                        book_url: "book-a".to_string(),
                        chapter_url: url.to_string(),
                        chapter_index: Some(index),
                        chapter_title: Some(format!("第{}章", index + 1)),
                        summary: summary.to_string(),
                        key_points: vec![],
                        prompt_version: "default-v1".to_string(),
                        model: "test-model".to_string(),
                        created_at: 1,
                        updated_at: 1,
                    },
                )
                .await
                .unwrap();
        }

        let context = service
            .load_previous_summary_context(
                "u1",
                "book-a",
                Some(1),
                &[
                    ChapterSummaryContextChapter {
                        chapter_url: "chapter-1".to_string(),
                        chapter_index: Some(0),
                        chapter_title: Some("第1章".to_string()),
                    },
                    ChapterSummaryContextChapter {
                        chapter_url: "chapter-3".to_string(),
                        chapter_index: Some(2),
                        chapter_title: Some("第3章".to_string()),
                    },
                ],
            )
            .await
            .unwrap();

        assert_eq!(context.len(), 1);
        assert_eq!(context[0].summary, "上一章摘要");
        assert!(!context.iter().any(|item| item.summary.contains("下一章")));

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

        let body = build_summary_model_body("/v1/responses", "test-model", &config, &req, &[]);
        assert!(body.get("messages").is_none());
        assert!(body.get("input").is_some());

        let content = extract_model_content(
            "/v1/responses",
            &json!({ "output_text": "{\"summary\":\"ok\",\"keyPoints\":[]}" }),
        )
        .unwrap();
        assert!(content.contains("\"summary\":\"ok\""));

        let nested = extract_model_content(
            "/v1/responses",
            &json!({
                "output": [{
                    "type": "message",
                    "content": [{ "type": "output_text", "text": "{\"summary\":\"nested\",\"keyPoints\":[]}" }]
                }]
            }),
        )
        .unwrap();
        assert!(nested.contains("\"summary\":\"nested\""));
    }

    #[test]
    fn gemini_endpoint_uses_generate_content_body_and_extracts_candidate_text() {
        let config = ChapterSummaryConfig::default();
        let req = GenerateChapterSummaryRequest {
            book_url: "book-a".to_string(),
            chapter_url: "chapter-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            content: "足够长的正文".repeat(80),
            ..Default::default()
        };

        let body = build_summary_model_body(
            "/v1beta/models/gemini-2.5-pro:generateContent",
            "gemini-2.5-pro",
            &config,
            &req,
            &[],
        );
        assert!(body.get("messages").is_none());
        assert_eq!(body.pointer("/contents/0/role"), Some(&json!("user")));
        assert_eq!(
            body.pointer("/systemInstruction/parts/0/text"),
            Some(&json!(config.prompt))
        );
        assert!(
            (body
                .pointer("/generationConfig/temperature")
                .and_then(Value::as_f64)
                .unwrap()
                - 0.3)
                .abs()
                < 0.0001
        );

        let content = extract_model_content(
            "/v1beta/models/gemini-2.5-pro:generateContent",
            &json!({ "candidates": [{ "content": { "parts": [{ "text": "{\"summary\":\"gemini\",\"keyPoints\":[]}" }] } }] }),
        )
        .unwrap();
        assert!(content.contains("\"summary\":\"gemini\""));
    }

    #[test]
    fn anthropic_endpoint_uses_messages_body_and_extracts_content_text() {
        let config = ChapterSummaryConfig::default();
        let req = GenerateChapterSummaryRequest {
            book_url: "book-a".to_string(),
            chapter_url: "chapter-1".to_string(),
            chapter_title: Some("第一章".to_string()),
            content: "足够长的正文".repeat(80),
            ..Default::default()
        };

        let body = build_summary_model_body("/v1/messages", "claude-sonnet-4", &config, &req, &[]);
        assert!(body.get("messages").is_some());
        assert_eq!(body.get("system"), Some(&json!(config.prompt)));
        assert_eq!(body.get("max_tokens"), Some(&json!(1024)));

        let content = extract_model_content(
            "/v1/messages",
            &json!({ "content": [{ "type": "text", "text": "{\"summary\":\"claude\",\"keyPoints\":[]}" }] }),
        )
        .unwrap();
        assert!(content.contains("\"summary\":\"claude\""));
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
            prompt_version: "default-v1".to_string(),
            model: "test-model".to_string(),
            created_at: 10,
            updated_at: 20,
        };

        service.save_summary("u1", record.clone()).await.unwrap();

        assert!(service
            .get_summary("u1", "book-a", "chapter-1")
            .await
            .unwrap()
            .is_some());
        assert!(service
            .get_summary("u2", "book-a", "chapter-1")
            .await
            .unwrap()
            .is_none());
        assert!(service
            .get_summary("u1", "book-a", "chapter-2")
            .await
            .unwrap()
            .is_none());

        let _ = fs::remove_dir_all(dir).await;
    }
}
