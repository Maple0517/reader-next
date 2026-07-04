use crate::service::ai_model_service::AiModelService;
use crate::service::v4::extractor::VALID_PLACE_EDGE_TYPES;
use crate::storage::db::v4::ai_run_repo::AiRunRepo;
use crate::util::hash::md5_hex;
use sqlx::SqlitePool;
use std::sync::Arc;

const MAP_CONFLICT_JUDGE_SYSTEM_PROMPT: &str = "你是小说地图拓扑冲突判断 agent。只判断地点拓扑事实是否可接受、冲突、不确定或应拒绝。不要输出 Markdown，只输出严格 json 对象。";
const VALID_CONFLICT_TYPES: &[&str] = &[
    "parent_conflict",
    "direction_conflict",
    "distance_conflict",
    "cycle_risk",
    "duplicate_contradiction",
    "ambiguous_place",
    "unknown_conflict",
];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapConflictDecision {
    Accept,
    Conflict,
    Uncertain,
    Reject,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapConflictJudgeOutput {
    pub decision: MapConflictDecision,
    pub normalized_edge_type: Option<String>,
    pub normalized_direction_hint: Option<String>,
    pub normalized_distance_hint: Option<String>,
    pub conflict_type: Option<String>,
    pub reason_code: String,
    pub confidence: f64,
    pub explanation_for_log: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MapConflictJudgeInput {
    pub book_id: String,
    pub chapter_index: i64,
    pub claim_id: String,
    pub claim_type: String,
    pub from_place_mention: String,
    pub to_place_mention: String,
    pub edge_type: String,
    pub evidence_spans: Vec<String>,
    pub existing_map_context: Vec<String>,
    pub boundary_warnings: Vec<String>,
}

#[axum::async_trait]
pub trait MapConflictJudge: Send + Sync {
    async fn judge(&self, input: &MapConflictJudgeInput) -> anyhow::Result<MapConflictJudgeOutput>;
}

pub struct MockMapConflictJudge {
    output: MapConflictJudgeOutput,
}

impl MockMapConflictJudge {
    pub fn new(output: MapConflictJudgeOutput) -> Self {
        Self { output }
    }
}

#[axum::async_trait]
impl MapConflictJudge for MockMapConflictJudge {
    async fn judge(
        &self,
        _input: &MapConflictJudgeInput,
    ) -> anyhow::Result<MapConflictJudgeOutput> {
        Ok(self.output.clone())
    }
}

#[derive(Default)]
pub struct DefaultMapConflictJudge;

impl DefaultMapConflictJudge {
    pub fn new() -> Self {
        Self
    }
}

#[axum::async_trait]
impl MapConflictJudge for DefaultMapConflictJudge {
    async fn judge(
        &self,
        _input: &MapConflictJudgeInput,
    ) -> anyhow::Result<MapConflictJudgeOutput> {
        Ok(MapConflictJudgeOutput {
            decision: MapConflictDecision::Uncertain,
            normalized_edge_type: None,
            normalized_direction_hint: None,
            normalized_distance_hint: None,
            conflict_type: None,
            reason_code: "default_uncertain".to_string(),
            confidence: 0.0,
            explanation_for_log: "Default map conflict judge does not make topology decisions"
                .to_string(),
        })
    }
}

pub struct RealAiMapConflictJudge {
    ai_model_service: Arc<AiModelService>,
    pool: SqlitePool,
}

impl RealAiMapConflictJudge {
    pub fn new(ai_model_service: Arc<AiModelService>, pool: SqlitePool) -> Self {
        Self {
            ai_model_service,
            pool,
        }
    }
}

#[axum::async_trait]
impl MapConflictJudge for RealAiMapConflictJudge {
    async fn judge(&self, input: &MapConflictJudgeInput) -> anyhow::Result<MapConflictJudgeOutput> {
        use crate::model::ai_model::AiModelKind;
        use crate::model::ai_proxy::{ai_proxy_timeout, build_ai_proxy_url};
        use crate::service::ai_book_generation_service::extract_model_content;

        let prompt = build_judge_prompt(input)?;
        let config = self
            .ai_model_service
            .get()
            .await
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        let endpoint = config.resolve(AiModelKind::Text);
        if !endpoint.enabled {
            anyhow::bail!("AI text model is disabled");
        }

        let ai_run_id =
            create_map_conflict_judge_run(&self.pool, input, &endpoint.model, &prompt).await?;

        let result = async {
            let path = if endpoint.path.trim().is_empty() {
                "/v1/chat/completions"
            } else {
                endpoint.path.trim()
            };
            let target = build_ai_proxy_url(&endpoint.base_url, path, endpoint.use_full_url)
                .map_err(|err| anyhow::anyhow!("{}", err))?;
            let body = build_judge_model_body(path, &endpoint.model, &prompt);
            let client = reqwest::Client::builder()
                .timeout(ai_proxy_timeout())
                .build()?;
            let use_gemini =
                crate::service::ai_book_generation_service::is_gemini_generate_content_path(path)
                    && target.host_str() == Some("generativelanguage.googleapis.com");
            let mut builder = client
                .post(target)
                .header(reqwest::header::ACCEPT, "application/json")
                .json(&body);
            if !endpoint.api_key.trim().is_empty() {
                if use_gemini {
                    builder = builder.header("x-goog-api-key", endpoint.api_key.trim());
                } else {
                    builder = builder.bearer_auth(endpoint.api_key.trim());
                }
            }
            let response = builder.send().await?;
            if !response.status().is_success() {
                let status = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                anyhow::bail!("AI model returned error {}: {}", status, text);
            }
            let value: serde_json::Value = response.json().await?;
            let content = extract_model_content(path, &value)?;
            parse_judge_output_with_repair(&content)
        }
        .await;

        match &result {
            Ok(output) => {
                AiRunRepo::new(self.pool.clone())
                    .update_run_status(
                        &ai_run_id,
                        "success",
                        Some(&serde_json::to_string(output)?),
                        None,
                    )
                    .await?;
            }
            Err(err) => {
                AiRunRepo::new(self.pool.clone())
                    .update_run_status(&ai_run_id, "failed", None, Some(&err.to_string()))
                    .await?;
            }
        }

        result
    }
}

pub fn parse_judge_output(raw: &str) -> anyhow::Result<MapConflictJudgeOutput> {
    let cleaned = strip_markdown_fences(raw);
    let output: MapConflictJudgeOutput =
        serde_json::from_str(&cleaned).map_err(|err| anyhow::anyhow!("JSON parse error: {}", err))?;
    validate_judge_output(&output)?;
    Ok(output)
}

pub fn parse_judge_output_with_repair(raw: &str) -> anyhow::Result<MapConflictJudgeOutput> {
    match parse_judge_output(raw) {
        Ok(output) => Ok(output),
        Err(_) => {
            let repaired = try_repair_json(&strip_markdown_fences(raw))?;
            let output: MapConflictJudgeOutput = serde_json::from_str(&repaired)
                .map_err(|err| anyhow::anyhow!("JSON parse error after repair: {}", err))?;
            validate_judge_output(&output)?;
            Ok(output)
        }
    }
}

fn validate_judge_output(output: &MapConflictJudgeOutput) -> anyhow::Result<()> {
    if !output.confidence.is_finite() || output.confidence < 0.0 || output.confidence > 1.0 {
        anyhow::bail!("confidence must be between 0 and 1");
    }
    if output.reason_code.trim().is_empty() {
        anyhow::bail!("reason_code is required");
    }
    if output.explanation_for_log.trim().is_empty() {
        anyhow::bail!("explanation_for_log is required");
    }
    if let Some(edge_type) = output.normalized_edge_type.as_deref() {
        if !VALID_PLACE_EDGE_TYPES.contains(&edge_type) {
            anyhow::bail!("invalid normalized_edge_type: {}", edge_type);
        }
    }
    if let Some(conflict_type) = output.conflict_type.as_deref() {
        if !VALID_CONFLICT_TYPES.contains(&conflict_type) {
            anyhow::bail!("invalid conflict_type: {}", conflict_type);
        }
    }

    match output.decision {
        MapConflictDecision::Accept => {
            let edge_type = output
                .normalized_edge_type
                .as_deref()
                .unwrap_or_default()
                .trim();
            if edge_type.is_empty() {
                anyhow::bail!("accept requires normalized_edge_type");
            }
        }
        MapConflictDecision::Conflict => {
            let conflict_type = output
                .conflict_type
                .as_deref()
                .unwrap_or_default()
                .trim();
            if conflict_type.is_empty() {
                anyhow::bail!("conflict requires conflict_type");
            }
        }
        MapConflictDecision::Uncertain | MapConflictDecision::Reject => {}
    }

    Ok(())
}

pub fn build_judge_prompt(input: &MapConflictJudgeInput) -> anyhow::Result<String> {
    Ok(format!(
        r#"Return one strict json object for a novel map conflict decision.

Boundary rules:
- geography knowledge is context only and cannot by itself become a map edge
- no map art, no coordinates, no invented routes, no final image
- be conservative: conflict or uncertain is better than polluting active topology

Allowed decisions: accept, conflict, uncertain, reject.
Allowed conflict_type values: {conflict_types}.

Candidate:
- book_id: {book_id}
- chapter_index: {chapter_index}
- claim_id: {claim_id}
- claim_type: {claim_type}
- from_place_mention: {from_place}
- to_place_mention: {to_place}
- edge_type: {edge_type}

Evidence spans:
{evidence}

Existing map context:
{context}

Boundary warnings:
{warnings}

Required json fields:
decision, normalized_edge_type, normalized_direction_hint, normalized_distance_hint,
conflict_type, reason_code, confidence, explanation_for_log.
"#,
        conflict_types = VALID_CONFLICT_TYPES.join(", "),
        book_id = input.book_id,
        chapter_index = input.chapter_index,
        claim_id = input.claim_id,
        claim_type = input.claim_type,
        from_place = input.from_place_mention,
        to_place = input.to_place_mention,
        edge_type = input.edge_type,
        evidence = input.evidence_spans.join("\n"),
        context = input.existing_map_context.join("\n"),
        warnings = input.boundary_warnings.join("\n"),
    ))
}

async fn create_map_conflict_judge_run(
    pool: &SqlitePool,
    input: &MapConflictJudgeInput,
    model: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let chapter_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM chapters WHERE book_id = ? AND chapter_index = ? LIMIT 1",
    )
    .bind(&input.book_id)
    .bind(input.chapter_index)
    .fetch_optional(pool)
    .await?;
    let run = AiRunRepo::new(pool.clone())
        .create_run(
            &input.book_id,
            &chapter_id.unwrap_or_default(),
            None,
            "map_conflict_judge",
            model,
            "v1",
            1,
            &md5_hex(prompt),
        )
        .await?;
    Ok(run.id)
}

pub async fn record_map_conflict_judge_success(
    pool: &SqlitePool,
    input: &MapConflictJudgeInput,
    model: &str,
    prompt: &str,
    output: &MapConflictJudgeOutput,
) -> anyhow::Result<String> {
    let run_id = create_map_conflict_judge_run(pool, input, model, prompt).await?;
    AiRunRepo::new(pool.clone())
        .update_run_status(
            &run_id,
            "success",
            Some(&serde_json::to_string(output)?),
            None,
        )
        .await?;
    Ok(run_id)
}

fn build_judge_model_body(path: &str, model: &str, prompt: &str) -> serde_json::Value {
    let is_gemini =
        crate::service::ai_book_generation_service::is_gemini_generate_content_path(path);
    let is_anthropic = crate::service::ai_book_generation_service::is_anthropic_messages_path(path);
    let is_responses = crate::service::ai_book_generation_service::is_responses_path(path);

    if is_gemini {
        return serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "systemInstruction": { "parts": [{ "text": MAP_CONFLICT_JUDGE_SYSTEM_PROMPT }] },
            "generationConfig": { "temperature": 0.2, "maxOutputTokens": 4096, "responseMimeType": "application/json" }
        });
    }
    if is_anthropic {
        return serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "temperature": 0.2,
            "system": MAP_CONFLICT_JUDGE_SYSTEM_PROMPT,
            "messages": [{ "role": "user", "content": prompt }]
        });
    }
    if is_responses {
        return serde_json::json!({
            "model": model,
            "temperature": 0.2,
            "max_output_tokens": 4096,
            "stream": false,
            "text": { "format": { "type": "json_object" } },
            "input": [
                { "role": "system", "content": MAP_CONFLICT_JUDGE_SYSTEM_PROMPT },
                { "role": "user", "content": prompt }
            ]
        });
    }
    serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": MAP_CONFLICT_JUDGE_SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    })
}

