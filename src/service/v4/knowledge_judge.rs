use crate::service::ai_model_service::AiModelService;
use crate::service::v4::extractor::{VALID_KNOWLEDGE_CATEGORIES, VALID_KNOWLEDGE_STATUS_HINTS};
use crate::service::v4::topic_resolver::{
    ProposedTopic, TopicCandidate, TopicCandidateMatch, TopicResolution, TopicResolutionStatus,
};
use crate::storage::db::v4::ai_run_repo::AiRunRepo;
use crate::storage::db::v4::claim_repo::{ClaimRecord, SourceSpanRecord};
use crate::util::hash::md5_hex;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeJudgeDecision {
    AddNew,
    Supplement,
    ReviseExisting,
    ContradictExisting,
    MarkRumor,
    MarkUncertain,
    MarkFalseInWorld,
    Reject,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeCardAction {
    UseExistingCard,
    CreateNewCard,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeJudgeOutput {
    pub decision: KnowledgeJudgeDecision,
    pub card_action: KnowledgeCardAction,
    pub target_card_id: Option<String>,
    pub affected_assertion_ids: Vec<String>,
    pub assertion_status: String,
    pub current_summary: Option<String>,
    pub confidence: f64,
    pub reason_code: String,
    pub explanation_for_log: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawKnowledgeJudgeOutput {
    pub decision: String,
    pub card_action: String,
    #[serde(default)]
    pub target_card_id: Option<String>,
    #[serde(default)]
    pub affected_assertion_ids: Vec<String>,
    #[serde(default)]
    pub assertion_status: Option<String>,
    #[serde(default)]
    pub current_summary: Option<String>,
    pub confidence: f64,
    pub reason_code: String,
    pub explanation_for_log: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KnowledgeJudgeInput {
    pub book_id: String,
    pub chapter_index: i64,
    pub claim_id: String,
    pub category: String,
    pub assertion_text: String,
    pub status_hint: Option<String>,
    pub proposed_topic: ProposedTopic,
    pub candidate_cards: Vec<TopicCandidate>,
    pub existing_assertions: Vec<KnowledgeJudgeAssertionInput>,
    pub evidence_spans: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KnowledgeJudgeAssertionInput {
    pub assertion_id: String,
    pub card_id: String,
    pub assertion_text: String,
    pub status: String,
    pub chapter_index: i64,
}

impl KnowledgeJudgeInput {
    #[cfg(test)]
    fn for_test() -> Self {
        Self {
            book_id: "book1".to_string(),
            chapter_index: 1,
            claim_id: "claim1".to_string(),
            category: "power_system".to_string(),
            assertion_text: "Cultivation has stable realm tiers.".to_string(),
            status_hint: Some("fact".to_string()),
            proposed_topic: ProposedTopic {
                topic_key: "cultivation-realms".to_string(),
                topic_display: "Cultivation Realms".to_string(),
            },
            candidate_cards: Vec::new(),
            existing_assertions: Vec::new(),
            evidence_spans: vec!["source evidence".to_string()],
        }
    }
}

#[axum::async_trait]
pub trait KnowledgeRevisionJudge: Send + Sync {
    async fn judge(&self, input: &KnowledgeJudgeInput) -> anyhow::Result<KnowledgeJudgeOutput>;
}

#[derive(Default)]
pub struct DefaultKnowledgeRevisionJudge;

impl DefaultKnowledgeRevisionJudge {
    pub fn new() -> Self {
        Self
    }
}

#[axum::async_trait]
impl KnowledgeRevisionJudge for DefaultKnowledgeRevisionJudge {
    async fn judge(&self, _input: &KnowledgeJudgeInput) -> anyhow::Result<KnowledgeJudgeOutput> {
        Ok(KnowledgeJudgeOutput {
            decision: KnowledgeJudgeDecision::Reject,
            card_action: KnowledgeCardAction::Uncertain,
            target_card_id: None,
            affected_assertion_ids: Vec::new(),
            assertion_status: "uncertain".to_string(),
            current_summary: None,
            confidence: 0.0,
            reason_code: "default_knowledge_revision_judge".to_string(),
            explanation_for_log: "Default knowledge judge: no real AI judge wired yet".to_string(),
        })
    }
}

pub struct MockKnowledgeRevisionJudge {
    output: KnowledgeJudgeOutput,
}

impl MockKnowledgeRevisionJudge {
    pub fn new(output: KnowledgeJudgeOutput) -> Self {
        Self { output }
    }
}

#[axum::async_trait]
impl KnowledgeRevisionJudge for MockKnowledgeRevisionJudge {
    async fn judge(&self, _input: &KnowledgeJudgeInput) -> anyhow::Result<KnowledgeJudgeOutput> {
        Ok(self.output.clone())
    }
}

pub struct RealAiKnowledgeRevisionJudge {
    ai_model_service: std::sync::Arc<AiModelService>,
    pool: SqlitePool,
}

impl RealAiKnowledgeRevisionJudge {
    pub fn new(ai_model_service: std::sync::Arc<AiModelService>, pool: SqlitePool) -> Self {
        Self {
            ai_model_service,
            pool,
        }
    }
}

#[axum::async_trait]
impl KnowledgeRevisionJudge for RealAiKnowledgeRevisionJudge {
    async fn judge(&self, input: &KnowledgeJudgeInput) -> anyhow::Result<KnowledgeJudgeOutput> {
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
            create_knowledge_judge_run(&self.pool, input, &endpoint.model, &prompt).await?;
        let captured_output = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let captured_output_for_request = captured_output.clone();

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
            *captured_output_for_request.lock().unwrap() =
                Some(serde_json::to_string(&value).unwrap_or_default());
            let content = extract_model_content(path, &value)?;
            let output = parse_judge_output_with_repair(&content)?;
            validate_judge_output_for_context(&output, matching_topic_candidate_count(input))?;
            Ok(output)
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
                let raw_output = captured_output.lock().unwrap().clone();
                AiRunRepo::new(self.pool.clone())
                    .update_run_status(
                        &ai_run_id,
                        "failed",
                        raw_output.as_deref(),
                        Some(&err.to_string()),
                    )
                    .await?;
            }
        }

        result
    }
}

const KNOWLEDGE_JUDGE_SYSTEM_PROMPT: &str = "你是小说世界观知识修订判断 agent。判断新知识断言是新增、补充、修正、推翻、传闻、不确定、世界内假说还是应拒绝。不要输出 Markdown，只输出严格 json 对象。";

pub fn parse_judge_output(raw: &str) -> anyhow::Result<KnowledgeJudgeOutput> {
    let cleaned = strip_markdown_fences(raw);
    let output: RawKnowledgeJudgeOutput =
        serde_json::from_str(&cleaned).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))?;
    materialize_judge_output(output)
}

pub fn parse_judge_output_with_repair(raw: &str) -> anyhow::Result<KnowledgeJudgeOutput> {
    match parse_judge_output(raw) {
        Ok(output) => Ok(output),
        Err(_) => {
            let repaired = try_repair_json(&strip_markdown_fences(raw))?;
            let output: RawKnowledgeJudgeOutput = serde_json::from_str(&repaired)
                .map_err(|e| anyhow::anyhow!("JSON parse error after repair: {}", e))?;
            materialize_judge_output(output)
        }
    }
}

fn materialize_judge_output(raw: RawKnowledgeJudgeOutput) -> anyhow::Result<KnowledgeJudgeOutput> {
    let decision = parse_decision(&raw.decision)?;
    let card_action = parse_card_action(&raw.card_action)?;
    let no_write = decision == KnowledgeJudgeDecision::Reject
        || card_action == KnowledgeCardAction::Uncertain;
    let assertion_status = if no_write {
        normalize_no_write_assertion_status(raw.assertion_status.as_deref())
    } else {
        normalize_write_assertion_status(raw.assertion_status.as_deref())?
    };

    let output = KnowledgeJudgeOutput {
        decision,
        card_action,
        target_card_id: raw.target_card_id,
        affected_assertion_ids: raw.affected_assertion_ids,
        assertion_status,
        current_summary: raw.current_summary,
        confidence: raw.confidence,
        reason_code: raw.reason_code,
        explanation_for_log: raw.explanation_for_log,
    };
    validate_judge_output_fields(&output)?;
    Ok(output)
}

fn parse_decision(value: &str) -> anyhow::Result<KnowledgeJudgeDecision> {
    match value.trim() {
        "add_new" => Ok(KnowledgeJudgeDecision::AddNew),
        "supplement" => Ok(KnowledgeJudgeDecision::Supplement),
        "revise_existing" => Ok(KnowledgeJudgeDecision::ReviseExisting),
        "contradict_existing" => Ok(KnowledgeJudgeDecision::ContradictExisting),
        "mark_rumor" => Ok(KnowledgeJudgeDecision::MarkRumor),
        "mark_uncertain" => Ok(KnowledgeJudgeDecision::MarkUncertain),
        "mark_false_in_world" => Ok(KnowledgeJudgeDecision::MarkFalseInWorld),
        "reject" => Ok(KnowledgeJudgeDecision::Reject),
        other => anyhow::bail!("invalid decision: {}", other),
    }
}

fn parse_card_action(value: &str) -> anyhow::Result<KnowledgeCardAction> {
    match value.trim() {
        "use_existing_card" => Ok(KnowledgeCardAction::UseExistingCard),
        "create_new_card" => Ok(KnowledgeCardAction::CreateNewCard),
        "uncertain" => Ok(KnowledgeCardAction::Uncertain),
        other => anyhow::bail!("invalid card_action: {}", other),
    }
}

fn normalize_assertion_status_value(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "fact" {
        return Some("active".to_string());
    }
    Some(trimmed.to_string())
}

fn normalize_no_write_assertion_status(value: Option<&str>) -> String {
    match normalize_assertion_status_value(value) {
        Some(status)
            if matches!(
                status.as_str(),
                "active" | "rumor" | "uncertain" | "false_in_world"
            ) =>
        {
            status
        }
        _ => "uncertain".to_string(),
    }
}

fn normalize_write_assertion_status(value: Option<&str>) -> anyhow::Result<String> {
    normalize_assertion_status_value(value)
        .ok_or_else(|| anyhow::anyhow!("assertion_status is required"))
}

fn is_no_write_output(output: &KnowledgeJudgeOutput) -> bool {
    output.decision == KnowledgeJudgeDecision::Reject
        || output.card_action == KnowledgeCardAction::Uncertain
}

fn validate_judge_output_fields(output: &KnowledgeJudgeOutput) -> anyhow::Result<()> {
    if output.confidence < 0.0 || output.confidence > 1.0 {
        anyhow::bail!("confidence out of range: {}", output.confidence);
    }
    if output.reason_code.trim().is_empty() {
        anyhow::bail!("reason_code is required");
    }
    if output.explanation_for_log.trim().is_empty() {
        anyhow::bail!("explanation_for_log is required");
    }
    if is_no_write_output(output) {
        return Ok(());
    }
    if !matches!(
        output.assertion_status.as_str(),
        "active" | "rumor" | "uncertain" | "false_in_world"
    ) {
        anyhow::bail!("invalid assertion_status: {}", output.assertion_status);
    }
    Ok(())
}

pub fn validate_judge_output_for_context(
    output: &KnowledgeJudgeOutput,
    matching_candidate_count: usize,
) -> anyhow::Result<()> {
    validate_judge_output_fields(output)?;

    if is_no_write_output(output) {
        if output.decision == KnowledgeJudgeDecision::Reject
            && output.card_action != KnowledgeCardAction::Uncertain
        {
            anyhow::bail!("reject decision requires card_action=uncertain");
        }
        return Ok(());
    }

    match output.card_action {
        KnowledgeCardAction::UseExistingCard => {
            if output
                .target_card_id
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                anyhow::bail!("use_existing_card requires target_card_id");
            }
        }
        KnowledgeCardAction::CreateNewCard => {
            if matching_candidate_count > 0 {
                anyhow::bail!("create_new_card requires no matching candidates");
            }
        }
        KnowledgeCardAction::Uncertain => unreachable!("no-write branch already returned"),
    }

    match output.decision {
        KnowledgeJudgeDecision::AddNew => {}
        KnowledgeJudgeDecision::Supplement => {
            require_use_existing(output, "supplement")?;
        }
        KnowledgeJudgeDecision::ReviseExisting => {
            require_use_existing(output, "revise_existing")?;
            require_affected_assertions(output, "revise_existing")?;
        }
        KnowledgeJudgeDecision::ContradictExisting => {
            require_use_existing(output, "contradict_existing")?;
            require_affected_assertions(output, "contradict_existing")?;
        }
        KnowledgeJudgeDecision::MarkRumor => require_status(output, "rumor")?,
        KnowledgeJudgeDecision::MarkUncertain => require_status(output, "uncertain")?,
        KnowledgeJudgeDecision::MarkFalseInWorld => require_status(output, "false_in_world")?,
        KnowledgeJudgeDecision::Reject => unreachable!("no-write branch already returned"),
    }

    Ok(())
}

