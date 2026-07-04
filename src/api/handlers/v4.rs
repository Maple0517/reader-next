use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

use crate::api::{auth::AuthContext, AppState};
use crate::error::error::{ApiResponse, AppError};
use crate::service::v4::knowledge_projection;
use crate::service::v4::projection;
use crate::service::v4::relationship_projection;
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::identity_repo::{
    IdentityLinkRecord, IdentityRepo, MergeOperationRecord,
};
use crate::storage::db::v4::progress_repo::ProgressRepo;
use crate::storage::db::v4::relationship_repo::{RelationshipEventRepo, RelationshipRepo};
use crate::storage::db::v4::reset_v4;
use crate::util::text::repair_encoded_url;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct V4BookRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct V4ChapterMemoryRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub chapter_index: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct V4EnabledRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct V4CatchupStartRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub target_chapter_index: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct V4RelationshipsRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
    pub group: Option<String>,
    #[serde(rename = "minImportance", alias = "min_importance")]
    pub min_importance: Option<f64>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4MemoryResponse {
    pub book_url: String,
    pub book_name: String,
    pub author: String,
    pub max_read_chapter: i64,
    pub max_processed_chapter: i64,
    pub processing: bool,
    pub character_count: i64,
    pub relationship_count: i64,
    pub knowledge_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4CharacterListView {
    pub characters: Vec<projection::CharacterListItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4CharacterResponse {
    pub character: projection::CharacterCardView,
    pub relationship_count: i64,
    pub recent_changes: Vec<Value>,
    pub evidence_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4ChapterMemoryResponse {
    pub chapter_index: i64,
    pub chapter_title: Option<String>,
    pub summary: Option<String>,
    pub key_points: Vec<String>,
    pub characters_in_chapter: Vec<projection::CharacterListItem>,
    pub relationships_in_chapter: Vec<relationship_projection::RelationshipEdge>,
    pub relationship_count: i64,
    pub knowledge_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4MemoryStatusResponse {
    pub max_read_chapter: i64,
    pub max_processed_chapter: i64,
    pub processing: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4CatchupStatusResponse {
    pub status: String,
    pub target_chapter: Option<i64>,
    pub current_chapter: Option<i64>,
    pub max_processed_chapter: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4CharacterRelationshipsResponse {
    pub relationships: Vec<relationship_projection::RelationshipEdge>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4IdentityLinkView {
    pub id: String,
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub link_type: String,
    pub status: String,
    pub confidence: f64,
    pub source_claim_id: String,
    pub redirect_target_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4IdentityLinksResponse {
    pub identity_links: Vec<V4IdentityLinkView>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4CharacterIdentityResponse {
    pub character_id: String,
    pub redirect_target_id: Option<String>,
    pub identity_links: Vec<V4IdentityLinkView>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4MergeOperationView {
    pub id: String,
    pub survivor_entity_id: String,
    pub victim_entity_id: String,
    pub source_identity_link_id: String,
    pub reason_code: String,
    pub confidence: f64,
    pub status: String,
    pub property_conflict_count: i64,
    pub relationship_merge_count: i64,
    pub result_json: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V4MergeOperationsResponse {
    pub merge_operations: Vec<V4MergeOperationView>,
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /v4/memory — book-level memory overview from view_model_cache
pub async fn get_v4_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let book_url = req.book_url.clone().unwrap_or_default();
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let progress_repo = ProgressRepo::new(state.pool.clone());
    let progress = progress_repo
        .get_progress(&ctx.book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let max_read = get_max_read(&state, &ctx.book_id).await?;
    let (max_processed, processing) = if let Some(ref p) = progress {
        (
            p.max_processed_chapter,
            p.status == "running" || p.status == "cancel_requested",
        )
    } else {
        (0, false)
    };

    let characters = projection::project_character_list(&ctx.book_id, max_processed, &state.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let character_count = characters.len() as i64;

    let relationship_repo = RelationshipRepo::new(state.pool.clone());
    let relationship_count = relationship_repo
        .count_active_by_book(&ctx.book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let knowledge_count =
        knowledge_projection::count_active_knowledge_cards(&ctx.book_id, &state.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4MemoryResponse {
            book_url,
            book_name: String::new(), // resolved from book metadata if available
            author: String::new(),    // resolved from book metadata if available
            max_read_chapter: max_read,
            max_processed_chapter: max_processed,
            processing,
            character_count,
            relationship_count,
            knowledge_count,
        })
        .unwrap_or_default(),
    )))
}

/// GET /v4/characters — book-level character list
pub async fn get_v4_characters(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let max_processed = get_max_processed(&state, &ctx.book_id).await?;

    let characters = projection::project_character_list(&ctx.book_id, max_processed, &state.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let total = characters.len() as i64;

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4CharacterListView { characters, total }).unwrap_or_default(),
    )))
}

/// GET /v4/characters/:character_id — entity-level character card
pub async fn get_v4_character_card(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(character_id): Path<String>,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let max_processed = get_max_processed(&state, &ctx.book_id).await?;

    let character =
        projection::project_character_card(&character_id, &ctx.book_id, max_processed, &state.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

    let relationship_repo = RelationshipRepo::new(state.pool.clone());
    let character_relationships = relationship_repo
        .list_by_character(&ctx.book_id, &character_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let relationship_count = character_relationships.len() as i64;

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4CharacterResponse {
            character,
            relationship_count,
            recent_changes: vec![],    // Phase 6: populated from claim history
            evidence_available: false, // Phase 6: populated when evidence drawer is ready
        })
        .unwrap_or_default(),
    )))
}

/// GET /v4/chapter-memory — chapter-level memory view
pub async fn get_v4_chapter_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4ChapterMemoryRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let chapter_index = req
        .chapter_index
        .ok_or_else(|| AppError::BadRequest("chapterIndex required".to_string()))?;

    let characters = projection::project_character_list(&ctx.book_id, chapter_index, &state.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Fetch chapter title from chapters table
    let chapter_row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT title FROM chapters WHERE book_id = ? AND chapter_index = ?")
            .bind(&ctx.book_id)
            .bind(chapter_index)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    let chapter_title = chapter_row.and_then(|r| r.0);

    // Fetch chapter summary
    let summary_repo = crate::storage::db::v4::chapter_repo::ChapterRepo::new(state.pool.clone());
    let summary_record = summary_repo
        .get_chapter_summary(&ctx.book_id, chapter_index)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let (summary, key_points) = if let Some(s) = summary_record {
        let kp: Vec<String> = s
            .key_points_json
            .as_deref()
            .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
            .unwrap_or_default();
        (Some(s.summary), kp)
    } else {
        (None, vec![])
    };

    // Fetch relationships that occurred in this chapter
    let relationship_repo = RelationshipRepo::new(state.pool.clone());
    let relationship_pairs = relationship_repo
        .list_relationships_by_chapter(&ctx.book_id, chapter_index)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let event_repo = RelationshipEventRepo::new(state.pool.clone());
    let identity_repo = IdentityRepo::new(state.pool.clone());
    let mut relationships_in_chapter = Vec::new();
    for (_ev, rel) in &relationship_pairs {
        let edge = relationship_projection::build_edge_for_relationship_in_book(
            &ctx.book_id,
            rel,
            &event_repo,
            &identity_repo,
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
        if let Some(edge) = edge {
            relationships_in_chapter.push(edge);
        }
    }
    let relationship_count = relationships_in_chapter.len() as i64;
    let knowledge_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*)
         FROM knowledge_assertions
         WHERE book_id = ? AND chapter_index = ?",
    )
    .bind(&ctx.book_id)
    .bind(chapter_index)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .0;

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4ChapterMemoryResponse {
            chapter_index,
            chapter_title,
            summary,
            key_points,
            characters_in_chapter: characters,
            relationships_in_chapter,
            relationship_count,
            knowledge_count,
        })
        .unwrap_or_default(),
    )))
}

/// GET /v4/memory/status — processing progress
pub async fn get_v4_memory_status(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let progress_repo = ProgressRepo::new(state.pool.clone());
    let progress = progress_repo
        .get_progress(&ctx.book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let max_read = get_max_read(&state, &ctx.book_id).await?;

    let (max_processed, processing, last_error) = if let Some(p) = progress {
        (
            p.max_processed_chapter,
            p.status == "running" || p.status == "cancel_requested",
            p.last_error,
        )
    } else {
        (0, false, None)
    };

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4MemoryStatusResponse {
            max_read_chapter: max_read,
            max_processed_chapter: max_processed,
            processing,
            last_error,
        })
        .unwrap_or_default(),
    )))
}

