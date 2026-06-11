use axum::{
    body::Bytes,
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use crate::api::{auth::AuthContext, AppState};
use crate::error::error::{ApiResponse, AppError};
use crate::util::text::repair_encoded_url;

#[derive(Debug, Deserialize, Default)]
pub struct AiBookMemoryRequest {
    #[serde(rename = "bookUrl", alias = "url")]
    pub book_url: Option<String>,
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
    ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let memory = state.ai_book_service.get_value(&user_ns, &book_url).await?;
    Ok(Json(ApiResponse::ok(serde_json::to_value(memory).unwrap_or_default())))
}

pub async fn save_ai_book_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(mut memory): Json<Value>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let book_url = required_book_url(read_json_string(&memory, "bookUrl"))?;
    let shelf_book = ensure_shelf_book(&state, &user_ns, &book_url).await?;
    set_json_string_if_empty(&mut memory, "bookName", shelf_book.name)?;
    set_json_string_if_empty(&mut memory, "author", shelf_book.author)?;
    let saved = state
        .ai_book_service
        .save_value_for_book(&user_ns, &book_url, memory)
        .await?;
    Ok(Json(ApiResponse::ok(saved)))
}

pub async fn delete_ai_book_memory(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<AiBookMemoryRequest>,
    body: Bytes,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let user_ns = resolve_user_ns(&state, &auth).await?;
    let req = parse_ai_book_request(q, body)?;
    let book_url = required_book_url(req.book_url)?;
    ensure_shelf_book(&state, &user_ns, &book_url).await?;
    let deleted = state.ai_book_service.delete(&user_ns, &book_url).await?;
    Ok(Json(ApiResponse::ok(
        serde_json::json!({ "deleted": deleted }),
    )))
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

fn required_book_url(book_url: Option<String>) -> Result<String, AppError> {
    let book_url = book_url
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("bookUrl required".to_string()))?;
    Ok(repair_encoded_url(&book_url))
}

fn read_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn set_json_string_if_empty(value: &mut Value, key: &str, next: String) -> Result<(), AppError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("AI memory must be a JSON object".to_string()))?;
    let current = object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if current.is_empty() {
        object.insert(key.to_string(), Value::String(next));
    }
    Ok(())
}