fn matching_topic_candidate_count(input: &KnowledgeJudgeInput) -> usize {
    input
        .candidate_cards
        .iter()
        .filter(|candidate| candidate.match_kind != TopicCandidateMatch::RecentSameCategory)
        .count()
}

fn require_use_existing(output: &KnowledgeJudgeOutput, decision: &str) -> anyhow::Result<()> {
    if output.card_action != KnowledgeCardAction::UseExistingCard {
        anyhow::bail!("{decision} requires use_existing_card");
    }
    Ok(())
}

fn require_affected_assertions(
    output: &KnowledgeJudgeOutput,
    decision: &str,
) -> anyhow::Result<()> {
    if output.affected_assertion_ids.is_empty() {
        anyhow::bail!("{decision} requires affected_assertion_ids");
    }
    Ok(())
}

fn require_status(output: &KnowledgeJudgeOutput, expected: &str) -> anyhow::Result<()> {
    if output.assertion_status != expected {
        anyhow::bail!("{:?} requires assertion_status={expected}", output.decision);
    }
    Ok(())
}

fn strip_markdown_fences(value: &str) -> String {
    let trimmed = value.trim();
    let stripped = if trimmed.starts_with("```") {
        let after_open = trimmed
            .find('\n')
            .map(|index| &trimmed[index + 1..])
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

fn try_repair_json(raw: &str) -> anyhow::Result<String> {
    let trimmed = raw.trim().trim_end_matches(',');
    if trimmed.ends_with('}') {
        return Ok(trimmed.to_string());
    }

    let repaired = format!("{}}}", trimmed);
    if serde_json::from_str::<serde_json::Value>(&repaired).is_ok() {
        return Ok(repaired);
    }

    anyhow::bail!("unable to repair knowledge judge JSON")
}

pub fn build_judge_prompt(input: &KnowledgeJudgeInput) -> anyhow::Result<String> {
    let input_json = serde_json::to_string_pretty(input)?;
    Ok(format!(
        "## Knowledge Revision Judge Input\n{}\n\n## Instructions\n\
         判断新 knowledge assertion 如何进入同一 topic/card 的知识状态。\n\
         decision: add_new, supplement, revise_existing, contradict_existing, mark_rumor, mark_uncertain, mark_false_in_world, reject。\n\
         card_action: use_existing_card, create_new_card, uncertain。\n\
         write path allowed assertion_status: active, rumor, uncertain, false_in_world。\n\
         decision=reject 时不会创建 assertion；如果必须填 assertion_status 占位，请使用 uncertain。\n\
         不要输出 unsupported。\n\
         revise_existing / contradict_existing 必须指定 affected_assertion_ids。\n\
         create_new_card 只能在没有匹配 candidate_cards 时使用。\n\
         输出严格 json 对象，字段：decision, card_action, target_card_id, affected_assertion_ids, assertion_status, current_summary, confidence, reason_code, explanation_for_log。",
        input_json
    ))
}

async fn create_knowledge_judge_run(
    pool: &SqlitePool,
    input: &KnowledgeJudgeInput,
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
            "knowledge_revision_judge",
            model,
            "v1",
            1,
            &md5_hex(prompt),
        )
        .await?;
    Ok(run.id)
}

pub async fn record_knowledge_judge_success(
    pool: &SqlitePool,
    input: &KnowledgeJudgeInput,
    model: &str,
    prompt: &str,
    output: &KnowledgeJudgeOutput,
) -> anyhow::Result<String> {
    let run_id = create_knowledge_judge_run(pool, input, model, prompt).await?;
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
            "systemInstruction": { "parts": [{ "text": KNOWLEDGE_JUDGE_SYSTEM_PROMPT }] },
            "generationConfig": { "temperature": 0.2, "maxOutputTokens": 4096, "responseMimeType": "application/json" }
        });
    }
    if is_anthropic {
        return serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "temperature": 0.2,
            "system": KNOWLEDGE_JUDGE_SYSTEM_PROMPT,
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
                { "role": "system", "content": KNOWLEDGE_JUDGE_SYSTEM_PROMPT },
                { "role": "user", "content": prompt }
            ]
        });
    }
    serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": KNOWLEDGE_JUDGE_SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum KnowledgeGateResult {
    Pass,
    Reject(String),
    Uncertain(String),
}

