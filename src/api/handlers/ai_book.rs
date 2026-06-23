use axum::{
    body::Bytes,
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::{auth::AuthContext, AppState};

use crate::error::error::{ApiResponse, AppError};
use crate::model::ai_book::{AiBookChapterMemoryViewResponse, AiBookMemoryViewResponse};
use crate::model::ai_book_catchup::{
    AiBookCatchupCancelRequest, AiBookCatchupStartRequest, AiBookCatchupStatusRequest,
    AiBookCatchupTaskView,
};
use crate::service::ai_book_catchup_service::{
    fetch_content_fn, save_memory_fn, CatchupBookContext, CatchupChapter,
};
use crate::service::ai_book_generation_service::AiBookGenerationMode;
use crate::service::ai_book_memory_v3::{
    select_ai_book_chapter_view_v3, select_ai_book_display_memory_v3,
};
use crate::service::local_txt_book::is_local_txt_origin;
use crate::util::text::repair_encoded_url;

const CATCHUP_TASK_MEMORY_KEY: &str = "catchupTask";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookMemoryRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookChapterMemoryRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub chapter_index: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookEnabledRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookGenerateChapterRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub chapter_index: Option<i32>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AiBookGenerateMapRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub source_chapter_index: Option<i32>,
    pub prompt: Option<String>,
}

pub async fn get_ai_book_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<AiBookMemoryRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let req = parse_ai_book_request(q, body)?;
    let book_url = required_book_url(req.book_url)?;
    let shelf_book = ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let memory = state
        .ai_book_service
        .get_or_create_v3(
            &user_ns,
            &book_url,
            Some(shelf_book.name.clone()),
            Some(shelf_book.author.clone()),
        )
        .await?;
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(AiBookMemoryViewResponse {
            memory: select_ai_book_display_memory_v3(&memory),
        })
        .unwrap_or_default(),
    )))
}

pub async fn get_ai_book_chapter_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<AiBookChapterMemoryRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let req = parse_ai_book_chapter_request(q, body)?;
    let book_url = required_book_url(req.book_url)?;
    let chapter_index = required_chapter_index(req.chapter_index)?;
    let shelf_book = ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let memory = state
        .ai_book_service
        .get_or_create_v3(
            &user_ns,
            &book_url,
            Some(shelf_book.name.clone()),
            Some(shelf_book.author.clone()),
        )
        .await?;
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(AiBookChapterMemoryViewResponse {
            chapter: select_ai_book_chapter_view_v3(&memory, chapter_index),
            memory: select_ai_book_display_memory_v3(&memory),
        })
        .unwrap_or_default(),
    )))
}

pub async fn reset_ai_book_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<AiBookMemoryRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let book_url = required_book_url(req.book_url)?;
    let shelf_book = ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let memory = state
        .ai_book_service
        .reset_v3(
            &user_ns,
            &book_url,
            Some(shelf_book.name.clone()),
            Some(shelf_book.author.clone()),
        )
        .await?;
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(AiBookMemoryViewResponse {
            memory: select_ai_book_display_memory_v3(&memory),
        })
        .unwrap_or_default(),
    )))
}

pub async fn set_ai_book_enabled(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<AiBookEnabledRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let book_url = required_book_url(req.book_url)?;
    let shelf_book = ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let mut memory = state
        .ai_book_service
        .get_or_create_v3(
            &user_ns,
            &book_url,
            Some(shelf_book.name.clone()),
            Some(shelf_book.author.clone()),
        )
        .await?;
    memory.enabled = req.enabled;
    let memory = state
        .ai_book_service
        .save_v3(&user_ns, &book_url, memory)
        .await?;
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(AiBookMemoryViewResponse {
            memory: select_ai_book_display_memory_v3(&memory),
        })
        .unwrap_or_default(),
    )))
}

pub async fn generate_ai_book_chapter_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<AiBookGenerateChapterRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let book_url = required_book_url(req.book_url)?;
    let chapter_index = required_chapter_index(req.chapter_index)?;
    let shelf_book = ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let response = state
        .ai_book_generation_service
        .generate_current_chapter(
            &user_ns,
            &shelf_book,
            chapter_index,
            parse_generation_mode(req.mode.as_deref()),
        )
        .await?;
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(response).unwrap_or_default(),
    )))
}

