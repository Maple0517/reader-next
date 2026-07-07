use crate::service::ai_model_service::AiModelService;
use crate::service::v4::extractor::{
    VALID_DIRECTIONALITIES, VALID_POLARITIES, VALID_RELATION_GROUPS,
};
use crate::storage::db::v4::ai_run_repo::AiRunRepo;
use crate::storage::db::v4::claim_repo::{ClaimRecord, SourceSpanRecord};
use crate::storage::db::v4::entity_repo::EntityRecord;
use crate::storage::db::v4::property_repo::CurrentPropertyRecord;
use crate::storage::db::v4::relationship_repo::RelationshipRecord;
use crate::util::hash::md5_hex;
use sqlx::SqlitePool;

/// Structural confidence threshold: claims below this are rejected.
pub const STRUCTURAL_CONFIDENCE_THRESHOLD: f64 = 0.3;

/// Result of the structural gate check.
#[derive(Debug, Clone, PartialEq)]
pub enum GateResult {
    /// Pass to AI Semantic Judge.
    Pass,
    /// Reject the claim with a reason.
    Reject(String),
    /// Redirect to property_update with optional dimension_key.
    Redirect {
        target: String,
        dimension_key: Option<String>,
    },
    /// Uncertain — keep for later chapter clarification.
    Uncertain(String),
}

/// AI Semantic Judge decision.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JudgeDecision {
    Accept,
    Reject,
    Redirect,
    Uncertain,
}

/// Output of the AI Semantic Judge.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JudgeOutput {
    pub decision: JudgeDecision,
    pub reason_code: String,
    pub confidence: f64,
    pub normalized_relation_group: Option<String>,
    pub normalized_relation_label: Option<String>,
    pub directionality: Option<String>,
    pub current_state: Option<String>,
    pub strength: Option<f64>,
    pub polarity: Option<String>,
    pub importance_score: Option<f64>,
    pub redirect_to: Option<String>,
    pub redirect_dimension_key: Option<String>,
    pub explanation_for_log: String,
}

/// Structural gate check for relationship_update claims.
///
/// Pure code logic, no AI calls. Runs after ClaimWriter, before AI Semantic Judge.
/// Uses structural signals (resolved entity type, Extractor hint, relation_group)
/// to quickly filter, redirect, or flag claims.
///
/// Callers must pre-fetch subject/object entities before calling this function.
///
/// This is the actual implementation. Callers fetch entities before calling this.
pub fn structural_gate_with_relationship_fields(
    claim: &ClaimRecord,
    subject_entity: Option<&EntityRecord>,
    object_entity: Option<&EntityRecord>,
    relation_group: &str,
    is_long_term_hint: bool,
) -> GateResult {
    // Rule 1: subject_entity_id must exist and entity must be character
    let _subject_entity_id = match &claim.subject_entity_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => return GateResult::Reject("subject_entity_id missing or empty".to_string()),
    };

    let subject = match subject_entity {
        Some(e) => e,
        None => return GateResult::Reject("subject entity not found".to_string()),
    };

    if subject.entity_type != "character" {
        return GateResult::Reject(format!(
            "subject entity_type is '{}', not 'character'",
            subject.entity_type
        ));
    }

    // Rule 2: subject == object (only when object resolved)
    if let (Some(subj_id), Some(obj_id)) = (&claim.subject_entity_id, &claim.object_entity_id) {
        if subj_id == obj_id {
            return GateResult::Reject("subject and object are the same entity".to_string());
        }
    }

    // Rule 3: relation_group must be in VALID_RELATION_GROUPS
    if relation_group.is_empty() || !VALID_RELATION_GROUPS.contains(&relation_group) {
        return GateResult::Reject(format!(
            "invalid or missing relation_group: '{}'",
            relation_group
        ));
    }

    // Rule 4: evidence_span_ids must be non-empty
    // Check claim_source_spans via primary_source_span_id or value_json evidence
    let has_evidence = !claim.primary_source_span_id.is_empty();
    if !has_evidence {
        return GateResult::Reject("no evidence spans".to_string());
    }

    // Rule 5: confidence >= STRUCTURAL_CONFIDENCE_THRESHOLD
    if claim.confidence < STRUCTURAL_CONFIDENCE_THRESHOLD {
        return GateResult::Reject(format!(
            "confidence {} below threshold {}",
            claim.confidence, STRUCTURAL_CONFIDENCE_THRESHOLD
        ));
    }

    // Rule 6: Redirect rules based on structural signals
    // Object resolved as non-character entity → Redirect (unconditional)
    if let Some(obj) = object_entity {
        if obj.entity_type != "character" {
            let dimension_key = map_non_character_to_dimension(&obj.entity_type);
            return GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some(dimension_key),
            };
        }
        // Object is character → Pass to AI Judge
        return GateResult::Pass;
    }

    // Rule 7: is_long_term_or_significant_hint + confidence rules
    // Only applies when object is unresolved (not a non-character entity)
    if !is_long_term_hint && claim.confidence < 0.6 {
        return GateResult::Uncertain(format!(
            "not long-term hint and confidence {} < 0.6",
            claim.confidence
        ));
    }

    // Object unresolved → Pass to AI Judge (do NOT Reject)
    // Cannot structurally determine if this is a real relationship
    GateResult::Pass
}