pub fn structural_gate(
    claim: &ClaimRecord,
    source_spans: &[SourceSpanRecord],
    topic_resolution: Option<&TopicResolution>,
) -> KnowledgeGateResult {
    if claim.claim_type != "knowledge_assertion" {
        return KnowledgeGateResult::Reject(format!(
            "unsupported claim_type for knowledge gate: {}",
            claim.claim_type
        ));
    }

    if claim.primary_source_span_id.trim().is_empty() {
        return KnowledgeGateResult::Reject("missing evidence span".to_string());
    }

    if !source_spans.iter().any(|span| {
        span.id == claim.primary_source_span_id
            && span.book_id == claim.book_id
            && span.status == "active"
    }) {
        return KnowledgeGateResult::Reject("source span evidence not found".to_string());
    }

    let value_json = match claim
        .value_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
    {
        Some(value) => value,
        None => return KnowledgeGateResult::Reject("missing or invalid value_json".to_string()),
    };

    if let Some(result) = reject_boundary_pollution(claim, &value_json) {
        return result;
    }

    let category = required_string(&value_json, "category").unwrap_or_default();
    if category.is_empty() || !VALID_KNOWLEDGE_CATEGORIES.contains(&category.as_str()) {
        return KnowledgeGateResult::Reject(format!("invalid knowledge category: {category}"));
    }

    let topic = required_string(&value_json, "topic_display")
        .or_else(|| required_string(&value_json, "raw_topic"));
    if topic.unwrap_or_default().trim().is_empty() {
        return KnowledgeGateResult::Reject("missing knowledge topic".to_string());
    }

    let assertion_text = required_string(&value_json, "assertion_text")
        .or_else(|| claim.value_text.clone())
        .unwrap_or_default();
    if assertion_text.trim().is_empty() {
        return KnowledgeGateResult::Reject("missing assertion_text".to_string());
    }

    if let Some(status_hint) = optional_string(&value_json, "status_hint") {
        if !VALID_KNOWLEDGE_STATUS_HINTS.contains(&status_hint.as_str()) {
            return KnowledgeGateResult::Reject(format!(
                "invalid knowledge status_hint: {status_hint}"
            ));
        }
        if status_hint == "uncertain" {
            return KnowledgeGateResult::Uncertain(
                "knowledge status_hint is uncertain".to_string(),
            );
        }
    }

    match topic_resolution {
        Some(resolution) if resolution.status == TopicResolutionStatus::Uncertain => {
            KnowledgeGateResult::Uncertain(format!(
                "topic resolution uncertain: {}",
                resolution
                    .reason
                    .as_deref()
                    .unwrap_or("ambiguous topic candidate")
            ))
        }
        Some(_) => KnowledgeGateResult::Pass,
        None => KnowledgeGateResult::Uncertain("topic resolution missing".to_string()),
    }
}