/// POST /v4/memory/reset — clear V4 canonical state (FK-safe order)
pub async fn post_v4_memory_reset(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    reset_v4(&state.pool, &ctx.book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(ApiResponse::ok(serde_json::json!({ "ok": true }))))
}

/// POST /v4/enabled — enable/disable V4 for a book
pub async fn post_v4_enabled(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4EnabledRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO book_memory_v4_settings (book_id, enabled, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT(book_id) DO UPDATE SET enabled = excluded.enabled, updated_at = excluded.updated_at"
    )
    .bind(&ctx.book_id)
    .bind(if req.enabled { 1i32 } else { 0i32 })
    .bind(&now)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(ApiResponse::ok(
        serde_json::json!({ "enabled": req.enabled }),
    )))
}

/// POST /v4/chapter-memory/generate — trigger single chapter processing
pub async fn post_v4_chapter_generate(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4ChapterMemoryRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let chapter_index = req
        .chapter_index
        .ok_or_else(|| AppError::BadRequest("chapterIndex required".to_string()))?;

    // Verify V4 is enabled
    ensure_v4_enabled(&state, &ctx.book_id).await?;

    // Get raw_text from chapters table
    let chapter_repo = crate::storage::db::v4::chapter_repo::ChapterRepo::new(state.pool.clone());
    let chapter = chapter_repo
        .get_chapter(&ctx.book_id, chapter_index)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let raw_text = match chapter {
        Some(ch) => ch.raw_text,
        None => {
            return Err(AppError::BadRequest(format!(
                "Chapter {} not found. Ensure reading progress is up to date.",
                chapter_index
            )));
        }
    };

    // Run pipeline
    let extractor =
        crate::service::v4::extractor::RealAiExtractor::new(state.ai_model_service.clone());
    let model_config = state
        .ai_model_service
        .get()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let model_name = model_config.text.model.clone();
    crate::service::v4::pipeline::process_chapter(
        &ctx.book_id,
        chapter_index,
        &raw_text,
        &state.pool,
        &extractor,
        Some(&model_name),
        None, // TODO: wire real AI judge when AiModelService integration is ready
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "ok": true,
        "chapter_index": chapter_index,
        "message": "Chapter processed successfully"
    }))))
}