/// Map non-character entity type to property_update dimension_key.
fn map_non_character_to_dimension(entity_type: &str) -> String {
    match entity_type {
        "place" | "location" => "location".to_string(),
        "organization" | "faction" => "affiliation".to_string(),
        "item" => "equipment".to_string(),
        "ability" | "skill" => "ability".to_string(),
        "realm" | "cultivation" => "realm".to_string(),
        _ => "affiliation".to_string(), // default fallback
    }
}

// --- AI Semantic Judge ---

const JUDGE_SYSTEM_PROMPT: &str = "你是小说人物关系判断 agent。判断一段关系是否值得记录到人物关系页。\n不要输出 Markdown，不要输出解释，只输出严格 JSON 对象。";

/// Normalize JSON value: convert string values for numeric fields to numbers.
/// Handles cases where AI returns "strong"/"weak" etc. for strength/confidence/importance_score.
fn normalize_judge_json(raw: &str) -> anyhow::Result<String> {
    let mut v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))?;
    if let Some(obj) = v.as_object_mut() {
        for key in &["strength", "importance_score", "confidence"] {
            if let Some(val) = obj.get_mut(*key) {
                match val {
                    serde_json::Value::String(s) => {
                        if let Ok(n) = s.parse::<f64>() {
                            *val = serde_json::Value::Number(
                                serde_json::Number::from_f64(n)
                                    .unwrap_or(serde_json::Number::from(0)),
                            );
                        } else {
                            let mapped = match s.to_lowercase().as_str() {
                                "strong" | "high" => 0.85,
                                "medium" | "moderate" => 0.6,
                                "weak" | "low" => 0.3,
                                _ => 0.5,
                            };
                            *val = serde_json::Value::Number(
                                serde_json::Number::from_f64(mapped)
                                    .unwrap_or(serde_json::Number::from(0)),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(serde_json::to_string(&v)?)
}

/// Parse and validate JudgeOutput from raw AI response JSON.
///
/// Strips markdown fences, normalizes numeric fields, parses JSON, validates fields.
/// Returns error if parse or validation fails (no auto-repair).
pub fn parse_judge_output(raw: &str) -> anyhow::Result<JudgeOutput> {
    let cleaned = strip_markdown_fences_judge(raw);

    let normalized = normalize_judge_json(&cleaned)?;
    let mut output: JudgeOutput = serde_json::from_str(&normalized)
        .map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))?;
    output.importance_score = output.importance_score.map(normalize_unit_score);

    validate_judge_output_fields(&output)?;

    Ok(output)
}

/// Try to repair a malformed JSON string for JudgeOutput.
/// Attempts common fixes: missing closing brace, trailing comma.
fn try_repair_judge_json(raw: &str) -> anyhow::Result<String> {
    let trimmed = raw.trim();

    // Try adding closing brace
    if !trimmed.ends_with('}') {
        let attempt = format!("{}}}", trimmed.trim_end_matches(',').trim_end());
        if serde_json::from_str::<serde_json::Value>(&attempt).is_ok() {
            return Ok(attempt);
        }
    }

    // Try finding last complete key-value and close
    if let Some(pos) = trimmed.rfind('"') {
        // Find the colon or comma before the last key
        let before = &trimmed[..pos];
        if let Some(colon_pos) = before.rfind(':') {
            let attempt = format!(
                "{}}}",
                &trimmed[..colon_pos].trim_end_matches(',').trim_end()
            );
            if serde_json::from_str::<serde_json::Value>(&attempt).is_ok() {
                return Ok(attempt);
            }
        }
    }

    anyhow::bail!("无法修复 JSON")
}

/// Parse JudgeOutput with one repair attempt on failure.
pub fn parse_judge_output_with_repair(raw: &str) -> anyhow::Result<JudgeOutput> {
    // First try direct parse
    match parse_judge_output(raw) {
        Ok(output) => return Ok(output),
        Err(_) => {}
    }

    // Try repair
    let cleaned = strip_markdown_fences_judge(raw);
    let repaired = try_repair_judge_json(&cleaned)?;
    let mut output: JudgeOutput = serde_json::from_str(&repaired)
        .map_err(|e| anyhow::anyhow!("JSON parse error after repair: {}", e))?;
    output.importance_score = output.importance_score.map(normalize_unit_score);

    validate_judge_output_fields(&output)?;

    Ok(output)
}

/// Validate JudgeOutput fields (confidence range, enum values).
fn validate_judge_output_fields(output: &JudgeOutput) -> anyhow::Result<()> {
    // Validate confidence
    if output.confidence < 0.0 || output.confidence > 1.0 {
        anyhow::bail!("confidence out of range: {}", output.confidence);
    }

    // Validate normalized_relation_group
    if let Some(ref group) = output.normalized_relation_group {
        if !VALID_RELATION_GROUPS.contains(&group.as_str()) {
            anyhow::bail!("invalid normalized_relation_group: {}", group);
        }
    }

    // Validate directionality
    if let Some(ref dir) = output.directionality {
        if !VALID_DIRECTIONALITIES.contains(&dir.as_str()) {
            anyhow::bail!("invalid directionality: {}", dir);
        }
    }

    // Validate polarity
    if let Some(ref polarity) = output.polarity {
        if !VALID_POLARITIES.contains(&polarity.as_str()) {
            anyhow::bail!("invalid polarity: {}", polarity);
        }
    }

    if let Some(importance_score) = output.importance_score {
        if !(0.0..=1.0).contains(&importance_score) {
            anyhow::bail!("importance_score out of range: {}", importance_score);
        }
    }

    Ok(())
}

fn normalize_unit_score(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    if value > 1.0 && value.fract().abs() < f64::EPSILON {
        if value <= 10.0 {
            return value / 10.0;
        }
        if value <= 100.0 {
            return value / 100.0;
        }
    }
    value
}

/// Validate decision constraints based on object resolve status.
///
/// - object resolved as character → can Accept / Reject / Redirect / Uncertain
/// - object unresolved → can only Reject / Redirect / Uncertain (NOT Accept)
/// - object resolved as non-character → cannot Accept
pub fn validate_judge_decision(
    output: &JudgeOutput,
    object: Option<&EntityRecord>,
) -> anyhow::Result<()> {
    match output.decision {
        JudgeDecision::Accept => match object {
            None => {
                anyhow::bail!("Accept decision requires resolved object, but object is unresolved")
            }
            Some(obj) if obj.entity_type != "character" => {
                anyhow::bail!(
                    "Accept decision requires character object, got '{}'",
                    obj.entity_type
                );
            }
            _ => Ok(()),
        },
        JudgeDecision::Reject | JudgeDecision::Redirect | JudgeDecision::Uncertain => Ok(()),
    }
}

fn strip_markdown_fences_judge(s: &str) -> String {
    let trimmed = s.trim();
    let stripped = if trimmed.starts_with("```") {
        let after_open = trimmed
            .find('\n')
            .map(|i| &trimmed[i + 1..])
            .unwrap_or(trimmed);
        if let Some(end) = after_open.rfind("```") {
            &after_open[..end]
        } else {
            after_open
        }
    } else {
        trimmed
    };
    stripped.trim().to_string()
}

/// Build the judge prompt for AI Semantic Judge.
pub fn build_judge_prompt(
    claim: &ClaimRecord,
    subject: &EntityRecord,
    object: Option<&EntityRecord>,
    object_mention: &str,
    source_spans: &[SourceSpanRecord],
    current_relationship: Option<&RelationshipRecord>,
    subject_properties: &[CurrentPropertyRecord],
    object_properties: &[CurrentPropertyRecord],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Subject info
    parts.push(format!(
        "## Subject Character\n- Name: {}\n- Summary: {}",
        subject.canonical_name,
        subject.short_summary.as_deref().unwrap_or("无")
    ));
    if !subject_properties.is_empty() {
        parts.push("- Current Properties:".to_string());
        for prop in subject_properties {
            parts.push(format!(
                "  - {}: {}",
                prop.dimension_key,
                prop.value_text.as_deref().unwrap_or("无")
            ));
        }
    }

    // Object info
    match object {
        Some(obj) => {
            parts.push(format!(
                "\n## Object Character\n- Name: {}\n- Summary: {}",
                obj.canonical_name,
                obj.short_summary.as_deref().unwrap_or("无")
            ));
            if !object_properties.is_empty() {
                parts.push("- Current Properties:".to_string());
                for prop in object_properties {
                    parts.push(format!(
                        "  - {}: {}",
                        prop.dimension_key,
                        prop.value_text.as_deref().unwrap_or("无")
                    ));
                }
            }
        }
        None => {
            parts.push(format!(
                "\n## Object (Unresolved)\n- Mention: {}",
                object_mention
            ));
        }
    }

    // Relation hint from claim value_json
    let value_json: serde_json::Value = claim
        .value_json
        .as_ref()
        .and_then(|vj| serde_json::from_str(vj).ok())
        .unwrap_or(serde_json::Value::Null);

    let relation_hint = value_json
        .get("relation_hint")
        .and_then(|v| v.as_str())
        .unwrap_or("无");
    let relation_group = value_json
        .get("relation_group")
        .and_then(|v| v.as_str())
        .unwrap_or("无");
    let relation_label = value_json
        .get("relation_label")
        .and_then(|v| v.as_str())
        .unwrap_or("无");

    parts.push(format!(
        "\n## Relation Hint\n- From text: {}\n- Extractor suggested group: {}\n- Extractor suggested label: {}",
        relation_hint, relation_group, relation_label
    ));

    // Current relationship state
    if let Some(rel) = current_relationship {
        parts.push(format!(
            "\n## Current Relationship\n- Group: {}\n- Label: {}\n- State: {}\n- Strength: {}\n- Polarity: {}",
            rel.relation_group,
            rel.relation_label,
            rel.current_state.as_deref().unwrap_or("无"),
            rel.strength,
            rel.polarity
        ));
    }

    // Source spans
    parts.push("\n## Source Evidence".to_string());
    for span in source_spans {
        parts.push(format!("- {}", span.text_excerpt));
    }

    // Instructions
    parts.push(format!(
        "\n## Instructions\n\
         判断这是否为值得进入人物关系页的长期或重要人物关系。\n\
         - 接受(accept)：师徒、父子、宿敌、盟友、恋人等长期关系\n\
         - 拒绝(reject)：同处一室、一次性战斗、普通社交互动\n\
         - 重定向(redirect)：人物-地点/技能/物品/境界 → property_update；临时互动 → minor_event\n\
         - 不确定(uncertain)：证据不足或关系不明确\n\n\
         关系组枚举：{}\n\
         方向性枚举：directed / undirected\n\
         极性枚举：positive / negative / mixed / neutral / unknown\n\n\
         importance_score 如果出现，必须使用 0 到 1 之间的小数；不要使用 0 到 100 或 1 到 10 的整数分数。\n\n\
         输出严格 JSON 对象，字段：\n\
         decision, reason_code, confidence, normalized_relation_group, normalized_relation_label,\n\
         directionality, current_state, strength, polarity, importance_score,\n\
         redirect_to, redirect_dimension_key, explanation_for_log",
        VALID_RELATION_GROUPS.join(", ")
    ));

    parts.join("\n")
}

/// Call AI Semantic Judge.
///
/// Creates an ai_run, calls the LLM, parses and validates output.
/// Returns JudgeOutput on success.
pub async fn ai_semantic_judge(
    claim: &ClaimRecord,
    subject: &EntityRecord,
    object: Option<&EntityRecord>,
    object_mention: &str,
    source_spans: &[SourceSpanRecord],
    current_relationship: Option<&RelationshipRecord>,
    subject_properties: &[CurrentPropertyRecord],
    object_properties: &[CurrentPropertyRecord],
    ai_model_service: &AiModelService,
    pool: &SqlitePool,
) -> anyhow::Result<JudgeOutput> {
    use crate::model::ai_model::AiModelKind;
    use crate::model::ai_proxy::{ai_proxy_timeout, build_ai_proxy_url};
    use crate::service::ai_book_generation_service::extract_model_content;

    // 1. Build prompt
    let prompt = build_judge_prompt(
        claim,
        subject,
        object,
        object_mention,
        source_spans,
        current_relationship,
        subject_properties,
        object_properties,
    );

    // 2. Create ai_run
    let ai_run_repo = AiRunRepo::new(pool.clone());
    let input_hash = md5_hex(&prompt);
    // Look up chapter_id from chapter_index
    let chapter_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM chapters WHERE book_id = ? AND chapter_index = ? LIMIT 1",
    )
    .bind(&claim.book_id)
    .bind(claim.chapter_index)
    .fetch_optional(pool)
    .await?;

    let chapter_id_str = chapter_id.unwrap_or_default();

    let ai_run = ai_run_repo
        .create_run(
            &claim.book_id,
            &chapter_id_str,
            None,
            "relationship_judge",
            "unknown",
            "v1",
            1,
            &input_hash,
        )
        .await?;

    // 3. Call AI + 4. Parse + 5. Validate
    let result = async {
        let config = ai_model_service
            .get()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let endpoint = config.resolve(AiModelKind::Text);

        if !endpoint.enabled {
            anyhow::bail!("AI text model is disabled");
        }

        let path = if endpoint.path.trim().is_empty() {
            "/v1/chat/completions"
        } else {
            endpoint.path.trim()
        };
        let target = build_ai_proxy_url(&endpoint.base_url, path, endpoint.use_full_url)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
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

        // 4. Parse output (with one repair attempt)
        let output = parse_judge_output_with_repair(&content)?;

        // 5. Validate decision constraints
        validate_judge_decision(&output, object)?;

        Ok(output)
    }
    .await;

    // 6. Update ai_run
    match &result {
        Ok(output) => {
            let output_json = serde_json::to_string(output)?;
            ai_run_repo
                .update_run_status(&ai_run.id, "success", Some(&output_json), None)
                .await?;
        }
        Err(e) => {
            ai_run_repo
                .update_run_status(&ai_run.id, "failed", None, Some(&e.to_string()))
                .await?;
        }
    }

    result
}

fn build_judge_model_body(path: &str, model: &str, prompt: &str) -> serde_json::Value {
    let is_gemini =
        crate::service::ai_book_generation_service::is_gemini_generate_content_path(path);
    let is_anthropic = crate::service::ai_book_generation_service::is_anthropic_messages_path(path);
    let is_responses = crate::service::ai_book_generation_service::is_responses_path(path);

    if is_gemini {
        return serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "systemInstruction": { "parts": [{ "text": JUDGE_SYSTEM_PROMPT }] },
            "generationConfig": { "temperature": 0.2, "maxOutputTokens": 4096, "responseMimeType": "application/json" }
        });
    }
    if is_anthropic {
        return serde_json::json!({
            "model": model, "max_tokens": 4096, "temperature": 0.2,
            "system": JUDGE_SYSTEM_PROMPT,
            "messages": [{ "role": "user", "content": prompt }]
        });
    }
    if is_responses {
        return serde_json::json!({
            "model": model, "temperature": 0.2, "max_output_tokens": 4096, "stream": false,
            "text": { "format": { "type": "json_object" } },
            "input": [
                { "role": "system", "content": JUDGE_SYSTEM_PROMPT },
                { "role": "user", "content": prompt }
            ]
        });
    }
    serde_json::json!({
        "model": model, "temperature": 0.2, "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": JUDGE_SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    })
}

/// Real AI Relationship Judge — calls LLM via AiModelService.
///
/// Used in production and real AI smoke tests.
/// MockJudge / DefaultJudge are for unit tests only.
pub struct RealAiRelationshipJudge {
    ai_model_service: std::sync::Arc<AiModelService>,
    pool: SqlitePool,
}

impl RealAiRelationshipJudge {
    pub fn new(ai_model_service: std::sync::Arc<AiModelService>, pool: SqlitePool) -> Self {
        Self {
            ai_model_service,
            pool,
        }
    }
}

#[axum::async_trait]
impl crate::service::v4::relationship_processor::Judge for RealAiRelationshipJudge {
    async fn judge(&self, claim: &ClaimRecord) -> anyhow::Result<JudgeOutput> {
        use crate::storage::db::v4::entity_repo::EntityRepo;
        use crate::storage::db::v4::property_repo::PropertyRepo;
        use crate::storage::db::v4::relationship_repo::RelationshipRepo;

        let entity_repo = EntityRepo::new(self.pool.clone());
        let property_repo = PropertyRepo::new(self.pool.clone());
        let rel_repo = RelationshipRepo::new(self.pool.clone());

        // Look up subject entity
        let subject = match &claim.subject_entity_id {
            Some(id) => entity_repo
                .get_by_id(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Subject entity not found: {}", id))?,
            None => anyhow::bail!("Claim has no subject_entity_id"),
        };

        // Look up object entity (optional)
        let object = match &claim.object_entity_id {
            Some(id) => entity_repo.get_by_id(id).await?,
            None => None,
        };

        let object_mention = claim.object_mention.clone().unwrap_or_default();

        // Look up source spans for this claim
        let claim_repo = crate::storage::db::v4::claim_repo::ClaimRepo::new(self.pool.clone());
        let source_spans = claim_repo
            .list_claim_spans(&claim.id)
            .await
            .unwrap_or_default();

        // Look up current relationship (if exists)
        let current_relationship =
            if let (Some(sid), Some(oid)) = (&claim.subject_entity_id, &claim.object_entity_id) {
                // Try to find existing relationship between these entities
                let rels = rel_repo
                    .list_by_character(&claim.book_id, sid)
                    .await
                    .unwrap_or_default();
                rels.into_iter()
                    .find(|r| &r.object_character_id == oid || &r.subject_character_id == oid)
            } else {
                None
            };

        // Look up properties
        let subject_properties = property_repo
            .list_current_properties(&claim.book_id, &subject.id)
            .await
            .unwrap_or_default();
        let object_properties = if let Some(ref obj) = object {
            property_repo
                .list_current_properties(&claim.book_id, &obj.id)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        // Call real AI judge
        ai_semantic_judge(
            claim,
            &subject,
            object.as_ref(),
            &object_mention,
            &source_spans,
            current_relationship.as_ref(),
            &subject_properties,
            &object_properties,
            &self.ai_model_service,
            &self.pool,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::v4::claim_repo::ClaimRecord;
    use crate::storage::db::v4::entity_repo::EntityRecord;

    fn make_claim(
        subject_entity_id: Option<&str>,
        object_entity_id: Option<&str>,
        confidence: f64,
        relation_group: &str,
        is_long_term_hint: bool,
        primary_source_span_id: &str,
    ) -> ClaimRecord {
        let value_json = serde_json::json!({
            "relation_group": relation_group,
            "relation_hint": "test hint",
            "relation_label": "test label",
            "directionality": "directed",
            "importance_hint": 0.5,
            "is_long_term_or_significant_hint": is_long_term_hint,
        });

        ClaimRecord {
            id: "claim1".to_string(),
            book_id: "book1".to_string(),
            chapter_index: 1,
            claim_type: "relationship_update".to_string(),
            subject_mention: Some("张三".to_string()),
            object_mention: Some("李四".to_string()),
            subject_entity_id: subject_entity_id.map(|s| s.to_string()),
            object_entity_id: object_entity_id.map(|s| s.to_string()),
            predicate: "relationship".to_string(),
            value_json: Some(value_json.to_string()),
            value_text: None,
            primary_source_span_id: primary_source_span_id.to_string(),
            ai_run_id: "run1".to_string(),
            confidence,
            risk_level: "high".to_string(),
            status: "proposed".to_string(),
            supersedes_claim_id: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_entity(id: &str, entity_type: &str) -> EntityRecord {
        EntityRecord {
            id: id.to_string(),
            book_id: "book1".to_string(),
            entity_type: entity_type.to_string(),
            canonical_name: format!("entity_{}", id),
            display_name: format!("Entity {}", id),
            short_summary: None,
            importance_score: 0.5,
            first_seen_chapter: 1,
            last_seen_chapter: 1,
            status: "active".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn structural_gate(
        claim: &ClaimRecord,
        subject_entity: Option<&EntityRecord>,
        object_entity: Option<&EntityRecord>,
    ) -> GateResult {
        let value_json: serde_json::Value = claim
            .value_json
            .as_ref()
            .and_then(|vj| serde_json::from_str(vj).ok())
            .unwrap_or(serde_json::Value::Null);
        let relation_group = value_json
            .get("relation_group")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let is_long_term_hint = value_json
            .get("is_long_term_or_significant_hint")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        structural_gate_with_relationship_fields(
            claim,
            subject_entity,
            object_entity,
            relation_group,
            is_long_term_hint,
        )
    }

    #[test]
    fn subject_is_character_object_is_character_pass() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "character");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(result, GateResult::Pass);
    }

    #[test]
    fn typed_structural_gate_does_not_reparse_claim_value_json() {
        let mut claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        claim.value_json = Some(
            serde_json::json!({
                "relation_group": "invalid_group",
                "is_long_term_or_significant_hint": false,
            })
            .to_string(),
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "character");

        let result = structural_gate_with_relationship_fields(
            &claim,
            Some(&subject),
            Some(&object),
            "friendship",
            true,
        );

        assert_eq!(result, GateResult::Pass);
    }

    #[test]
    fn subject_not_resolved_reject() {
        let claim = make_claim(None, Some("obj1"), 0.8, "friendship", true, "span1");

        let result = structural_gate(&claim, None, None);
        assert_eq!(
            result,
            GateResult::Reject("subject_entity_id missing or empty".to_string())
        );
    }

    #[test]
    fn subject_is_non_character_entity_reject() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "place");
        let object = make_entity("obj1", "character");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Reject("subject entity_type is 'place', not 'character'".to_string())
        );
    }

    #[test]
    fn subject_equals_object_reject() {
        let claim = make_claim(
            Some("same_id"),
            Some("same_id"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("same_id", "character");
        let object = make_entity("same_id", "character");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Reject("subject and object are the same entity".to_string())
        );
    }

    #[test]
    fn object_resolved_as_place_redirect_location() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "place");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some("location".to_string()),
            }
        );
    }

    #[test]
    fn object_resolved_as_organization_redirect_affiliation() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "organization");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some("affiliation".to_string()),
            }
        );
    }

    #[test]
    fn object_resolved_as_item_redirect_equipment() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "item");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some("equipment".to_string()),
            }
        );
    }

    #[test]
    fn object_resolved_as_ability_redirect_ability() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "ability");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some("ability".to_string()),
            }
        );
    }

    #[test]
    fn object_unresolved_with_hint_true_pass() {
        let claim = make_claim(Some("subj1"), None, 0.8, "friendship", true, "span1");
        let subject = make_entity("subj1", "character");

        let result = structural_gate(&claim, Some(&subject), None);
        assert_eq!(result, GateResult::Pass);
    }

    #[test]
    fn object_unresolved_hint_false_confidence_high_pass() {
        let claim = make_claim(Some("subj1"), None, 0.7, "friendship", false, "span1");
        let subject = make_entity("subj1", "character");

        let result = structural_gate(&claim, Some(&subject), None);
        assert_eq!(result, GateResult::Pass);
    }

    #[test]
    fn object_unresolved_hint_false_confidence_low_uncertain() {
        let claim = make_claim(Some("subj1"), None, 0.5, "friendship", false, "span1");
        let subject = make_entity("subj1", "character");

        let result = structural_gate(&claim, Some(&subject), None);
        assert_eq!(
            result,
            GateResult::Uncertain("not long-term hint and confidence 0.5 < 0.6".to_string())
        );
    }

    #[test]
    fn invalid_relation_group_reject() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "invalid_group",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "character");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Reject("invalid or missing relation_group: 'invalid_group'".to_string())
        );
    }

    #[test]
    fn confidence_below_threshold_reject() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.2,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "character");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Reject("confidence 0.2 below threshold 0.3".to_string())
        );
    }

    #[test]
    fn no_evidence_spans_reject() {
        let claim = make_claim(Some("subj1"), Some("obj1"), 0.8, "friendship", true, "");
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "character");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(result, GateResult::Reject("no evidence spans".to_string()));
    }

    #[test]
    fn subject_entity_not_found_reject() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let object = make_entity("obj1", "character");

        let result = structural_gate(&claim, None, Some(&object));
        assert_eq!(
            result,
            GateResult::Reject("subject entity not found".to_string())
        );
    }

    #[test]
    fn empty_subject_entity_id_reject() {
        let claim = make_claim(Some(""), Some("obj1"), 0.8, "friendship", true, "span1");
        let object = make_entity("obj1", "character");

        let result = structural_gate(&claim, None, Some(&object));
        assert_eq!(
            result,
            GateResult::Reject("subject_entity_id missing or empty".to_string())
        );
    }

    #[test]
    fn object_resolved_as_faction_redirect_affiliation() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "faction");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some("affiliation".to_string()),
            }
        );
    }

    #[test]
    fn object_resolved_as_realm_redirect_realm() {
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.8,
            "friendship",
            true,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "realm");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some("realm".to_string()),
            }
        );
    }

    #[test]
    fn object_resolved_as_place_low_confidence_no_hint_redirect_not_uncertain() {
        // Bug repro: object=place, hint=false, confidence=0.5
        // Must be Redirect (non-character entity takes priority), NOT Uncertain
        let claim = make_claim(
            Some("subj1"),
            Some("obj1"),
            0.5,
            "friendship",
            false,
            "span1",
        );
        let subject = make_entity("subj1", "character");
        let object = make_entity("obj1", "place");

        let result = structural_gate(&claim, Some(&subject), Some(&object));
        assert_eq!(
            result,
            GateResult::Redirect {
                target: "property_update".to_string(),
                dimension_key: Some("location".to_string()),
            }
        );
    }

    #[test]
    fn map_non_character_to_dimension_variants() {
        assert_eq!(map_non_character_to_dimension("place"), "location");
        assert_eq!(map_non_character_to_dimension("location"), "location");
        assert_eq!(
            map_non_character_to_dimension("organization"),
            "affiliation"
        );
        assert_eq!(map_non_character_to_dimension("faction"), "affiliation");
        assert_eq!(map_non_character_to_dimension("item"), "equipment");
        assert_eq!(map_non_character_to_dimension("ability"), "ability");
        assert_eq!(map_non_character_to_dimension("skill"), "ability");
        assert_eq!(map_non_character_to_dimension("realm"), "realm");
        assert_eq!(map_non_character_to_dimension("cultivation"), "realm");
        assert_eq!(map_non_character_to_dimension("unknown"), "affiliation"); // fallback
    }

    // --- AI Semantic Judge tests ---

    #[test]
    fn parse_valid_judge_output() {
        let json = r#"{
            "decision": "accept",
            "reason_code": "long_term_relationship",
            "confidence": 0.9,
            "normalized_relation_group": "mentorship",
            "normalized_relation_label": "师徒",
            "directionality": "directed",
            "current_state": "活跃",
            "strength": 0.8,
            "polarity": "positive",
            "importance_score": 0.9,
            "redirect_to": null,
            "redirect_dimension_key": null,
            "explanation_for_log": "这是一段长期的师徒关系"
        }"#;

        let result = parse_judge_output(json);
        assert!(result.is_ok(), "valid JSON should parse successfully");
        let output = result.unwrap();
        assert_eq!(output.decision, JudgeDecision::Accept);
        assert_eq!(output.reason_code, "long_term_relationship");
        assert_eq!(output.confidence, 0.9);
        assert_eq!(
            output.normalized_relation_group,
            Some("mentorship".to_string())
        );
        assert_eq!(output.normalized_relation_label, Some("师徒".to_string()));
        assert_eq!(output.directionality, Some("directed".to_string()));
        assert_eq!(output.strength, Some(0.8));
        assert_eq!(output.polarity, Some("positive".to_string()));
        assert_eq!(output.importance_score, Some(0.9));
        assert!(output.redirect_to.is_none());
        assert!(output.redirect_dimension_key.is_none());
    }

    #[test]
    fn parse_judge_output_normalizes_percentage_importance_score() {
        let json = r#"{
            "decision": "accept",
            "reason_code": "long_term_relationship",
            "confidence": 0.91,
            "normalized_relation_group": "mentorship",
            "normalized_relation_label": "师徒",
            "directionality": "directed",
            "current_state": "活跃",
            "strength": 0.8,
            "polarity": "positive",
            "importance_score": 72,
            "redirect_to": null,
            "redirect_dimension_key": null,
            "explanation_for_log": "这是一段长期的师徒关系"
        }"#;

        let output = parse_judge_output(json).unwrap();
        assert_eq!(output.importance_score, Some(0.72));
    }

    #[test]
    fn parse_judge_output_with_markdown_fences() {
        let json = "```json\n{\"decision\":\"reject\",\"reason_code\":\"temporary\",\"confidence\":0.8,\"explanation_for_log\":\"test\"}\n```";
        let result = parse_judge_output(json);
        assert!(result.is_ok(), "markdown fences should be stripped");
        let output = result.unwrap();
        assert_eq!(output.decision, JudgeDecision::Reject);
    }

    #[test]
    fn parse_judge_output_invalid_confidence_rejected() {
        let json = r#"{
            "decision": "accept",
            "reason_code": "test",
            "confidence": 1.5,
            "explanation_for_log": "test"
        }"#;
        let result = parse_judge_output(json);
        assert!(result.is_err(), "confidence > 1.0 should be rejected");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("confidence out of range"));
    }

    #[test]
    fn parse_judge_output_invalid_relation_group_rejected() {
        let json = r#"{
            "decision": "accept",
            "reason_code": "test",
            "confidence": 0.8,
            "normalized_relation_group": "invalid_group",
            "explanation_for_log": "test"
        }"#;
        let result = parse_judge_output(json);
        assert!(
            result.is_err(),
            "invalid normalized_relation_group should be rejected"
        );
    }

    #[test]
    fn parse_judge_output_invalid_directionality_rejected() {
        let json = r#"{
            "decision": "accept",
            "reason_code": "test",
            "confidence": 0.8,
            "directionality": "sideways",
            "explanation_for_log": "test"
        }"#;
        let result = parse_judge_output(json);
        assert!(result.is_err(), "invalid directionality should be rejected");
    }

    #[test]
    fn parse_judge_output_invalid_polarity_rejected() {
        let json = r#"{
            "decision": "accept",
            "reason_code": "test",
            "confidence": 0.8,
            "polarity": "very_positive",
            "explanation_for_log": "test"
        }"#;
        let result = parse_judge_output(json);
        assert!(result.is_err(), "invalid polarity should be rejected");
    }

    #[test]
    fn parse_judge_output_with_repair_success() {
        // Missing closing brace - should be repairable
        let json = r#"{"decision":"reject","reason_code":"temporary","confidence":0.8,"explanation_for_log":"test""#;
        let result = parse_judge_output_with_repair(json);
        assert!(result.is_ok(), "repairable JSON should succeed");
        let output = result.unwrap();
        assert_eq!(output.decision, JudgeDecision::Reject);
    }

    #[test]
    fn parse_judge_output_with_repair_fail() {
        // Completely malformed JSON
        let json = "this is not json at all";
        let result = parse_judge_output_with_repair(json);
        assert!(result.is_err(), "unrepairable JSON should fail");
    }

    #[test]
    fn validate_accept_with_character_object_ok() {
        let output = JudgeOutput {
            decision: JudgeDecision::Accept,
            reason_code: "test".to_string(),
            confidence: 0.9,
            normalized_relation_group: Some("friendship".to_string()),
            normalized_relation_label: Some("朋友".to_string()),
            directionality: Some("undirected".to_string()),
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "test".to_string(),
        };

        let object = make_entity("obj1", "character");
        let result = validate_judge_decision(&output, Some(&object));
        assert!(
            result.is_ok(),
            "Accept with character object should be valid"
        );
    }

    #[test]
    fn validate_accept_with_unresolved_object_rejected() {
        let output = JudgeOutput {
            decision: JudgeDecision::Accept,
            reason_code: "test".to_string(),
            confidence: 0.9,
            normalized_relation_group: None,
            normalized_relation_label: None,
            directionality: None,
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "test".to_string(),
        };

        let result = validate_judge_decision(&output, None);
        assert!(
            result.is_err(),
            "Accept with unresolved object should be rejected"
        );
        assert!(result.unwrap_err().to_string().contains("unresolved"));
    }

    #[test]
    fn validate_accept_with_noncharacter_object_rejected() {
        let output = JudgeOutput {
            decision: JudgeDecision::Accept,
            reason_code: "test".to_string(),
            confidence: 0.9,
            normalized_relation_group: None,
            normalized_relation_label: None,
            directionality: None,
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "test".to_string(),
        };

        let object = make_entity("obj1", "place");
        let result = validate_judge_decision(&output, Some(&object));
        assert!(
            result.is_err(),
            "Accept with non-character object should be rejected"
        );
        assert!(result.unwrap_err().to_string().contains("character"));
    }

    #[test]
    fn validate_reject_decision_ok() {
        let output = JudgeOutput {
            decision: JudgeDecision::Reject,
            reason_code: "temporary_interaction".to_string(),
            confidence: 0.8,
            normalized_relation_group: None,
            normalized_relation_label: None,
            directionality: None,
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "一次性互动".to_string(),
        };

        // Reject is valid regardless of object status
        let result = validate_judge_decision(&output, None);
        assert!(result.is_ok(), "Reject should always be valid");

        let object = make_entity("obj1", "place");
        let result = validate_judge_decision(&output, Some(&object));
        assert!(
            result.is_ok(),
            "Reject with non-character object should be valid"
        );
    }

    #[test]
    fn validate_redirect_decision_ok() {
        let output = JudgeOutput {
            decision: JudgeDecision::Redirect,
            reason_code: "person_place".to_string(),
            confidence: 0.7,
            normalized_relation_group: None,
            normalized_relation_label: None,
            directionality: None,
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: Some("property_update".to_string()),
            redirect_dimension_key: Some("affiliation".to_string()),
            explanation_for_log: "人物-地点关系".to_string(),
        };

        let result = validate_judge_decision(&output, None);
        assert!(result.is_ok(), "Redirect should always be valid");
    }

    #[test]
    fn validate_uncertain_decision_ok() {
        let output = JudgeOutput {
            decision: JudgeDecision::Uncertain,
            reason_code: "insufficient_evidence".to_string(),
            confidence: 0.5,
            normalized_relation_group: None,
            normalized_relation_label: None,
            directionality: None,
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: None,
            redirect_dimension_key: None,
            explanation_for_log: "证据不足".to_string(),
        };

        let result = validate_judge_decision(&output, None);
        assert!(result.is_ok(), "Uncertain should always be valid");
    }

    #[test]
    fn judge_output_roundtrip_serde() {
        let output = JudgeOutput {
            decision: JudgeDecision::Redirect,
            reason_code: "person_item".to_string(),
            confidence: 0.75,
            normalized_relation_group: None,
            normalized_relation_label: None,
            directionality: None,
            current_state: None,
            strength: None,
            polarity: None,
            importance_score: None,
            redirect_to: Some("property_update".to_string()),
            redirect_dimension_key: Some("equipment".to_string()),
            explanation_for_log: "人物-物品关系，重定向到装备属性".to_string(),
        };

        let json = serde_json::to_string(&output).unwrap();
        let parsed: JudgeOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.decision, JudgeDecision::Redirect);
        assert_eq!(parsed.redirect_to, Some("property_update".to_string()));
        assert_eq!(parsed.redirect_dimension_key, Some("equipment".to_string()));
    }

    #[test]
    fn parse_all_judge_decisions() {
        for decision in &["accept", "reject", "redirect", "uncertain"] {
            let json = format!(
                r#"{{"decision":"{}","reason_code":"test","confidence":0.8,"explanation_for_log":"test"}}"#,
                decision
            );
            let result = parse_judge_output(&json);
            assert!(result.is_ok(), "decision '{}' should parse", decision);
        }
    }

    #[test]
    fn parse_invalid_decision_rejected() {
        let json = r#"{"decision":"maybe","reason_code":"test","confidence":0.8,"explanation_for_log":"test"}"#;
        let result = parse_judge_output(&json);
        assert!(result.is_err(), "invalid decision should be rejected");
    }
}
