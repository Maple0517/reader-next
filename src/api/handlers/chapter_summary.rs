use axum::{
    extract::{Query, State},
    Json,
};
use reqwest::Client;
use serde_json::Value;

use crate::api::{auth::AuthContext, AppState};
use crate::error::error::{ApiResponse, AppError};
use crate::model::chapter_summary::{
    GenerateChapterSummaryRequest, GetChapterSummaryQuery, SaveChapterSummaryConfigRequest,
};

pub async fn get_chapter_summary(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(query): Query<GetChapterSummaryQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let summary = state
        .chapter_summary_service
        .get_summary(&user_ns, &query.book_url, &query.chapter_url)
        .await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "summary": summary }))))
}

pub async fn generate_chapter_summary(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<GenerateChapterSummaryRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    if req.book_url.trim().is_empty() || req.chapter_url.trim().is_empty() {
        return Err(AppError::BadRequest("bookUrl and chapterUrl required".to_string()));
    }
    let can_use = state
        .user_service
        .can_use_ai_model(auth.access_token(), auth.secure_key())
        .await?;
    if !can_use {
        return Err(AppError::BadRequest(
            "当前账号没有使用后端模型配置的权限".to_string(),
        ));
    }
    let ai_config = state.ai_model_service.get().await?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(
            state.config.request_timeout_secs.max(15),
        ))
        .build()?;
    let summary = state
        .chapter_summary_service
        .generate_summary(&user_ns, req, ai_config, &client)
        .await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "summary": summary }))))
}

pub async fn get_chapter_summary_config(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let is_admin = is_chapter_summary_admin(&state, &auth).await?;
    let can_use_server_model = state
        .user_service
        .can_use_ai_model(auth.access_token(), auth.secure_key())
        .await?;
    let config = state.chapter_summary_service.get_config().await?;
    let visible = if is_admin {
        config
    } else {
        config.without_admin_fields()
    };
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "config": visible,
        "canUseServerModel": can_use_server_model,
        "isAdmin": is_admin,
    }))))
}

pub async fn save_chapter_summary_config(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<SaveChapterSummaryConfigRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    if !is_chapter_summary_admin(&state, &auth).await? {
        return Ok(Json(ApiResponse::err_with_data(
            "请输入管理密码",
            Value::String("NEED_SECURE_KEY".to_string()),
        )));
    }
    let config = state.chapter_summary_service.save_config(req.config).await?;
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "config": config,
        "canUseServerModel": true,
        "isAdmin": true,
    }))))
}

async fn resolve_user_ns(state: &AppState, auth: &AuthContext) -> Result<String, AppError> {
    state
        .user_service
        .resolve_user_ns_with_override(auth.access_token(), auth.secure_key(), auth.user_ns())
        .await
        .map_err(|_| AppError::BadRequest("NEED_LOGIN".to_string()))
}

async fn is_chapter_summary_admin(state: &AppState, auth: &AuthContext) -> Result<bool, AppError> {
    if !state.user_service.secure_enabled() {
        return Ok(true);
    }
    state
        .user_service
        .is_admin(auth.access_token(), auth.secure_key())
        .await
}