/// POST /v4/catchup/start — trigger batch processing
pub async fn post_v4_catchup_start(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4CatchupStartRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let target_chapter = req
        .target_chapter_index
        .ok_or_else(|| AppError::BadRequest("targetChapterIndex required".to_string()))?;

    // Verify V4 is enabled
    ensure_v4_enabled(&state, &ctx.book_id).await?;

    // Concurrency control: check if already running
    let progress_repo = ProgressRepo::new(state.pool.clone());
    let progress = progress_repo
        .get_progress(&ctx.book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if let Some(ref p) = progress {
        if p.status == "running" || p.status == "cancel_requested" {
            return Err(AppError::BadRequest(
                "Catchup already running for this book".to_string(),
            ));
        }
    }

    // Set status to running
    progress_repo
        .set_status(
            &ctx.book_id,
            "running",
            Some(target_chapter),
            None,
            None,
            None,
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Spawn background worker
    let pool = state.pool.clone();
    let book_id = ctx.book_id.clone();
    let ai_model_service = state.ai_model_service.clone();
    tokio::spawn(async move {
        run_catchup_worker(&book_id, target_chapter, &pool, ai_model_service).await;
    });

    Ok(Json(ApiResponse::ok(serde_json::json!({
        "ok": true,
        "target_chapter": target_chapter,
    }))))
}

/// GET /v4/catchup/status — batch processing status
pub async fn get_v4_catchup_status(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let progress_repo = ProgressRepo::new(state.pool.clone());
    let progress = progress_repo
        .get_progress(&ctx.book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if let Some(p) = progress {
        Ok(Json(ApiResponse::ok(
            serde_json::to_value(V4CatchupStatusResponse {
                status: p.status,
                target_chapter: p.target_chapter,
                current_chapter: p.current_chapter,
                max_processed_chapter: p.max_processed_chapter,
                last_error: p.last_error,
            })
            .unwrap_or_default(),
        )))
    } else {
        Ok(Json(ApiResponse::ok(
            serde_json::to_value(V4CatchupStatusResponse {
                status: "idle".to_string(),
                target_chapter: None,
                current_chapter: None,
                max_processed_chapter: 0,
                last_error: None,
            })
            .unwrap_or_default(),
        )))
    }
}

/// POST /v4/catchup/cancel — cancel batch processing
pub async fn post_v4_catchup_cancel(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let progress_repo = ProgressRepo::new(state.pool.clone());
    let progress = progress_repo
        .get_progress(&ctx.book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    match progress {
        Some(ref p) if p.status == "running" => {
            progress_repo
                .set_status(&ctx.book_id, "cancel_requested", None, None, None, None)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            Ok(Json(ApiResponse::ok(serde_json::json!({ "ok": true }))))
        }
        Some(ref p) if p.status == "cancel_requested" => {
            Ok(Json(ApiResponse::ok(serde_json::json!({ "ok": true }))))
        }
        _ => Err(AppError::BadRequest(
            "No running catchup to cancel".to_string(),
        )),
    }
}

/// GET /v4/relationships — book-level relationship graph
pub async fn get_v4_relationships(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4RelationshipsRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req: V4RelationshipsRequest = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let rel_repo = RelationshipRepo::new(state.pool.clone());
    let relationships = rel_repo
        .list_by_book(&ctx.book_id, req.group.as_deref(), req.min_importance)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let event_repo = RelationshipEventRepo::new(state.pool.clone());
    let identity_repo = IdentityRepo::new(state.pool.clone());
    let mut edges = Vec::new();
    let mut entity_ids: HashSet<String> = HashSet::new();
    let mut groups_set: HashSet<String> = HashSet::new();
    for rel in &relationships {
        if let Some(edge) = relationship_projection::build_edge_for_relationship_in_book(
            &ctx.book_id,
            rel,
            &event_repo,
            &identity_repo,
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        {
            entity_ids.insert(edge.source_id.clone());
            entity_ids.insert(edge.target_id.clone());
            groups_set.insert(edge.group.clone());
            edges.push(edge);
        }
    }

    // Build nodes from entities
    let entity_repo = EntityRepo::new(state.pool.clone());
    let mut nodes = Vec::new();
    for eid in &entity_ids {
        if let Some(entity) = entity_repo
            .get_by_id(eid)
            .await
            .map_err(|e| AppError::Internal(e.into()))?
        {
            let aliases = entity_repo
                .list_aliases_by_entity(eid)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            nodes.push(relationship_projection::RelationshipNode {
                id: entity.id,
                name: entity.display_name,
                aliases: aliases.into_iter().map(|a| a.alias).collect(),
                importance: entity.importance_score,
                first_seen_chapter: entity.first_seen_chapter,
                last_seen_chapter: entity.last_seen_chapter,
            });
        }
    }

    let mut groups: Vec<String> = groups_set.into_iter().collect();
    groups.sort();
    let total = edges.len();

    let view = relationship_projection::RelationshipGraphView {
        nodes,
        edges,
        groups,
        total,
    };

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(view).unwrap_or_default(),
    )))
}

/// GET /v4/characters/:character_id/relationships — relationships for a specific character
pub async fn get_v4_character_relationships(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(character_id): Path<String>,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;

    let edges = relationship_projection::project_relationship_edges_for_character(
        &ctx.book_id,
        &character_id,
        &state.pool,
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    let total = edges.len();

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4CharacterRelationshipsResponse {
            relationships: edges,
            total,
        })
        .unwrap_or_default(),
    )))
}