fn reject_boundary_pollution(
    claim: &ClaimRecord,
    value_json: &serde_json::Value,
) -> Option<KnowledgeGateResult> {
    if claim.subject_mention.is_some()
        || has_any(
            value_json,
            &["subject_mention", "dimension_key", "value_text"],
        )
    {
        return Some(KnowledgeGateResult::Reject(
            "property-shaped claim is not knowledge".to_string(),
        ));
    }

    if claim.object_mention.is_some()
        || has_any(
            value_json,
            &[
                "object_mention",
                "relation_hint",
                "relation_group",
                "relation_label",
                "directionality",
            ],
        )
    {
        return Some(KnowledgeGateResult::Reject(
            "relationship-shaped claim is not knowledge".to_string(),
        ));
    }

    if has_any(
        value_json,
        &[
            "revealed_mention",
            "canonical_mention",
            "reveal_type",
            "identity_kind",
            "candidate_type",
        ],
    ) {
        return Some(KnowledgeGateResult::Reject(
            "identity-shaped claim is not knowledge".to_string(),
        ));
    }

    if has_any(
        value_json,
        &[
            "from_place_mention",
            "to_place_mention",
            "edge_type",
            "direction_hint",
            "distance_hint",
        ],
    ) {
        return Some(KnowledgeGateResult::Reject(
            "map-shaped claim is not knowledge".to_string(),
        ));
    }

    None
}