pub async fn generate_ai_book_map(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<AiBookGenerateMapRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let book_url = required_book_url(req.book_url)?;
    let _ = req.source_chapter_index;
    let _ = req.prompt;
    ensure_shelf_book(&state, &user_ns, &book_url).await?;
    Err(AppError::BadRequest(
        "AI资料地图生成功能尚未接入模型".to_string(),
    ))
}

async fn resolve_user_ns(state: &AppState, auth: &AuthContext) -> Result<String, AppError> {
    state
        .user_service
        .resolve_user_ns_with_override(auth.access_token(), auth.secure_key(), auth.user_ns())
        .await
        .map_err(|_| AppError::BadRequest("NEED_LOGIN".to_string()))
}

async fn ensure_shelf_book(
    state: &AppState,
    user_ns: &str,
    book_url: &str,
) -> Result<crate::model::book::Book, AppError> {
    state
        .book_service
        .get_shelf_book(user_ns, book_url)
        .await?
        .ok_or_else(|| AppError::BadRequest("书籍未加入书架".to_string()))
}

fn parse_ai_book_request(
    q: AiBookMemoryRequest,
    body: Bytes,
) -> Result<AiBookMemoryRequest, AppError> {
    if body.is_empty() {
        return Ok(q);
    }
    if let Ok(v) = serde_json::from_slice::<AiBookMemoryRequest>(&body) {
        return Ok(v);
    }
    let text = std::str::from_utf8(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let mut req = q;
    for (k, v) in url::form_urlencoded::parse(text.as_bytes()) {
        match k.as_ref() {
            "bookUrl" | "url" => req.book_url = Some(v.into_owned()),
            _ => {}
        }
    }
    Ok(req)
}

fn parse_ai_book_chapter_request(
    q: AiBookChapterMemoryRequest,
    body: Bytes,
) -> Result<AiBookChapterMemoryRequest, AppError> {
    parse_catchup_request(q, body)
}

fn required_book_url(book_url: Option<String>) -> Result<String, AppError> {
    let book_url = book_url
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("bookUrl required".to_string()))?;
    Ok(repair_encoded_url(&book_url))
}

fn required_chapter_index(chapter_index: Option<i32>) -> Result<i32, AppError> {
    chapter_index.ok_or_else(|| AppError::BadRequest("chapterIndex required".to_string()))
}

fn parse_generation_mode(raw: Option<&str>) -> AiBookGenerationMode {
    match raw.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "auto" => AiBookGenerationMode::Auto,
        _ => AiBookGenerationMode::Manual,
    }
}