/// GET /v4/identity-links — book-level identity debug links.
pub async fn get_v4_identity_links(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;
    let identity_repo = IdentityRepo::new(state.pool.clone());
    let links = identity_repo
        .list_identity_links_by_book(&ctx.book_id, None)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let identity_links = links
        .into_iter()
        .map(identity_link_to_view)
        .collect::<Vec<_>>();
    let total = identity_links.len();

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4IdentityLinksResponse {
            identity_links,
            total,
        })
        .unwrap_or_default(),
    )))
}

/// GET /v4/characters/:character_id/identity — identity debug view for one character.
pub async fn get_v4_character_identity(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(character_id): Path<String>,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;
    let identity_repo = IdentityRepo::new(state.pool.clone());
    let redirect_target_id = identity_repo
        .resolve_redirect_target(&ctx.book_id, &character_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let links = identity_repo
        .list_identity_links_by_book(&ctx.book_id, None)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let identity_links = links
        .into_iter()
        .filter(|link| {
            link.entity_a_id == character_id
                || link.entity_b_id == character_id
                || redirect_target_id.as_ref().is_some_and(|target_id| {
                    link.entity_a_id == *target_id || link.entity_b_id == *target_id
                })
        })
        .map(identity_link_to_view)
        .collect::<Vec<_>>();
    let total = identity_links.len();

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4CharacterIdentityResponse {
            character_id,
            redirect_target_id,
            identity_links,
            total,
        })
        .unwrap_or_default(),
    )))
}

/// GET /v4/merge-operations — book-level merge operation debug list.
pub async fn get_v4_merge_operations(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;
    let identity_repo = IdentityRepo::new(state.pool.clone());
    let operations = identity_repo
        .list_merge_operations_by_book(&ctx.book_id, None)
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .into_iter()
        .map(merge_operation_to_view)
        .collect::<Vec<_>>();
    let total = operations.len();

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(V4MergeOperationsResponse {
            merge_operations: operations,
            total,
        })
        .unwrap_or_default(),
    )))
}

/// GET /v4/knowledge — book-level knowledge overview.
pub async fn get_v4_knowledge(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;
    let max_processed = get_max_processed(&state, &ctx.book_id).await?;
    let view =
        knowledge_projection::project_knowledge_overview(&ctx.book_id, max_processed, &state.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(view).unwrap_or_default(),
    )))
}

/// GET /v4/knowledge/cards/:card_id — knowledge card detail.
pub async fn get_v4_knowledge_card(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(card_id): Path<String>,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;
    let max_processed = get_max_processed(&state, &ctx.book_id).await?;
    let view = knowledge_projection::project_knowledge_card_detail(
        &ctx.book_id,
        &card_id,
        max_processed,
        &state.pool,
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(view).unwrap_or_default(),
    )))
}