fn has_any(value_json: &serde_json::Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| value_json.get(*key).is_some())
}

fn required_string(value_json: &serde_json::Value, key: &str) -> Option<String> {
    value_json
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_string(value_json: &serde_json::Value, key: &str) -> Option<String> {
    value_json
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::topic_resolver::{
        ProposedTopic, TopicCandidate, TopicCandidateMatch, TopicResolution, TopicResolutionStatus,
    };
    use crate::storage::db::v4::claim_repo::{ClaimRecord, SourceSpanRecord};

    fn make_claim(value_json: serde_json::Value, primary_source_span_id: &str) -> ClaimRecord {
        ClaimRecord {
            id: "claim1".to_string(),
            book_id: "book1".to_string(),
            chapter_index: 1,
            claim_type: "knowledge_assertion".to_string(),
            subject_mention: None,
            object_mention: None,
            subject_entity_id: None,
            object_entity_id: None,
            predicate: "knowledge".to_string(),
            value_json: Some(value_json.to_string()),
            value_text: value_json
                .get("assertion_text")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            primary_source_span_id: primary_source_span_id.to_string(),
            ai_run_id: "run1".to_string(),
            confidence: 0.8,
            risk_level: "high".to_string(),
            status: "proposed".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn valid_value_json() -> serde_json::Value {
        serde_json::json!({
            "category": "power_system",
            "raw_topic": "Cultivation Realms",
            "topic_display": "Cultivation Realms",
            "assertion_text": "Cultivation has stable realm tiers.",
            "importance_score": 0.8,
            "referenced_entity_mentions": [],
            "status_hint": "fact",
            "reason_hint": "narrator explanation"
        })
    }

    fn span(span_id: &str) -> SourceSpanRecord {
        SourceSpanRecord {
            id: span_id.to_string(),
            book_id: "book1".to_string(),
            chapter_id: "chapter1".to_string(),
            chapter_hash: "hash1".to_string(),
            segment_id: "segment1".to_string(),
            span_index: 0,
            start_offset: 0,
            end_offset: 10,
            text_excerpt: "evidence".to_string(),
            status: "active".to_string(),
            created_at: "now".to_string(),
        }
    }

    fn resolved_topic() -> TopicResolution {
        TopicResolution {
            proposed: ProposedTopic {
                topic_key: "cultivation-realms".to_string(),
                topic_display: "Cultivation Realms".to_string(),
            },
            candidates: Vec::new(),
            status: TopicResolutionStatus::Resolved,
            reason: None,
        }
    }

    #[test]
    fn missing_evidence_rejected() {
        let claim = make_claim(valid_value_json(), "");

        let result = structural_gate(&claim, &[], Some(&resolved_topic()));

        assert!(
            matches!(result, KnowledgeGateResult::Reject(reason) if reason.contains("evidence"))
        );
    }

    #[test]
    fn invalid_category_rejected() {
        let mut value_json = valid_value_json();
        value_json["category"] = serde_json::Value::String("temporary_event".to_string());
        let claim = make_claim(value_json, "span1");

        let result = structural_gate(&claim, &[span("span1")], Some(&resolved_topic()));

        assert!(
            matches!(result, KnowledgeGateResult::Reject(reason) if reason.contains("category"))
        );
    }

    #[test]
    fn character_property_rejected() {
        let mut value_json = valid_value_json();
        value_json["dimension_key"] = serde_json::Value::String("realm".to_string());
        let claim = make_claim(value_json, "span1");

        let result = structural_gate(&claim, &[span("span1")], Some(&resolved_topic()));

        assert!(
            matches!(result, KnowledgeGateResult::Reject(reason) if reason.contains("property"))
        );
    }

    #[test]
    fn relationship_rejected() {
        let mut value_json = valid_value_json();
        value_json["object_mention"] = serde_json::Value::String("李四".to_string());
        value_json["relation_group"] = serde_json::Value::String("alliance".to_string());
        let claim = make_claim(value_json, "span1");

        let result = structural_gate(&claim, &[span("span1")], Some(&resolved_topic()));

        assert!(
            matches!(result, KnowledgeGateResult::Reject(reason) if reason.contains("relationship"))
        );
    }

    #[test]
    fn identity_reveal_rejected() {
        let mut value_json = valid_value_json();
        value_json["revealed_mention"] = serde_json::Value::String("黑衣人".to_string());
        value_json["canonical_mention"] = serde_json::Value::String("张三".to_string());
        let claim = make_claim(value_json, "span1");

        let result = structural_gate(&claim, &[span("span1")], Some(&resolved_topic()));

        assert!(
            matches!(result, KnowledgeGateResult::Reject(reason) if reason.contains("identity"))
        );
    }

    #[test]
    fn map_edge_rejected() {
        let mut value_json = valid_value_json();
        value_json["from_place_mention"] = serde_json::Value::String("青云门".to_string());
        value_json["to_place_mention"] = serde_json::Value::String("东域".to_string());
        value_json["edge_type"] = serde_json::Value::String("north_of".to_string());
        let claim = make_claim(value_json, "span1");

        let result = structural_gate(&claim, &[span("span1")], Some(&resolved_topic()));

        assert!(matches!(result, KnowledgeGateResult::Reject(reason) if reason.contains("map")));
    }

    #[test]
    fn ambiguous_topic_uncertain() {
        let claim = make_claim(valid_value_json(), "span1");
        let mut topic = resolved_topic();
        topic.status = TopicResolutionStatus::Uncertain;
        topic.reason = Some("topic is too broad".to_string());

        let result = structural_gate(&claim, &[span("span1")], Some(&topic));

        assert!(
            matches!(result, KnowledgeGateResult::Uncertain(reason) if reason.contains("topic"))
        );
    }

    #[test]
    fn uncertain_status_hint_is_uncertain() {
        let mut value_json = valid_value_json();
        value_json["status_hint"] = serde_json::Value::String("uncertain".to_string());
        let claim = make_claim(value_json, "span1");

        let result = structural_gate(&claim, &[span("span1")], Some(&resolved_topic()));

        assert!(
            matches!(result, KnowledgeGateResult::Uncertain(reason) if reason.contains("status_hint"))
        );
    }

    #[test]
    fn multiple_resolved_topic_candidates_pass_to_ai_judge() {
        let claim = make_claim(valid_value_json(), "span1");
        let mut topic = resolved_topic();
        topic.candidates = vec![
            TopicCandidate {
                book_id: "book1".to_string(),
                category: "power_system".to_string(),
                card_id: "card1".to_string(),
                topic_key: "cultivation-realms".to_string(),
                topic_display: "Cultivation Realms".to_string(),
                current_summary: None,
                match_kind: TopicCandidateMatch::Alias,
                score: 0.9,
                last_updated_chapter: 2,
            },
            TopicCandidate {
                book_id: "book1".to_string(),
                category: "power_system".to_string(),
                card_id: "card2".to_string(),
                topic_key: "mana-rules".to_string(),
                topic_display: "Mana Rules".to_string(),
                current_summary: None,
                match_kind: TopicCandidateMatch::RecentSameCategory,
                score: 0.2,
                last_updated_chapter: 1,
            },
        ];

        let result = structural_gate(&claim, &[span("span1")], Some(&topic));

        assert_eq!(result, KnowledgeGateResult::Pass);
    }

    #[test]
    fn valid_long_term_knowledge_passes_to_ai_judge() {
        let claim = make_claim(valid_value_json(), "span1");

        let result = structural_gate(&claim, &[span("span1")], Some(&resolved_topic()));

        assert_eq!(result, KnowledgeGateResult::Pass);
    }

    fn valid_judge_output_json(decision: &str, card_action: &str) -> String {
        serde_json::json!({
            "decision": decision,
            "card_action": card_action,
            "target_card_id": if card_action == "use_existing_card" { serde_json::Value::String("card1".to_string()) } else { serde_json::Value::Null },
            "affected_assertion_ids": if decision == "revise_existing" || decision == "contradict_existing" { serde_json::json!(["assertion1"]) } else { serde_json::json!([]) },
            "assertion_status": match decision {
                "mark_rumor" => "rumor",
                "mark_uncertain" => "uncertain",
                "mark_false_in_world" => "false_in_world",
                _ => "active",
            },
            "current_summary": "Knowledge summary",
            "confidence": 0.82,
            "reason_code": "ok",
            "explanation_for_log": "valid"
        })
        .to_string()
    }

    #[test]
    fn parse_valid_ai_output() {
        let output =
            parse_judge_output(&valid_judge_output_json("add_new", "create_new_card")).unwrap();

        assert_eq!(output.decision, KnowledgeJudgeDecision::AddNew);
        assert_eq!(output.card_action, KnowledgeCardAction::CreateNewCard);
    }

    #[test]
    fn parse_judge_output_normalizes_fact_status_to_active() {
        let mut value: serde_json::Value =
            serde_json::from_str(&valid_judge_output_json("add_new", "create_new_card")).unwrap();
        value["assertion_status"] = serde_json::Value::String("fact".to_string());

        let output = parse_judge_output(&value.to_string()).unwrap();

        assert_eq!(output.assertion_status, "active");
    }

    #[test]
    fn reject_with_unsupported_assertion_status_parses_and_validates_for_no_write() {
        let mut value: serde_json::Value =
            serde_json::from_str(&valid_judge_output_json("reject", "uncertain")).unwrap();
        value["assertion_status"] = serde_json::Value::String("unsupported".to_string());

        let output = parse_judge_output(&value.to_string()).unwrap();

        assert_eq!(output.decision, KnowledgeJudgeDecision::Reject);
        assert_eq!(output.card_action, KnowledgeCardAction::Uncertain);
        validate_judge_output_for_context(&output, 0).unwrap();
    }

    #[test]
    fn reject_with_missing_assertion_status_parses_and_validates_for_no_write() {
        let mut value: serde_json::Value =
            serde_json::from_str(&valid_judge_output_json("reject", "uncertain")).unwrap();
        value.as_object_mut().unwrap().remove("assertion_status");

        let output = parse_judge_output(&value.to_string()).unwrap();

        assert_eq!(output.decision, KnowledgeJudgeDecision::Reject);
        assert_eq!(output.card_action, KnowledgeCardAction::Uncertain);
        validate_judge_output_for_context(&output, 0).unwrap();
    }

    #[test]
    fn uncertain_card_action_with_unsupported_status_validates_for_no_write() {
        let mut value: serde_json::Value =
            serde_json::from_str(&valid_judge_output_json("add_new", "uncertain")).unwrap();
        value["assertion_status"] = serde_json::Value::String("unsupported".to_string());

        let output = parse_judge_output(&value.to_string()).unwrap();

        assert_eq!(output.card_action, KnowledgeCardAction::Uncertain);
        validate_judge_output_for_context(&output, 0).unwrap();
    }

    #[test]
    fn write_path_with_unsupported_assertion_status_fails() {
        let mut value: serde_json::Value =
            serde_json::from_str(&valid_judge_output_json("add_new", "create_new_card")).unwrap();
        value["assertion_status"] = serde_json::Value::String("unsupported".to_string());

        assert!(parse_judge_output(&value.to_string()).is_err());
    }

    #[test]
    fn invalid_output_repairs_once() {
        let raw = valid_judge_output_json("supplement", "use_existing_card");
        let truncated = raw.trim_end_matches('}').to_string();

        let output = parse_judge_output_with_repair(&truncated).unwrap();

        assert_eq!(output.decision, KnowledgeJudgeDecision::Supplement);
    }

    #[test]
    fn validates_all_supported_decisions() {
        for (decision, card_action) in [
            ("add_new", "create_new_card"),
            ("supplement", "use_existing_card"),
            ("revise_existing", "use_existing_card"),
            ("contradict_existing", "use_existing_card"),
            ("mark_rumor", "create_new_card"),
            ("mark_uncertain", "create_new_card"),
            ("mark_false_in_world", "create_new_card"),
            ("reject", "uncertain"),
        ] {
            let output =
                parse_judge_output(&valid_judge_output_json(decision, card_action)).unwrap();
            validate_judge_output_for_context(&output, 0).unwrap();
        }
    }

    #[test]
    fn validates_card_action_compatibility() {
        let invalid_revise = parse_judge_output(&valid_judge_output_json(
            "revise_existing",
            "create_new_card",
        ))
        .unwrap();
        assert!(validate_judge_output_for_context(&invalid_revise, 0).is_err());

        let invalid_create_with_candidates =
            parse_judge_output(&valid_judge_output_json("add_new", "create_new_card")).unwrap();
        assert!(validate_judge_output_for_context(&invalid_create_with_candidates, 1).is_err());

        let uncertain =
            parse_judge_output(&valid_judge_output_json("reject", "uncertain")).unwrap();
        validate_judge_output_for_context(&uncertain, 2).unwrap();
    }

    #[test]
    fn build_judge_prompt_mentions_lowercase_json_for_response_format() {
        let prompt = build_judge_prompt(&KnowledgeJudgeInput::for_test()).unwrap();

        assert!(
            prompt.contains("json") || KNOWLEDGE_JUDGE_SYSTEM_PROMPT.contains("json"),
            "json_object response format requires input messages to contain lowercase json"
        );
    }

    #[test]
    fn build_judge_prompt_mentions_assertion_status_enum_and_reject_placeholder_rule() {
        let prompt = build_judge_prompt(&KnowledgeJudgeInput::for_test()).unwrap();

        assert!(prompt.contains("active"));
        assert!(prompt.contains("rumor"));
        assert!(prompt.contains("uncertain"));
        assert!(prompt.contains("false_in_world"));
        assert!(prompt.contains("reject"));
        assert!(prompt.contains("unsupported"));
    }

    #[test]
    fn recent_same_category_context_does_not_block_create_new_card() {
        let mut input = KnowledgeJudgeInput::for_test();
        input.candidate_cards.push(TopicCandidate {
            book_id: "book1".to_string(),
            category: "power_system".to_string(),
            card_id: "recent-card".to_string(),
            topic_key: "mana-rules".to_string(),
            topic_display: "Mana Rules".to_string(),
            current_summary: None,
            match_kind: TopicCandidateMatch::RecentSameCategory,
            score: 0.2,
            last_updated_chapter: 1,
        });
        let output =
            parse_judge_output(&valid_judge_output_json("add_new", "create_new_card")).unwrap();

        validate_judge_output_for_context(&output, matching_topic_candidate_count(&input)).unwrap();
    }

    #[tokio::test]
    async fn mock_judge_returns_configured_output() {
        let output =
            parse_judge_output(&valid_judge_output_json("add_new", "create_new_card")).unwrap();
        let judge = MockKnowledgeRevisionJudge::new(output.clone());
        let input = KnowledgeJudgeInput::for_test();

        assert_eq!(judge.judge(&input).await.unwrap(), output);
    }

    #[tokio::test]
    async fn default_judge_returns_uncertain() {
        let judge = DefaultKnowledgeRevisionJudge::new();
        let input = KnowledgeJudgeInput::for_test();

        let output = judge.judge(&input).await.unwrap();

        assert_eq!(output.decision, KnowledgeJudgeDecision::Reject);
        assert_eq!(output.card_action, KnowledgeCardAction::Uncertain);
    }

    #[tokio::test]
    async fn knowledge_judge_ai_run_lifecycle_records_success() {
        use crate::storage::db;
        use crate::storage::db::v4::ai_run_repo::AiRunRepo;

        let dir = std::env::temp_dir().join(format!(
            "reader-knowledge-judge-run-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter1', 'book1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();

        let input = KnowledgeJudgeInput::for_test();
        let output =
            parse_judge_output(&valid_judge_output_json("add_new", "create_new_card")).unwrap();
        let run_id = record_knowledge_judge_success(&pool, &input, "test-model", "prompt", &output)
            .await
            .unwrap();

        let run = AiRunRepo::new(pool)
            .get_run(&run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.run_type, "knowledge_revision_judge");
        assert_eq!(run.status, "success");
        assert_eq!(run.model, "test-model");
        assert!(run.output_json.unwrap().contains("add_new"));
    }

    #[tokio::test]
    async fn knowledge_judge_ai_run_failure_preserves_raw_output() {
        use crate::model::ai_model::{AiModelConfig, AiModelEndpointConfig};
        use crate::service::ai_model_service::AiModelService;
        use crate::service::json_document_service::JsonDocumentService;
        use crate::storage::db;
        use crate::storage::db::v4::ai_run_repo::AiRunRepo;
        use axum::{routing::post, Json, Router};
        use std::sync::Arc;

        let dir = std::env::temp_dir().join(format!(
            "reader-knowledge-judge-failure-run-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter1', 'book1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async move {
                Json(serde_json::json!({
                    "choices": [{
                        "message": {
                            "content": serde_json::json!({
                                "decision": "add_new",
                                "card_action": "create_new_card",
                                "target_card_id": null,
                                "affected_assertion_ids": [],
                                "assertion_status": "unsupported",
                                "current_summary": "bad status",
                                "confidence": 0.9,
                                "reason_code": "bad_status",
                                "explanation_for_log": "write path invalid status"
                            }).to_string()
                        }
                    }]
                }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let docs = Arc::new(JsonDocumentService::new(pool.clone(), dir.to_str().unwrap()));
        let ai_service = AiModelService::new(docs, dir.to_str().unwrap());
        let mut config = AiModelConfig::default();
        config.text = AiModelEndpointConfig {
            enabled: true,
            base_url: format!("http://{}", addr),
            api_key: String::new(),
            model: "test-model".to_string(),
            path: String::new(),
            use_full_url: false,
        };
        ai_service.save(config).await.unwrap();

        let judge = RealAiKnowledgeRevisionJudge::new(Arc::new(ai_service), pool.clone());
        let err = judge.judge(&KnowledgeJudgeInput::for_test()).await.unwrap_err();
        assert!(err.to_string().contains("invalid assertion_status"));

        let runs = AiRunRepo::new(pool)
            .list_runs_by_chapter("book1", "chapter1")
            .await
            .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "failed");
        assert!(runs[0].output_json.as_deref().unwrap_or_default().contains("unsupported"));
        assert!(runs[0].error.as_deref().unwrap_or_default().contains("invalid assertion_status"));
    }
}
