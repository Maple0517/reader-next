use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::{auth::AuthContext, AppState};
use crate::error::error::{ApiResponse, AppError};
use crate::model::world_map::*;
use crate::service::world_map_builder::WorldMapBuilderService;

// ==================== Request/Response 结构体 ====================

#[derive(Debug, Deserialize)]
pub struct WorldMapRequest {
    pub book_url: String,
}

#[derive(Debug, Deserialize)]
pub struct BuildWorldMapRequest {
    pub book_url: String,
    pub novel_title: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorldMapRequest {
    pub book_url: String,
    pub end_chapter: i32,
}

#[derive(Debug, Serialize)]
pub struct UpdateWorldMapResponse {
    pub spec: WorldMapSpec,
    pub added_entities: usize,
    pub added_relations: usize,
}

#[derive(Debug, Deserialize)]
pub struct GenerateCoordinatesRequest {
    pub book_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveReviewRequest {
    pub book_url: String,
    pub item_id: String,
    pub resolution: String,
    pub comment: Option<String>,
}

// ==================== API Handlers ====================

/// 获取地图规格书
pub async fn get_world_map_spec(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(req): Query<WorldMapRequest>,
) -> Result<Json<ApiResponse<WorldMapSpec>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;

    let service = WorldMapBuilderService::new(&state.config.storage_dir);

    let spec = service
        .load(&user_ns, &req.book_url)
        .await?
        .ok_or_else(|| AppError::NotFound("世界地图规格书不存在".to_string()))?;

    Ok(Json(ApiResponse::ok(spec)))
}

/// 构建地图规格书（从 mock 数据）
pub async fn build_world_map(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<BuildWorldMapRequest>,
) -> Result<Json<ApiResponse<WorldMapSpec>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;

    let service = WorldMapBuilderService::new(&state.config.storage_dir);

    let spec = service
        .build_from_mock(&user_ns, &req.book_url, &req.novel_title)
        .await?;

    Ok(Json(ApiResponse::ok(spec)))
}

/// 保存地图规格书
pub async fn save_world_map_spec(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(spec): Json<WorldMapSpec>,
) -> Result<Json<ApiResponse<WorldMapSpec>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;

    let service = WorldMapBuilderService::new(&state.config.storage_dir);

    // 从 spec 中提取 book_url
    let book_url = &spec.metadata.novel_title;
    service.save(&user_ns, book_url, &spec).await?;

    Ok(Json(ApiResponse::ok(spec)))
}

/// 增量更新（新章节）
pub async fn update_world_map(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<UpdateWorldMapRequest>,
) -> Result<Json<ApiResponse<UpdateWorldMapResponse>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;

    let service = WorldMapBuilderService::new(&state.config.storage_dir);

    // 加载现有 spec
    let existing = service
        .load(&user_ns, &req.book_url)
        .await?
        .ok_or_else(|| AppError::NotFound("世界地图规格书不存在".to_string()))?;

    let old_entity_count = existing.entities.len();
    let old_relation_count = existing.relations.len();

    // TODO: 实现增量更新逻辑
    let updated = existing;

    let response = UpdateWorldMapResponse {
        added_entities: updated.entities.len() - old_entity_count,
        added_relations: updated.relations.len() - old_relation_count,
        spec: updated,
    };

    Ok(Json(ApiResponse::ok(response)))
}

/// 生成坐标
pub async fn generate_coordinates(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<GenerateCoordinatesRequest>,
) -> Result<Json<ApiResponse<WorldMapCoordinates>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;

    let service = WorldMapBuilderService::new(&state.config.storage_dir);

    let coords = service
        .generate_coordinates(&user_ns, &req.book_url)
        .await?;

    Ok(Json(ApiResponse::ok(coords)))
}

/// 获取审查清单
pub async fn get_review_items(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(req): Query<WorldMapRequest>,
) -> Result<Json<ApiResponse<Vec<WorldMapReviewItem>>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;

    let service = WorldMapBuilderService::new(&state.config.storage_dir);

    let items = service.get_review_items(&user_ns, &req.book_url).await?;

    Ok(Json(ApiResponse::ok(items)))
}

/// 人工修正审查项
pub async fn resolve_review_item(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<ResolveReviewRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let _user_ns = resolve_user_ns(&state, &auth).await?;

    // TODO: 实现修正逻辑
    Ok(Json(ApiResponse::ok(format!(
        "已标记审查项 {} 为 {}",
        req.item_id, req.resolution
    ))))
}

async fn resolve_user_ns(state: &AppState, auth: &AuthContext) -> Result<String, AppError> {
    state
        .user_service
        .resolve_user_ns_with_override(auth.access_token(), auth.secure_key(), auth.user_ns())
        .await
        .map_err(|_| AppError::BadRequest("NEED_LOGIN".to_string()))
}