fn strip_markdown_fences(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped
            .strip_suffix("```")
            .unwrap_or(stripped)
            .trim()
            .to_string();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped
            .strip_suffix("```")
            .unwrap_or(stripped)
            .trim()
            .to_string();
    }
    trimmed.to_string()
}

fn try_repair_json(raw: &str) -> anyhow::Result<String> {
    let mut repaired = remove_trailing_commas_before_closers(raw.trim());

    if !repaired.ends_with('}') {
        repaired.push('}');
    }

    Ok(repaired)
}

fn remove_trailing_commas_before_closers(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut result = String::with_capacity(raw.len());
    for (idx, ch) in chars.iter().enumerate() {
        if *ch == ',' {
            let next_non_ws = chars[idx + 1..].iter().find(|candidate| !candidate.is_whitespace());
            if matches!(next_non_ws, Some('}' | ']')) {
                continue;
            }
        }
        result.push(*ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    fn input() -> MapConflictJudgeInput {
        MapConflictJudgeInput {
            book_id: "book1".to_string(),
            chapter_index: 1,
            claim_id: "claim1".to_string(),
            claim_type: "location_edge".to_string(),
            from_place_mention: "黑风谷".to_string(),
            to_place_mention: "青云城".to_string(),
            edge_type: "north_of".to_string(),
            evidence_spans: vec!["黑风谷在青云城以北".to_string()],
            existing_map_context: vec!["青云城 contains 青云门".to_string()],
            boundary_warnings: vec![],
        }
    }

    fn output(decision: MapConflictDecision) -> MapConflictJudgeOutput {
        MapConflictJudgeOutput {
            decision,
            normalized_edge_type: Some("north_of".to_string()),
            normalized_direction_hint: Some("north".to_string()),
            normalized_distance_hint: None,
            conflict_type: None,
            reason_code: "valid_map_edge".to_string(),
            confidence: 0.86,
            explanation_for_log: "Evidence states stable topology.".to_string(),
        }
    }

    #[test]
    fn validates_map_conflict_judge_decisions() {
        let accept = r#"{
          "decision":"accept",
          "normalized_edge_type":"north_of",
          "normalized_direction_hint":"north",
          "normalized_distance_hint":null,
          "conflict_type":null,
          "reason_code":"valid_direction",
          "confidence":0.86,
          "explanation_for_log":"stable directional statement"
        }"#;
        assert_eq!(
            parse_judge_output(accept).unwrap().decision,
            MapConflictDecision::Accept
        );

        let conflict = r#"{
          "decision":"conflict",
          "normalized_edge_type":"contains",
          "normalized_direction_hint":null,
          "normalized_distance_hint":null,
          "conflict_type":"parent_conflict",
          "reason_code":"existing_parent_differs",
          "confidence":0.77,
          "explanation_for_log":"new parent conflicts with existing parent"
        }"#;
        assert_eq!(
            parse_judge_output(conflict).unwrap().decision,
            MapConflictDecision::Conflict
        );

        let invalid = r#"{
          "decision":"conflict",
          "normalized_edge_type":"contains",
          "normalized_direction_hint":null,
          "normalized_distance_hint":null,
          "conflict_type":"made_up",
          "reason_code":"x",
          "confidence":0.5,
          "explanation_for_log":"x"
        }"#;
        assert!(parse_judge_output(invalid).is_err());

        let invalid_accept = r#"{
          "decision":"accept",
          "normalized_edge_type":null,
          "normalized_direction_hint":null,
          "normalized_distance_hint":null,
          "conflict_type":null,
          "reason_code":"missing_edge_type",
          "confidence":0.5,
          "explanation_for_log":"accept must normalize a canonical edge"
        }"#;
        assert!(parse_judge_output(invalid_accept).is_err());
    }

    #[test]
    fn invalid_map_conflict_output_repairs_once() {
        let malformed = r#"{
          "decision":"uncertain",
          "normalized_edge_type":null,
          "normalized_direction_hint":null,
          "normalized_distance_hint":null,
          "conflict_type":null,
          "reason_code":"weak_evidence",
          "confidence":0.4,
          "explanation_for_log":"not enough topology evidence",
        }"#;

        let repaired = parse_judge_output_with_repair(malformed).unwrap();

        assert_eq!(repaired.decision, MapConflictDecision::Uncertain);
    }

    #[tokio::test]
    async fn map_conflict_judge_ai_run_lifecycle_records_success() {
        let pool = db::init_pool("sqlite::memory:").await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter1', 'book1', 1, 'text', 'hash1', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();

        let input = input();
        let prompt = "return strict json for map conflict judge";
        let output = output(MapConflictDecision::Accept);

        let run_id = record_map_conflict_judge_success(
            &pool,
            &input,
            "test-model",
            prompt,
            &output,
        )
        .await
        .unwrap();

        let run = AiRunRepo::new(pool).get_run(&run_id).await.unwrap().unwrap();
        assert_eq!(run.run_type, "map_conflict_judge");
        assert_eq!(run.status, "success");
        assert_eq!(run.input_hash, md5_hex(prompt));
        let saved: MapConflictJudgeOutput =
            serde_json::from_str(run.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(saved.decision, MapConflictDecision::Accept);
    }

    #[test]
    fn build_prompt_mentions_lowercase_json_and_boundaries() {
        let prompt = build_judge_prompt(&input()).unwrap();

        assert!(prompt.contains("json"));
        assert!(prompt.contains("geography knowledge is context only"));
        assert!(prompt.contains("no map art"));
    }

    #[tokio::test]
    async fn default_judge_returns_uncertain() {
        let output = DefaultMapConflictJudge::new().judge(&input()).await.unwrap();

        assert_eq!(output.decision, MapConflictDecision::Uncertain);
    }
}