pub async fn start_ai_book_catchup(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<AiBookCatchupStartRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let req = parse_ai_book_catchup_start_request(q, body)?;
    let book_url = required_book_url(req.book_url)?;
    let shelf_book = ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let target_chapter_index = req.target_chapter_index;
    let state_for_task = state.clone();
    let user_ns_for_task = user_ns.clone();
    let book_url_for_task = book_url.clone();
    let shelf_book_for_task = shelf_book.clone();
    let task = state
        .ai_book_catchup_service
        .start_with(
            user_ns.clone(),
            book_url.clone(),
            target_chapter_index,
            move || async move {
                let memory = state_for_task
                    .ai_book_service
                    .get_value(&user_ns_for_task, &book_url_for_task)
                    .await?
                    .unwrap_or_else(|| {
                        serde_json::json!({
                            "schemaVersion": 2,
                            "bookUrl": book_url_for_task.clone(),
                            "bookName": shelf_book_for_task.name.clone(),
                            "author": shelf_book_for_task.author.clone(),
                            "enabled": true,
                            "summary": { "current": "", "recentChanges": [], "openQuestions": [] },
                            "chapterDigests": [],
                            "arcs": [],
                            "worldFacts": [],
                            "characters": [],
                            "relationships": [],
                            "locations": [],
                            "mapState": { "dirty": false, "nodes": [], "edges": [] },
                            "renderArtifacts": {},
                        })
                    });
                let start_chapter_index = memory
                    .get("processedChapterIndex")
                    .and_then(Value::as_i64)
                    .map(|index| (index as i32) + 1)
                    .unwrap_or(0);
                let save_start_failure = {
                    let state = state_for_task.clone();
                    let user_ns = user_ns_for_task.clone();
                    let book_url = book_url_for_task.clone();
                    let fallback_memory = memory.clone();
                    move |error: String| {
                        let state = state.clone();
                        let user_ns = user_ns.clone();
                        let book_url = book_url.clone();
                        let fallback_memory = fallback_memory.clone();
                        async move {
                            let latest = state
                                .ai_book_service
                                .get_value(&user_ns, &book_url)
                                .await
                                .ok()
                                .flatten();
                            let memory = build_catchup_start_failure_memory(
                                latest,
                                fallback_memory,
                                start_chapter_index,
                                &error,
                            );
                            let _ = state
                                .ai_book_service
                                .save_value_for_book(&user_ns, &book_url, memory)
                                .await;
                            AppError::BadRequest(error)
                        }
                    }
                };
                let ai_config = match state_for_task.ai_model_service.get().await {
                    Ok(config) => config,
                    Err(err) => return Err(save_start_failure(err.to_string()).await),
                };
                let chapters = match load_catchup_chapters(
                    &state_for_task,
                    &user_ns_for_task,
                    &shelf_book_for_task,
                    start_chapter_index,
                    target_chapter_index,
                )
                .await
                {
                    Ok(chapters) => chapters,
                    Err(err) => return Err(save_start_failure(err.to_string()).await),
                };
                let save_state = state_for_task.clone();
                let save_user_ns = user_ns_for_task.clone();
                let save_book_url = book_url_for_task.clone();
                let fetch_state = state_for_task.clone();
                let fetch_user_ns = user_ns_for_task.clone();
                let fetch_book_url = book_url_for_task.clone();
                let fetch_origin = shelf_book_for_task.origin.clone();
                Ok(CatchupBookContext {
                    book_name: shelf_book_for_task.name.clone(),
                    author: shelf_book_for_task.author.clone(),
                    chapters,
                    memory,
                    ai_config,
                    save_memory: save_memory_fn(move |memory| {
                        let save_state = save_state.clone();
                        let save_user_ns = save_user_ns.clone();
                        let save_book_url = save_book_url.clone();
                        async move {
                            save_state
                                .ai_book_service
                                .save_value_for_book(&save_user_ns, &save_book_url, memory)
                                .await
                        }
                    }),
                    fetch_content: fetch_content_fn(move |chapter| {
                        let fetch_state = fetch_state.clone();
                        let fetch_user_ns = fetch_user_ns.clone();
                        let fetch_book_url = fetch_book_url.clone();
                        let fetch_origin = fetch_origin.clone();
                        async move {
                            if is_local_txt_origin(&fetch_origin)
                                || fetch_book_url.starts_with("local-txt:")
                            {
                                return fetch_state
                                    .local_txt_book_service
                                    .get_content(&fetch_user_ns, &chapter.chapter_url)
                                    .await;
                            }
                            let source = crate::api::handlers::resolve_book_source(
                                &fetch_state,
                                &fetch_user_ns,
                                Some(fetch_origin),
                                None,
                                Some(&fetch_book_url),
                            )
                            .await?;
                            fetch_state
                                .book_service
                                .get_content(
                                    &fetch_user_ns,
                                    &fetch_book_url,
                                    &source,
                                    &chapter.chapter_url,
                                )
                                .await
                        }
                    }),
                })
            },
        )
        .await?;
    persist_catchup_status(&state, &user_ns, &book_url, &task).await;
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(task).unwrap_or_default(),
    )))
}

pub async fn get_ai_book_catchup_status(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<AiBookCatchupStatusRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let req = parse_ai_book_catchup_status_request(q, body)?;
    let book_url = required_book_url(req.book_url)?;
    ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let task = if let Some(task) = state
        .ai_book_catchup_service
        .get_status(&user_ns, &book_url)
        .await
    {
        task
    } else {
        idle_catchup_status(&state, &user_ns, &book_url).await?
    };
    if task.status != "idle" {
        persist_catchup_status(&state, &user_ns, &book_url, &task).await;
    }
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(task).unwrap_or_default(),
    )))
}

