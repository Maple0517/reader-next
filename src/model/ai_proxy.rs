use crate::model::ai_model::AiModelKind;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use url::Url;

const AI_PROXY_TIMEOUT_SECS: u64 = 300;
const OPENAI_COMPATIBLE_SUFFIXES: [&str; 4] = [
    "/chat/completions",
    "/images/generations",
    "/audio/speech",
    "/responses",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProxyRequest {
    #[serde(default)]
    pub base_url: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub full_url: bool,
    #[serde(default)]
    pub use_server_config: bool,
    pub kind: Option<AiModelKind>,
    pub body: Value,
}

#[derive(Debug, Deserialize)]
pub struct AiProxyImageRequest {
    pub url: String,
}

pub fn build_ai_proxy_url(base_url: &str, path: &str, full_url: bool) -> Result<Url, String> {
    if full_url {
        return parse_http_url(base_url);
    }

    let proxy_path = parse_ai_proxy_path(path)?;

    let mut base = parse_http_url(base_url)?;
    let joined_path = join_ai_proxy_path(base.path(), &proxy_path.path);
    base.set_path(&joined_path);
    base.set_query(proxy_path.query.as_deref());
    base.set_fragment(None);
    Ok(base)
}

pub fn validate_ai_proxy_image_url(url: &str) -> Result<Url, String> {
    parse_http_url(url)
}

pub fn ai_proxy_timeout() -> Duration {
    Duration::from_secs(AI_PROXY_TIMEOUT_SECS)
}

pub fn format_ai_proxy_upstream_error(status: u16, body: &str) -> String {
    let reason = http::StatusCode::from_u16(status)
        .ok()
        .and_then(|status| status.canonical_reason())
        .unwrap_or("Upstream Error");
    let detail = extract_error_detail(body);
    if detail.is_empty() {
        return format!("模型服务返回 {} {}", status, reason);
    }
    format!("模型服务返回 {} {}：{}", status, reason, detail)
}

fn parse_http_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw.trim()).map_err(|e| e.to_string())?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        _ => Err("only http/https proxy targets are supported".to_string()),
    }
}

struct ParsedAiProxyPath {
    path: String,
    query: Option<String>,
}

fn parse_ai_proxy_path(raw: &str) -> Result<ParsedAiProxyPath, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("proxy path must not be empty".to_string());
    }
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return Err("proxy path must start with /".to_string());
    }
    if trimmed.contains('#') {
        return Err("proxy path must not contain fragment".to_string());
    }

    let url = Url::parse(&format!("http://reader.local{trimmed}")).map_err(|e| e.to_string())?;
    let mut path = url.path().to_string();
    if path == "/v1/response" {
        // ponytail: tolerate common Responses API typo from saved/browser configs.
        path = "/v1/responses".to_string();
    }
    if path
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err("proxy path must not contain path traversal".to_string());
    }
    if !is_allowed_ai_proxy_path(&path) {
        return Err(format!("unsupported proxy path: {}", path));
    }

    Ok(ParsedAiProxyPath {
        path,
        query: url.query().map(str::to_string),
    })
}

fn is_allowed_ai_proxy_path(path: &str) -> bool {
    if path == "/v1/messages" {
        return true;
    }

    if (path.starts_with("/v1/") || path.starts_with("/v1beta/"))
        && OPENAI_COMPATIBLE_SUFFIXES
            .iter()
            .any(|suffix| path.ends_with(suffix))
    {
        return true;
    }

    if (path.starts_with("/v1/models/") || path.starts_with("/v1beta/models/"))
        && (path.ends_with(":generateContent") || path.ends_with(":streamGenerateContent"))
    {
        return true;
    }

    false
}

fn join_ai_proxy_path(base_path: &str, proxy_path: &str) -> String {
    let base_path = base_path.trim_end_matches('/');
    if base_path.ends_with("/v1")
        && proxy_path.starts_with("/v1/")
        && proxy_path.len() > "/v1".len()
    {
        return format!("{}{}", base_path, &proxy_path["/v1".len()..]);
    }
    if (base_path.ends_with("/v1/openai") || base_path.ends_with("/v1beta/openai"))
        && proxy_path.starts_with("/v1/")
        && proxy_path.len() > "/v1".len()
    {
        return format!("{}{}", base_path, &proxy_path["/v1".len()..]);
    }
    format!("{}{}", base_path, proxy_path)
}

fn extract_error_detail(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(message) = value
            .pointer("/error/message")
            .or_else(|| value.get("errorMsg"))
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
        {
            return truncate_error_detail(message);
        }
    }

    truncate_error_detail(&strip_html_tags(body))
}

fn strip_html_tags(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_error_detail(value: &str) -> String {
    let cleaned = value.trim();
    if cleaned.chars().count() <= 240 {
        return cleaned.to_string();
    }
    let mut result = cleaned.chars().take(240).collect::<String>();
    result.push('…');
    result
}