/// GET /v4/knowledge/categories/:category — category-filtered knowledge cards.
pub async fn get_v4_knowledge_category(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(category): Path<String>,
    Query(q): Query<V4BookRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let req = parse_request(q, body)?;
    let ctx = resolve_v4_book(&state, &auth, req.book_url).await?;
    let max_processed = get_max_processed(&state, &ctx.book_id).await?;
    let view = knowledge_projection::project_knowledge_category(
        &ctx.book_id,
        &category,
        max_processed,
        &state.pool,
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(ApiResponse::ok(
        serde_json::to_value(view).unwrap_or_default(),
    )))
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

struct V4BookCtx {
    book_id: String,
    #[allow(dead_code)]
    user_ns: String,
}

fn parse_request<T: serde::de::DeserializeOwned + Default>(
    q: T,
    body: Bytes,
) -> Result<T, AppError> {
    if body.is_empty() {
        return Ok(q);
    }
    if let Ok(v) = serde_json::from_slice::<T>(&body) {
        return Ok(v);
    }
    Ok(q)
}

fn identity_link_to_view(link: IdentityLinkRecord) -> V4IdentityLinkView {
    let redirect_target_id = if link.link_type == "redirect" {
        Some(link.entity_b_id.clone())
    } else {
        None
    };
    V4IdentityLinkView {
        id: link.id,
        entity_a_id: link.entity_a_id,
        entity_b_id: link.entity_b_id,
        link_type: link.link_type,
        status: link.status,
        confidence: link.confidence,
        source_claim_id: link.source_claim_id,
        redirect_target_id,
    }
}

fn merge_operation_to_view(operation: MergeOperationRecord) -> V4MergeOperationView {
    V4MergeOperationView {
        id: operation.id,
        survivor_entity_id: operation.survivor_entity_id,
        victim_entity_id: operation.victim_entity_id,
        source_identity_link_id: operation.source_identity_link_id,
        reason_code: operation.reason_code,
        confidence: operation.confidence,
        status: operation.status,
        property_conflict_count: operation.property_conflict_count,
        relationship_merge_count: operation.relationship_merge_count,
        result_json: operation.result_json,
        created_at: operation.created_at,
        completed_at: operation.completed_at,
    }
}

async fn resolve_v4_book(
    state: &AppState,
    auth: &AuthContext,
    book_url: Option<String>,
) -> Result<V4BookCtx, AppError> {
    let user_ns = state
        .user_service
        .resolve_user_ns_with_override(auth.access_token(), auth.secure_key(), auth.user_ns())
        .await
        .map_err(|_| AppError::BadRequest("NEED_LOGIN".to_string()))?;

    let book_url = book_url
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("bookUrl required".to_string()))?;
    let book_url = repair_encoded_url(&book_url);

    // V4 book_id = md5 of book_url (consistent with V3 key derivation)
    let book_id = crate::util::hash::md5_hex(&book_url);

    // Ensure reading_progress exists
    sqlx::query("INSERT OR IGNORE INTO reading_progress (book_id, max_read_chapter) VALUES (?, 0)")
        .bind(&book_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    // Ensure processing_progress exists
    let progress_repo = ProgressRepo::new(state.pool.clone());
    progress_repo
        .init_progress(&book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(V4BookCtx { book_id, user_ns })
}

async fn get_max_processed(state: &AppState, book_id: &str) -> Result<i64, AppError> {
    let progress_repo = ProgressRepo::new(state.pool.clone());
    let progress = progress_repo
        .get_progress(book_id)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    Ok(progress.map(|p| p.max_processed_chapter).unwrap_or(0))
}

async fn get_max_read(state: &AppState, book_id: &str) -> Result<i64, AppError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT max_read_chapter FROM reading_progress WHERE book_id = ?")
            .bind(book_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    Ok(row.map(|r| r.0).unwrap_or(0))
}

async fn ensure_v4_enabled(state: &AppState, book_id: &str) -> Result<(), AppError> {
    let row: Option<(i32,)> =
        sqlx::query_as("SELECT enabled FROM book_memory_v4_settings WHERE book_id = ?")
            .bind(book_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;

    match row {
        Some((enabled,)) if enabled == 1 => Ok(()),
        Some(_) => Err(AppError::BadRequest(
            "V4 is disabled for this book".to_string(),
        )),
        None => {
            // Default: enabled
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT OR IGNORE INTO book_memory_v4_settings (book_id, enabled, updated_at) VALUES (?, 1, ?)"
            )
            .bind(book_id)
            .bind(&now)
            .execute(&state.pool)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Catchup Worker
// ---------------------------------------------------------------------------

/// Background worker that processes chapters sequentially.
/// Checks `processing_progress.status` for cooperative cancellation.
async fn run_catchup_worker(
    book_id: &str,
    target_chapter: i64,
    pool: &sqlx::SqlitePool,
    ai_model_service: std::sync::Arc<crate::service::ai_model_service::AiModelService>,
) {
    let progress_repo = ProgressRepo::new(pool.clone());
    let chapter_repo = crate::storage::db::v4::chapter_repo::ChapterRepo::new(pool.clone());
    let extractor = crate::service::v4::extractor::RealAiExtractor::new(ai_model_service.clone());
    let model_name = ai_model_service
        .get()
        .await
        .map(|c| c.text.model)
        .unwrap_or_default();

    // Get current progress
    let start_chapter = match progress_repo.get_progress(book_id).await {
        Ok(Some(p)) => p.max_processed_chapter + 1,
        _ => 1,
    };

    for chapter_index in start_chapter..=target_chapter {
        // Check for cooperative cancellation
        match progress_repo.get_progress(book_id).await {
            Ok(Some(p)) if p.status == "cancel_requested" => {
                let _ = progress_repo
                    .set_status(book_id, "cancelled", None, None, None, None)
                    .await;
                return;
            }
            Err(e) => {
                tracing::error!("Failed to check progress for {}: {}", book_id, e);
                let _ = progress_repo
                    .set_status(
                        book_id,
                        "failed",
                        None,
                        None,
                        None,
                        Some(&format!("Progress check failed: {}", e)),
                    )
                    .await;
                return;
            }
            _ => {}
        }

        // Update current chapter
        let _ = progress_repo
            .set_status(
                book_id,
                "running",
                Some(target_chapter),
                Some(chapter_index),
                None,
                None,
            )
            .await;

        // Get raw_text from chapters table
        let chapter = match chapter_repo.get_chapter(book_id, chapter_index).await {
            Ok(Some(ch)) => ch,
            Ok(None) => {
                tracing::warn!(
                    "Chapter {} not found for book {}, skipping",
                    chapter_index,
                    book_id
                );
                continue;
            }
            Err(e) => {
                tracing::error!(
                    "Failed to fetch chapter {} for {}: {}",
                    chapter_index,
                    book_id,
                    e
                );
                let _ = progress_repo
                    .set_status(
                        book_id,
                        "failed",
                        None,
                        None,
                        None,
                        Some(&format!("Chapter fetch failed: {}", e)),
                    )
                    .await;
                return;
            }
        };

        // Process chapter through pipeline
        match crate::service::v4::pipeline::process_chapter(
            book_id,
            chapter_index,
            &chapter.raw_text,
            pool,
            &extractor,
            Some(&model_name),
            None, // TODO: wire real AI judge
        )
        .await
        {
            Ok(()) => {
                tracing::info!("Processed chapter {} for book {}", chapter_index, book_id);
            }
            Err(e) => {
                tracing::error!(
                    "Failed to process chapter {} for {}: {}",
                    chapter_index,
                    book_id,
                    e
                );
                let _ = progress_repo
                    .set_status(
                        book_id,
                        "failed",
                        None,
                        None,
                        None,
                        Some(&format!(
                            "Pipeline failed at chapter {}: {}",
                            chapter_index, e
                        )),
                    )
                    .await;
                return;
            }
        }
    }

    let _ = progress_repo
        .set_status(book_id, "completed", None, None, None, None)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup_pool() -> sqlx::SqlitePool {
        let dir = std::env::temp_dir().join(format!("v4-api-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        db::v4::init_v4(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn v4_memory_status_default() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("test-book").await.unwrap();

        let progress = progress_repo.get_progress("test-book").await.unwrap();
        assert!(progress.is_some());
        let p = progress.unwrap();
        assert_eq!(p.max_processed_chapter, 0);
        assert_eq!(p.status, "idle");
        assert!(p.last_error.is_none());
    }

    #[tokio::test]
    async fn v4_catchup_status_state_machine() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("test-book").await.unwrap();

        // Set to running
        progress_repo
            .set_status("test-book", "running", Some(10), Some(1), None, None)
            .await
            .unwrap();

        let p = progress_repo
            .get_progress("test-book")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.status, "running");
        assert_eq!(p.target_chapter, Some(10));
        assert_eq!(p.current_chapter, Some(1));

        // Advance
        progress_repo
            .advance_processed_chapter("test-book", 1)
            .await
            .unwrap();

        let p = progress_repo
            .get_progress("test-book")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.max_processed_chapter, 1);

        // Cancel
        progress_repo
            .set_status("test-book", "cancel_requested", None, None, None, None)
            .await
            .unwrap();

        let p = progress_repo
            .get_progress("test-book")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.status, "cancel_requested");

        // Set to cancelled
        progress_repo
            .set_status("test-book", "cancelled", None, None, None, None)
            .await
            .unwrap();

        let p = progress_repo
            .get_progress("test-book")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.status, "cancelled");
    }

    #[tokio::test]
    async fn v4_reset_clears_entities_preserves_chapters() {
        let pool = setup_pool().await;

        // Insert chapter (should survive reset)
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at)
             VALUES (?, 'b1', 1, 'text', 'hash', datetime('now'))",
        )
        .bind(&chapter_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert entity (should be cleared)
        sqlx::query(
            "INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at)
             VALUES ('e1', 'b1', 'character', 'Test', 'Test', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Reset
        reset_v4(&pool, "b1").await.unwrap();

        // Chapter preserved
        let ch_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chapters WHERE book_id = 'b1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(ch_count.0, 1, "chapters should be preserved");

        // Entity cleared
        let ent_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM entities WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(ent_count.0, 0, "entities should be cleared");
    }

    #[tokio::test]
    async fn v4_characters_empty_on_fresh_book() {
        let pool = setup_pool().await;

        let characters = projection::project_character_list("fresh-book", 0, &pool)
            .await
            .unwrap();
        assert!(characters.is_empty());
    }

    #[tokio::test]
    async fn v4_phase2_phase4_fields_zero() {
        // Verify that V4MemoryResponse serializes with expected fields
        let resp = V4MemoryResponse {
            book_url: "http://example.com/book.txt".to_string(),
            book_name: "Test Book".to_string(),
            author: "Author".to_string(),
            max_read_chapter: 10,
            max_processed_chapter: 5,
            processing: false,
            character_count: 3,
            relationship_count: 0,
            knowledge_count: 0,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["relationshipCount"], 0);
        assert_eq!(json["knowledgeCount"], 0);
        assert_eq!(json["characterCount"], 3);
        assert_eq!(json["maxReadChapter"], 10);
        assert_eq!(json["maxProcessedChapter"], 5);
        assert_eq!(json["processing"], false);
        assert_eq!(json["bookUrl"], "http://example.com/book.txt");
    }

    #[tokio::test]
    async fn catchup_worker_runs_to_completion() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        // Simulate catchup from chapter 1 to 3
        progress_repo
            .set_status("b1", "running", Some(3), None, None, None)
            .await
            .unwrap();

        for ch in 1..=3i64 {
            progress_repo
                .advance_processed_chapter("b1", ch)
                .await
                .unwrap();
        }
        progress_repo
            .set_status("b1", "completed", None, None, None, None)
            .await
            .unwrap();

        let p = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(p.status, "completed");
        assert_eq!(p.max_processed_chapter, 3);
    }

    #[tokio::test]
    async fn catchup_cooperative_cancel() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        // Start running
        progress_repo
            .set_status("b1", "running", Some(10), Some(2), None, None)
            .await
            .unwrap();

        // Request cancel
        progress_repo
            .set_status("b1", "cancel_requested", None, None, None, None)
            .await
            .unwrap();

        // Worker checks status
        let p = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(p.status, "cancel_requested");

        // Worker acknowledges cancel
        progress_repo
            .set_status("b1", "cancelled", None, None, None, None)
            .await
            .unwrap();

        let p = progress_repo.get_progress("b1").await.unwrap().unwrap();
        assert_eq!(p.status, "cancelled");
    }

    #[tokio::test]
    async fn v4_enabled_settings() {
        let pool = setup_pool().await;

        // Default: no settings
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT enabled FROM book_memory_v4_settings WHERE book_id = 'b1'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(row.is_none());

        // Enable
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO book_memory_v4_settings (book_id, enabled, updated_at) VALUES ('b1', 1, ?)"
        )
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let row: (i32,) =
            sqlx::query_as("SELECT enabled FROM book_memory_v4_settings WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, 1);

        // Disable
        sqlx::query(
            "UPDATE book_memory_v4_settings SET enabled = 0, updated_at = ? WHERE book_id = 'b1'",
        )
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let row: (i32,) =
            sqlx::query_as("SELECT enabled FROM book_memory_v4_settings WHERE book_id = 'b1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, 0);
    }

    #[tokio::test]
    async fn concurrent_catchup_rejected_while_running() {
        let pool = setup_pool().await;
        let progress_repo = ProgressRepo::new(pool.clone());
        progress_repo.init_progress("b1").await.unwrap();

        // Simulate first catchup is running
        progress_repo
            .set_status("b1", "running", Some(10), Some(1), None, None)
            .await
            .unwrap();

        // Second catchup attempt should detect running state
        let progress = progress_repo.get_progress("b1").await.unwrap();
        assert!(progress.is_some());
        let p = progress.unwrap();
        let already_running = p.status == "running" || p.status == "cancel_requested";
        assert!(
            already_running,
            "should detect that catchup is already running"
        );

        // Also verify cancel_requested blocks
        progress_repo
            .set_status("b1", "cancel_requested", Some(10), Some(3), None, None)
            .await
            .unwrap();

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        let already_running = progress.status == "running" || progress.status == "cancel_requested";
        assert!(
            already_running,
            "cancel_requested should also block new catchup"
        );

        // After completion, new catchup should be allowed
        progress_repo
            .set_status("b1", "completed", None, None, None, None)
            .await
            .unwrap();

        let progress = progress_repo.get_progress("b1").await.unwrap().unwrap();
        let blocked = progress.status == "running" || progress.status == "cancel_requested";
        assert!(!blocked, "completed status should allow new catchup");
    }

    #[tokio::test]
    async fn v4_character_list_view_serialization() {
        // Verify CharacterListView wrapper serializes correctly
        let view = V4CharacterListView {
            characters: vec![],
            total: 0,
        };
        let json = serde_json::to_value(&view).unwrap();
        assert!(json["characters"].is_array());
        assert_eq!(json["total"], 0);
    }

    #[tokio::test]
    async fn v4_character_response_has_new_fields() {
        // Verify V4CharacterResponse includes recent_changes and evidence_available
        use crate::service::v4::projection::CharacterCardView;
        use std::collections::HashMap;

        let resp = V4CharacterResponse {
            character: CharacterCardView {
                id: "c1".to_string(),
                name: "Test".to_string(),
                aliases: vec![],
                summary: None,
                importance: 0.5,
                first_seen_chapter: 1,
                last_seen_chapter: 1,
                current_states: HashMap::new(),
            },
            relationship_count: 0,
            recent_changes: vec![],
            evidence_available: false,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["recentChanges"].is_array());
        assert_eq!(json["recentChanges"].as_array().unwrap().len(), 0);
        assert_eq!(json["evidenceAvailable"], false);
    }

    #[tokio::test]
    async fn v4_chapter_memory_response_serialization() {
        // Verify V4ChapterMemoryResponse has the correct fields
        let resp = V4ChapterMemoryResponse {
            chapter_index: 1,
            chapter_title: Some("Chapter One".to_string()),
            summary: Some("Summary text".to_string()),
            key_points: vec!["point 1".to_string(), "point 2".to_string()],
            characters_in_chapter: vec![],
            relationships_in_chapter: vec![],
            relationship_count: 0,
            knowledge_count: 0,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["chapterIndex"], 1);
        assert_eq!(json["chapterTitle"], "Chapter One");
        assert_eq!(json["summary"], "Summary text");
        assert!(json["keyPoints"].is_array());
        assert_eq!(json["keyPoints"].as_array().unwrap().len(), 2);
        assert!(json["charactersInChapter"].is_array());
        assert!(json["relationshipsInChapter"].is_array());
    }

    #[test]
    fn v4_relationships_request_deserializes_with_filters() {
        // With all filters
        let req: V4RelationshipsRequest = serde_json::from_str(
            r#"{"bookUrl":"http://example.com/book.txt","group":"friendship","minImportance":0.5}"#,
        )
        .unwrap();
        assert_eq!(req.book_url.as_deref(), Some("http://example.com/book.txt"));
        assert_eq!(req.group.as_deref(), Some("friendship"));
        assert!((req.min_importance.unwrap() - 0.5).abs() < f64::EPSILON);

        // With only bookUrl
        let req: V4RelationshipsRequest =
            serde_json::from_str(r#"{"bookUrl":"http://example.com/book.txt"}"#).unwrap();
        assert_eq!(req.book_url.as_deref(), Some("http://example.com/book.txt"));
        assert!(req.group.is_none());
        assert!(req.min_importance.is_none());

        // Empty/default
        let req = V4RelationshipsRequest::default();
        assert!(req.book_url.is_none());
        assert!(req.group.is_none());
        assert!(req.min_importance.is_none());
    }

    #[test]
    fn v4_character_relationships_response_serialization() {
        let resp = V4CharacterRelationshipsResponse {
            relationships: vec![],
            total: 0,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["relationships"].is_array());
        assert_eq!(json["relationships"].as_array().unwrap().len(), 0);
        assert_eq!(json["total"], 0);
    }

    #[test]
    fn v4_identity_debug_responses_serialize_camel_case() {
        let link = V4IdentityLinkView {
            id: "l1".to_string(),
            entity_a_id: "victim".to_string(),
            entity_b_id: "survivor".to_string(),
            link_type: "redirect".to_string(),
            status: "active".to_string(),
            confidence: 0.96,
            source_claim_id: "claim-1".to_string(),
            redirect_target_id: Some("survivor".to_string()),
        };
        let links = V4IdentityLinksResponse {
            identity_links: vec![link.clone()],
            total: 1,
        };
        let character = V4CharacterIdentityResponse {
            character_id: "victim".to_string(),
            redirect_target_id: Some("survivor".to_string()),
            identity_links: vec![link],
            total: 1,
        };
        let operations = V4MergeOperationsResponse {
            merge_operations: vec![V4MergeOperationView {
                id: "op-1".to_string(),
                survivor_entity_id: "survivor".to_string(),
                victim_entity_id: "victim".to_string(),
                source_identity_link_id: "l1".to_string(),
                reason_code: "explicit_reveal".to_string(),
                confidence: 0.96,
                status: "completed".to_string(),
                property_conflict_count: 0,
                relationship_merge_count: 2,
                result_json: Some(r#"{"ok":true}"#.to_string()),
                created_at: "2026-07-03T00:00:00Z".to_string(),
                completed_at: Some("2026-07-03T00:00:01Z".to_string()),
            }],
            total: 1,
        };

        let links_json = serde_json::to_value(&links).unwrap();
        assert!(links_json["identityLinks"].is_array());
        assert_eq!(
            links_json["identityLinks"][0]["redirectTargetId"],
            "survivor"
        );

        let character_json = serde_json::to_value(&character).unwrap();
        assert_eq!(character_json["characterId"], "victim");
        assert_eq!(character_json["redirectTargetId"], "survivor");

        let operations_json = serde_json::to_value(&operations).unwrap();
        assert!(operations_json["mergeOperations"].is_array());
        assert_eq!(
            operations_json["mergeOperations"][0]["relationshipMergeCount"],
            2
        );
    }

    #[test]
    fn v4_relationships_request_deserializes_with_url_alias() {
        // Test the `url` alias for bookUrl
        let req: V4RelationshipsRequest =
            serde_json::from_str(r#"{"url":"http://example.com/book.txt","group":"rivalry"}"#)
                .unwrap();
        assert_eq!(req.book_url.as_deref(), Some("http://example.com/book.txt"));
        assert_eq!(req.group.as_deref(), Some("rivalry"));
    }
}