pub async fn cancel_ai_book_catchup(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<AiBookCatchupCancelRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let req = parse_ai_book_catchup_cancel_request(q, body)?;
    let book_url = required_book_url(req.book_url)?;
    ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let _ = state
        .ai_book_catchup_service
        .request_cancel(&user_ns, &book_url)
        .await;
    clear_catchup_status(&state, &user_ns, &book_url).await;
    Ok(Json(ApiResponse::ok(
        serde_json::to_value(idle_catchup_status(&state, &user_ns, &book_url).await?)
            .unwrap_or_default(),
    )))
}

async fn load_catchup_chapters(
    state: &AppState,
    user_ns: &str,
    shelf_book: &crate::model::book::Book,
    start_chapter_index: i32,
    target_chapter_index: Option<i32>,
) -> Result<Vec<CatchupChapter>, AppError> {
    let book_url = repair_encoded_url(&shelf_book.book_url);
    if is_local_txt_origin(&shelf_book.origin) || book_url.starts_with("local-txt:") {
        let chapters = state
            .local_txt_book_service
            .get_chapter_list(user_ns, &book_url)
            .await?;
        let mut items = Vec::with_capacity(chapters.len());
        for chapter in chapters {
            if chapter.index < start_chapter_index {
                continue;
            }
            if target_chapter_index.is_some_and(|target| chapter.index > target) {
                break;
            }
            items.push(CatchupChapter {
                title: chapter.title,
                chapter_url: chapter.url,
                index: chapter.index,
            });
        }
        return Ok(items);
    }
    let source = crate::api::handlers::resolve_book_source(
        state,
        user_ns,
        Some(shelf_book.origin.clone()),
        None,
        Some(&book_url),
    )
    .await?;
    let toc_url = shelf_book.toc_url.as_deref().unwrap_or(&book_url);
    let chapters = state
        .book_service
        .get_chapter_list_with_cache(user_ns, &source, toc_url, false)
        .await?;
    let mut items = Vec::with_capacity(chapters.len());
    for chapter in chapters {
        if chapter.index < start_chapter_index {
            continue;
        }
        if target_chapter_index.is_some_and(|target| chapter.index > target) {
            break;
        }
        items.push(CatchupChapter {
            title: chapter.title,
            chapter_url: chapter.url,
            index: chapter.index,
        });
    }
    Ok(items)
}

async fn idle_catchup_status(
    state: &AppState,
    user_ns: &str,
    book_url: &str,
) -> Result<crate::model::ai_book_catchup::AiBookCatchupTaskView, AppError> {
    let memory = state.ai_book_service.get_value(user_ns, book_url).await?;
    if let Some(task) = memory
        .as_ref()
        .and_then(|value| read_persisted_catchup_status(value, user_ns, book_url))
    {
        return Ok(task);
    }
    let processed_chapter_index = memory
        .as_ref()
        .and_then(|value| value.get("processedChapterIndex"))
        .and_then(Value::as_i64)
        .map(|value| value as i32);
    let processed_chapter_title = memory
        .as_ref()
        .and_then(|value| value.get("processedChapterTitle"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let error = memory
        .as_ref()
        .and_then(|value| value.get("lastError"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(crate::model::ai_book_catchup::AiBookCatchupTaskView {
        user_ns: user_ns.to_string(),
        book_url: book_url.to_string(),
        status: if error.is_some() { "failed" } else { "idle" }.to_string(),
        start_chapter_index: processed_chapter_index.map(|index| index + 1),
        target_chapter_index: None,
        total_chapters: 0,
        completed_chapters: 0,
        current_chapter_index: None,
        current_chapter_title: None,
        processed_chapter_index,
        processed_chapter_title,
        error,
        current_stage: Some("idle".to_string()),
        stats: None,
        updated_at: memory
            .as_ref()
            .and_then(|value| value.get("updatedAt"))
            .and_then(Value::as_i64)
            .unwrap_or(0),
    })
}

async fn persist_catchup_status(
    state: &AppState,
    user_ns: &str,
    book_url: &str,
    task: &AiBookCatchupTaskView,
) {
    let mut memory = match state.ai_book_service.get_value(user_ns, book_url).await {
        Ok(Some(memory)) => memory,
        _ => empty_ai_book_memory(book_url),
    };
    write_catchup_status_to_memory(&mut memory, task);
    let _ = state
        .ai_book_service
        .save_value_for_book(user_ns, book_url, memory)
        .await;
}

async fn clear_catchup_status(state: &AppState, user_ns: &str, book_url: &str) {
    let Ok(Some(mut memory)) = state.ai_book_service.get_value(user_ns, book_url).await else {
        return;
    };
    let Some(object) = memory.as_object_mut() else {
        return;
    };
    object.remove(CATCHUP_TASK_MEMORY_KEY);
    let _ = state
        .ai_book_service
        .save_value_for_book(user_ns, book_url, memory)
        .await;
}

fn empty_ai_book_memory(book_url: &str) -> Value {
    json!({
        "schemaVersion": 2,
        "bookUrl": book_url,
        "enabled": true,
        "summary": { "current": "", "recentChanges": [], "openQuestions": [] },
        "chapterDigests": [],
        "arcs": [],
        "worldFacts": [],
        "characters": [],
        "relationships": [],
        "locations": [],
        "mapState": { "dirty": false, "nodes": [], "edges": [] },
        "renderArtifacts": {},
    })
}

fn write_catchup_status_to_memory(memory: &mut Value, task: &AiBookCatchupTaskView) {
    let Some(object) = memory.as_object_mut() else {
        return;
    };
    let Ok(value) = serde_json::to_value(task) else {
        return;
    };
    object.insert(CATCHUP_TASK_MEMORY_KEY.to_string(), value);
    object.insert(
        "updatedAt".to_string(),
        Value::Number(task.updated_at.max(0).into()),
    );
}

fn read_persisted_catchup_status(
    memory: &Value,
    user_ns: &str,
    book_url: &str,
) -> Option<AiBookCatchupTaskView> {
    let mut task = serde_json::from_value::<AiBookCatchupTaskView>(
        memory.get(CATCHUP_TASK_MEMORY_KEY)?.clone(),
    )
    .ok()?;
    if task.book_url.trim().is_empty() {
        task.book_url = book_url.to_string();
    }
    if task.book_url != book_url {
        return None;
    }
    task.user_ns = user_ns.to_string();
    if task.updated_at <= 0 {
        task.updated_at = memory
            .get("updatedAt")
            .and_then(Value::as_i64)
            .unwrap_or_default();
    }
    if task.processed_chapter_index.is_none() {
        task.processed_chapter_index = memory
            .get("processedChapterIndex")
            .and_then(Value::as_i64)
            .map(|value| value as i32);
    }
    if task.processed_chapter_title.is_none() {
        task.processed_chapter_title = memory
            .get("processedChapterTitle")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if task.error.is_none() {
        task.error = memory
            .get("lastError")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    normalize_persisted_catchup_status(&mut task);
    Some(task)
}

fn normalize_persisted_catchup_status(task: &mut AiBookCatchupTaskView) {
    if matches!(task.status.as_str(), "running" | "pausing") {
        task.status = "paused".to_string();
    }
    if task.status == "paused" {
        task.current_chapter_index = None;
        task.current_chapter_title = None;
    }
}

fn parse_ai_book_catchup_start_request(
    q: AiBookCatchupStartRequest,
    body: Bytes,
) -> Result<AiBookCatchupStartRequest, AppError> {
    parse_catchup_request(q, body)
}

fn parse_ai_book_catchup_status_request(
    q: AiBookCatchupStatusRequest,
    body: Bytes,
) -> Result<AiBookCatchupStatusRequest, AppError> {
    parse_catchup_request(q, body)
}

fn parse_ai_book_catchup_cancel_request(
    q: AiBookCatchupCancelRequest,
    body: Bytes,
) -> Result<AiBookCatchupCancelRequest, AppError> {
    parse_catchup_request(q, body)
}

fn parse_catchup_request<T>(q: T, body: Bytes) -> Result<T, AppError>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    if body.is_empty() {
        return Ok(q);
    }
    if let Ok(v) = serde_json::from_slice::<T>(&body) {
        return Ok(v);
    }
    let text = std::str::from_utf8(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let mut value = serde_json::to_value(q).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("invalid request".to_string()))?;
    for (k, v) in url::form_urlencoded::parse(text.as_bytes()) {
        object.insert(k.into_owned(), Value::String(v.into_owned()));
    }
    serde_json::from_value(value).map_err(|e| AppError::BadRequest(e.to_string()))
}

fn mark_catchup_start_failed_memory(memory: &mut Value, start_chapter_index: i32, error: &str) {
    let Some(object) = memory.as_object_mut() else {
        return;
    };
    object.insert("lastError".to_string(), Value::String(error.to_string()));
    object.insert(
        "lastErrorChapterIndex".to_string(),
        Value::Number(start_chapter_index.into()),
    );
    object.insert(
        "lastErrorChapterTitle".to_string(),
        Value::String(format!("第 {} 章", start_chapter_index + 1)),
    );
    object.insert(
        "updatedAt".to_string(),
        Value::Number((crate::util::time::now_ts() * 1000).into()),
    );
}

fn build_catchup_start_failure_memory(
    latest: Option<Value>,
    fallback: Value,
    start_chapter_index: i32,
    error: &str,
) -> Value {
    let mut memory = latest.unwrap_or(fallback);
    mark_catchup_start_failed_memory(&mut memory, start_chapter_index, error);
    memory
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::config::AppConfig;
    use crate::model::ai_book_catchup::AiBookCatchupTaskStats;
    use crate::model::book::Book;
    use crate::service::ai_book_generation_service::AiBookGenerationService;
    use crate::service::ai_book_memory_v3::create_empty_ai_book_memory_v3;
    use crate::service::ai_book_service::AiBookService;
    use crate::service::ai_model_service::AiModelService;
    use crate::service::book_group_service::BookGroupService;
    use crate::service::book_service::BookService;
    use crate::service::book_source_service::BookSourceService;
    use crate::service::chapter_summary_service::ChapterSummaryService;
    use crate::service::json_document_service::JsonDocumentService;
    use crate::service::local_txt_book::LocalTxtBookService;
    use crate::service::update_service::UpdateService;
    use crate::service::user_service::UserService;
    use crate::storage::cache::file_cache::FileCache;
    use crate::storage::db;
    use crate::storage::db::repo::BookSourceRepo;
    use crate::util::crypto::random_string;
    use axum::extract::Query;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;


    async fn create_test_state() -> (AppState, PathBuf) {
        let dir = std::env::temp_dir().join(format!("reader-ai-book-handler-{}", random_string(8)));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        let cfg = AppConfig {
            storage_dir: dir.to_string_lossy().to_string(),
            assets_dir: dir.join("assets").to_string_lossy().to_string(),
            web_root: dir.join("web").to_string_lossy().to_string(),
            database_url,
            ..AppConfig::default()
        };
        let http = crate::crawler::http_client::HttpClient::new(cfg.request_timeout_secs, None).unwrap();
        let parser = crate::parser::rule_engine::RuleEngine::new().unwrap();
        let cache = FileCache::new(format!("{}/cache", cfg.storage_dir));
        let book_service = Arc::new(BookService::new(http, parser, cache, &cfg.storage_dir));
        let book_source_service = Arc::new(BookSourceService::new(
            BookSourceRepo::new(pool.clone()),
            &cfg.storage_dir,
        ));
        let local_txt_book_service = Arc::new(LocalTxtBookService::new(&cfg.storage_dir));
        let json_document_service = Arc::new(JsonDocumentService::new(pool.clone(), &cfg.storage_dir));
        let user_service = Arc::new(UserService::new(cfg.clone(), pool.clone()));
        user_service.migrate_legacy_users_from_json().await.unwrap();
        let book_group_service = Arc::new(BookGroupService::new(json_document_service.clone()));
        let ai_book_service = Arc::new(AiBookService::new(pool.clone(), &cfg.storage_dir));
        let ai_book_generation_service = Arc::new(AiBookGenerationService::new(
            ai_book_service.clone(),
            book_service.clone(),
            book_source_service.clone(),
            local_txt_book_service.clone(),
        ));
        let ai_book_catchup_service = Arc::new(crate::service::ai_book_catchup_service::AiBookCatchupService::new());
        let ai_model_service = Arc::new(AiModelService::new(
            json_document_service.clone(),
            &cfg.storage_dir,
        ));
        let chapter_summary_service = Arc::new(ChapterSummaryService::new(json_document_service.clone()));
        let update_service = Arc::new(
            UpdateService::new(
                json_document_service.clone(),
                cfg.request_timeout_secs,
                format!("v{}", env!("CARGO_PKG_VERSION")),
            )
            .unwrap(),
        );
        let state = AppState {
            config: cfg,
            book_service,
            book_source_service,
            user_service,
            book_group_service,
            local_txt_book_service,
            json_document_service,
            ai_book_service,
            ai_book_generation_service,
            ai_book_catchup_service,
            ai_model_service,
            chapter_summary_service,
            update_service,
        };
        (state, dir)
    }

    async fn seed_shelf_book(state: &AppState, user_ns: &str, book_url: &str) {
        state
            .book_service
            .save_book(
                user_ns,
                Book {
                    name: "测试书".to_string(),
                    author: "测试作者".to_string(),
                    book_url: book_url.to_string(),
                    origin: "local".to_string(),
                    ..Book::default()
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn ai_book_v3_get_memory_wraps_api_response() {
        let (state, dir) = create_test_state().await;
        let user_ns = "default";
        let book_url = "book://api-memory";
        seed_shelf_book(&state, user_ns, book_url).await;
        state
            .ai_book_service
            .save_v3(
                user_ns,
                book_url,
                create_empty_ai_book_memory_v3(
                    book_url,
                    Some("接口书".to_string()),
                    Some("接口作者".to_string()),
                ),
            )
            .await
            .unwrap();

        let response = get_ai_book_memory(
            State(state),
            AuthContext::default(),
            Query(AiBookMemoryRequest {
                book_url: Some(book_url.to_string()),
            }),
            Bytes::new(),
        )
        .await
        .unwrap();

        let payload = serde_json::to_value(response.0).unwrap();
        assert_eq!(payload["isSuccess"], json!(true));
        assert_eq!(payload["errorMsg"], json!(""));
        assert_eq!(payload["data"]["memory"]["bookUrl"], json!(book_url));
        assert_eq!(payload["data"]["memory"]["bookName"], json!("接口书"));
        assert_eq!(payload["data"]["memory"]["author"], json!("接口作者"));

        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[tokio::test]
    async fn ai_book_v3_reset_and_enabled_handlers_return_memory_view() {
        let (state, dir) = create_test_state().await;
        let user_ns = "default";
        let book_url = "book://api-actions";
        seed_shelf_book(&state, user_ns, book_url).await;

        let enabled = set_ai_book_enabled(
            State(state.clone()),
            AuthContext::default(),
            Json(AiBookEnabledRequest {
                book_url: Some(book_url.to_string()),
                enabled: true,
            }),
        )
        .await
        .unwrap();
        let enabled_payload = serde_json::to_value(enabled.0).unwrap();
        assert_eq!(enabled_payload["isSuccess"], json!(true));
        assert_eq!(enabled_payload["data"]["memory"]["bookUrl"], json!(book_url));
        assert_eq!(enabled_payload["data"]["memory"]["enabled"], json!(true));
        assert_eq!(enabled_payload["data"]["memory"]["bookName"], json!("测试书"));
        assert_eq!(enabled_payload["data"]["memory"]["author"], json!("测试作者"));

        let reset = reset_ai_book_memory(
            State(state),
            AuthContext::default(),
            Json(AiBookMemoryRequest {
                book_url: Some(book_url.to_string()),
            }),
        )
        .await
        .unwrap();
        let reset_payload = serde_json::to_value(reset.0).unwrap();
        assert_eq!(reset_payload["isSuccess"], json!(true));
        assert_eq!(reset_payload["data"]["memory"]["bookUrl"], json!(book_url));
        assert_eq!(reset_payload["data"]["memory"]["enabled"], json!(false));
        assert_eq!(reset_payload["data"]["memory"]["summary"]["current"], json!(""));

        let _ = tokio::fs::remove_dir_all(dir).await;
    }

    #[test]
    fn start_failure_memory_records_last_error_without_advancing_processed_chapter() {
        let mut memory = json!({
            "schemaVersion": 2,
            "bookUrl": "book-a",
            "processedChapterIndex": 4,
            "processedChapterTitle": "第5章",
            "summary": { "current": "旧资料" },
        });

        mark_catchup_start_failed_memory(&mut memory, 5, "目录加载失败");

        assert_eq!(
            memory.get("processedChapterIndex").and_then(Value::as_i64),
            Some(4)
        );
        assert_eq!(
            memory.get("lastError").and_then(Value::as_str),
            Some("目录加载失败")
        );
        assert_eq!(
            memory.get("lastErrorChapterIndex").and_then(Value::as_i64),
            Some(5)
        );
    }

    #[test]
    fn start_failure_memory_prefers_latest_memory_over_start_snapshot() {
        let snapshot = json!({
            "schemaVersion": 2,
            "bookUrl": "book-a",
            "summary": { "current": "旧资料" },
            "characters": [{ "id": "old", "name": "旧角色" }],
        });
        let latest = json!({
            "schemaVersion": 2,
            "bookUrl": "book-a",
            "summary": { "current": "新资料" },
            "characters": [{ "id": "new", "name": "新角色" }],
        });

        let memory = build_catchup_start_failure_memory(Some(latest), snapshot, 5, "目录加载失败");

        assert_eq!(
            memory.pointer("/summary/current").and_then(Value::as_str),
            Some("新资料")
        );
        assert_eq!(
            memory
                .get("characters")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("id"))
                .and_then(Value::as_str),
            Some("new")
        );
        assert_eq!(
            memory.get("lastError").and_then(Value::as_str),
            Some("目录加载失败")
        );
    }

    #[test]
    fn persisted_catchup_status_survives_idle_fallback() {
        let mut memory = json!({
            "schemaVersion": 2,
            "bookUrl": "book-a",
            "processedChapterIndex": 4,
            "processedChapterTitle": "第5章",
            "updatedAt": 111,
        });
        let task = AiBookCatchupTaskView {
            user_ns: "default".to_string(),
            book_url: "book-a".to_string(),
            status: "paused".to_string(),
            start_chapter_index: Some(5),
            target_chapter_index: Some(9),
            total_chapters: 5,
            completed_chapters: 2,
            current_chapter_index: None,
            current_chapter_title: None,
            processed_chapter_index: Some(6),
            processed_chapter_title: Some("第7章".to_string()),
            error: None,
            current_stage: Some("patch".to_string()),
            stats: Some(AiBookCatchupTaskStats::default()),
            updated_at: 222,
        };

        write_catchup_status_to_memory(&mut memory, &task);
        let restored = read_persisted_catchup_status(&memory, "default", "book-a").unwrap();

        assert_eq!(restored.status, "paused");
        assert_eq!(restored.completed_chapters, 2);
        assert_eq!(restored.processed_chapter_index, Some(6));
        assert_eq!(restored.processed_chapter_title.as_deref(), Some("第7章"));
        assert_eq!(restored.updated_at, 222);
    }

    #[test]
    fn persisted_catchup_status_rejects_mismatched_book_url() {
        let memory = json!({
            "bookUrl": "book-a",
            "catchupTask": {
                "bookUrl": "book-b",
                "status": "paused",
                "totalChapters": 3,
                "completedChapters": 1,
                "updatedAt": 123
            }
        });

        assert!(read_persisted_catchup_status(&memory, "default", "book-a").is_none());
    }

    #[test]
    fn persisted_running_or_pausing_status_restores_as_paused() {
        let memory = json!({
            "bookUrl": "book-a",
            "catchupTask": {
                "bookUrl": "book-a",
                "status": "pausing",
                "currentChapterIndex": 0,
                "currentChapterTitle": "第1章",
                "totalChapters": 10,
                "completedChapters": 0,
                "updatedAt": 123
            }
        });

        let restored = read_persisted_catchup_status(&memory, "default", "book-a").unwrap();
        assert_eq!(restored.status, "paused");
        assert_eq!(restored.current_chapter_index, None);
        assert_eq!(restored.current_chapter_title, None);
    }
}
