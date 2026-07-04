use crate::service::ai_model_service::AiModelService;
use crate::storage::db::v4::ai_run_repo::AiRunRepo;
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo, SourceSpanRecord};
use crate::storage::db::v4::entity_repo::{EntityRecord, EntityRepo};
use crate::storage::db::v4::identity_repo::{IdentityLinkRecord, IdentityRepo};
use crate::storage::db::v4::property_repo::{CurrentPropertyRecord, PropertyRepo};
use crate::storage::db::v4::relationship_repo::{RelationshipRecord, RelationshipRepo};
use crate::util::hash::md5_hex;
use sqlx::SqlitePool;

pub const VALID_IDENTITY_LINK_TYPES: &[&str] = &[
    "same_identity",
    "possible_same_identity",
    "not_same_identity",
    "disguise",
    "alias_reveal",
    "true_name_reveal",
    "mistaken_identity",
];

pub const VALID_SURVIVOR_HINTS: &[&str] = &["entity_a", "entity_b", "unknown"];
pub const VALID_RELATIONSHIP_MIGRATION_HINTS: &[&str] =
    &["safe", "dedupe_required", "self_edge_risk", "unknown"];

#[derive(Debug, Clone, PartialEq)]
pub enum IdentityGateResult {
    Pass,
    Reject(String),
    Uncertain(String),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityJudgeDecision {
    Merge,
    PossibleSameIdentity,
    NotSameIdentity,
    Reject,
    Uncertain,
    SplitRequired,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IdentityJudgeOutput {
    pub decision: IdentityJudgeDecision,
    pub link_type: String,
    pub survivor_hint: String,
    pub confidence: f64,
    pub reason_code: String,
    pub explanation_for_log: String,
    pub property_conflicts: Vec<String>,
    pub relationship_migration_hint: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityJudgeInput {
    pub claim_id: String,
    pub claim_type: String,
    pub claim_text_summary: String,
    pub entity_a: IdentityJudgeEntityInput,
    pub entity_b: IdentityJudgeEntityInput,
    pub evidence_spans: Vec<String>,
    pub existing_identity_links: Vec<IdentityJudgeLinkInput>,
    pub blocked_reason: Option<String>,
    pub related_relationships: Vec<IdentityJudgeRelationshipInput>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityJudgeEntityInput {
    pub id: String,
    pub canonical_name: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub current_properties: Vec<IdentityJudgePropertyInput>,
    pub recent_relationships: Vec<IdentityJudgeRelationshipInput>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityJudgePropertyInput {
    pub dimension_key: String,
    pub value_text: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityJudgeRelationshipInput {
    pub other_entity_id: String,
    pub relation_group: String,
    pub relation_label: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IdentityJudgeLinkInput {
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub link_type: String,
    pub status: String,
    pub confidence: f64,
}

#[axum::async_trait]
pub trait IdentityJudge: Send + Sync {
    async fn judge(&self, claim: &ClaimRecord) -> anyhow::Result<IdentityJudgeOutput>;
}

#[derive(Default)]
pub struct DefaultIdentityJudge;

impl DefaultIdentityJudge {
    pub fn new() -> Self {
        Self
    }
}

#[axum::async_trait]
impl IdentityJudge for DefaultIdentityJudge {
    async fn judge(&self, _claim: &ClaimRecord) -> anyhow::Result<IdentityJudgeOutput> {
        Ok(IdentityJudgeOutput {
            decision: IdentityJudgeDecision::Uncertain,
            link_type: "possible_same_identity".to_string(),
            survivor_hint: "unknown".to_string(),
            confidence: 0.0,
            reason_code: "default_identity_judge".to_string(),
            explanation_for_log: "Default identity judge: no real AI judge wired yet".to_string(),
            property_conflicts: vec![],
            relationship_migration_hint: "unknown".to_string(),
        })
    }
}

pub struct MockIdentityJudge {
    pub output: IdentityJudgeOutput,
}

impl MockIdentityJudge {
    pub fn new(output: IdentityJudgeOutput) -> Self {
        Self { output }
    }
}

#[axum::async_trait]
impl IdentityJudge for MockIdentityJudge {
    async fn judge(&self, _claim: &ClaimRecord) -> anyhow::Result<IdentityJudgeOutput> {
        Ok(self.output.clone())
    }
}

pub fn structural_gate(
    claim: &ClaimRecord,
    entity_a: Option<&EntityRecord>,
    entity_b: Option<&EntityRecord>,
    blocked_by_not_same_identity: bool,
) -> IdentityGateResult {
    if claim.primary_source_span_id.trim().is_empty() {
        return IdentityGateResult::Reject("missing evidence".to_string());
    }

    if blocked_by_not_same_identity {
        return IdentityGateResult::Reject(
            "active not_same_identity link already blocks this pair".to_string(),
        );
    }

    if let (Some(left), Some(right)) = (&claim.subject_entity_id, &claim.object_entity_id) {
        if left == right {
            return IdentityGateResult::Reject("identity pair is self-referential".to_string());
        }
    }

    for (label, entity) in [("entity_a", entity_a), ("entity_b", entity_b)] {
        if let Some(entity) = entity {
            if entity.entity_type != "character" {
                return IdentityGateResult::Reject(format!(
                    "{label} entity_type is '{}', not 'character'",
                    entity.entity_type
                ));
            }
        }
    }

    if entity_a.is_none() && entity_b.is_none() {
        return IdentityGateResult::Reject("both identity sides are unresolved".to_string());
    }

    if claim.confidence < 0.5 {
        return IdentityGateResult::Uncertain("identity evidence is too weak".to_string());
    }

    if entity_a.is_none() || entity_b.is_none() {
        return IdentityGateResult::Uncertain(
            "identity pair is only partially resolved".to_string(),
        );
    }

    IdentityGateResult::Pass
}

const JUDGE_SYSTEM_PROMPT: &str = "你是小说人物身份判断 agent。判断两个人物是否为同一真实人物。不要输出 Markdown，不要输出解释，只输出严格 JSON 对象。";

pub fn parse_judge_output(raw: &str) -> anyhow::Result<IdentityJudgeOutput> {
    let cleaned = strip_markdown_fences(raw);
    let output: IdentityJudgeOutput =
        serde_json::from_str(&cleaned).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))?;
    validate_judge_output_fields(&output)?;
    Ok(output)
}

pub fn parse_judge_output_with_repair(raw: &str) -> anyhow::Result<IdentityJudgeOutput> {
    match parse_judge_output(raw) {
        Ok(output) => Ok(output),
        Err(_) => {
            let cleaned = strip_markdown_fences(raw);
            let repaired = try_repair_json(&cleaned)?;
            let output: IdentityJudgeOutput = serde_json::from_str(&repaired)
                .map_err(|e| anyhow::anyhow!("JSON parse error after repair: {}", e))?;
            validate_judge_output_fields(&output)?;
            Ok(output)
        }
    }
}

fn validate_judge_output_fields(output: &IdentityJudgeOutput) -> anyhow::Result<()> {
    if output.confidence < 0.0 || output.confidence > 1.0 {
        anyhow::bail!("confidence out of range: {}", output.confidence);
    }
    if !VALID_IDENTITY_LINK_TYPES.contains(&output.link_type.as_str()) {
        anyhow::bail!("invalid link_type: {}", output.link_type);
    }
    if !VALID_SURVIVOR_HINTS.contains(&output.survivor_hint.as_str()) {
        anyhow::bail!("invalid survivor_hint: {}", output.survivor_hint);
    }
    if !VALID_RELATIONSHIP_MIGRATION_HINTS.contains(&output.relationship_migration_hint.as_str()) {
        anyhow::bail!(
            "invalid relationship_migration_hint: {}",
            output.relationship_migration_hint
        );
    }
    Ok(())
}

pub fn validate_judge_decision(
    output: &IdentityJudgeOutput,
    pair_fully_resolved: bool,
) -> anyhow::Result<()> {
    match output.decision {
        IdentityJudgeDecision::Merge => {
            if !pair_fully_resolved {
                anyhow::bail!("merge decision requires both identity sides resolved");
            }
            if output.link_type == "not_same_identity" {
                anyhow::bail!("merge decision cannot use not_same_identity link_type");
            }
        }
        IdentityJudgeDecision::PossibleSameIdentity => {
            if output.link_type != "possible_same_identity" {
                anyhow::bail!(
                    "possible_same_identity decision requires possible_same_identity link_type"
                );
            }
        }
        IdentityJudgeDecision::NotSameIdentity => {
            if output.link_type != "not_same_identity" {
                anyhow::bail!("not_same_identity decision requires not_same_identity link_type");
            }
        }
        IdentityJudgeDecision::SplitRequired => {
            if output.link_type != "mistaken_identity" {
                anyhow::bail!("split_required decision requires mistaken_identity link_type");
            }
        }
        IdentityJudgeDecision::Reject | IdentityJudgeDecision::Uncertain => {}
    }
    Ok(())
}

fn strip_markdown_fences(s: &str) -> String {
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

fn try_repair_json(raw: &str) -> anyhow::Result<String> {
    let trimmed = raw.trim().trim_end_matches(',');
    if trimmed.ends_with('}') {
        return Ok(trimmed.to_string());
    }

    let repaired = format!("{}}}", trimmed);
    if serde_json::from_str::<serde_json::Value>(&repaired).is_ok() {
        return Ok(repaired);
    }

    anyhow::bail!("无法修复 JSON")
}

pub fn build_judge_prompt(input: &IdentityJudgeInput) -> anyhow::Result<String> {
    let input_json = serde_json::to_string_pretty(input)?;
    Ok(format!(
        "## Identity Judge Input\n{}\n\n## Instructions\n\
         判断这两个实体是否代表同一真实人物。\n\
         - merge: 明确同一人，允许后续 merge reducer 处理\n\
         - possible_same_identity: 有较强同一人信号，但不足以 merge\n\
         - not_same_identity: 明确不是同一人\n\
         - reject: 明确不成立或违反硬规则\n\
         - uncertain: 证据不足\n\
         - split_required: 历史 canonical 可能误合并，但当前不自动 split\n\n\
         硬规则：\n\
         - 没有明确 source-span 证据不能 merge\n\
         - 名字相似不能单独触发 merge\n\
         - 若 blocked_reason 非空，不得输出 merge\n\n\
         link_type 必须是以下之一： {}\n\
         survivor_hint 必须是以下之一： {}\n\
         relationship_migration_hint 必须是以下之一： {}\n\n\
         输出严格 JSON 对象，字段： decision, link_type, survivor_hint, confidence, reason_code, explanation_for_log, property_conflicts, relationship_migration_hint",
        input_json,
        VALID_IDENTITY_LINK_TYPES.join(", "),
        VALID_SURVIVOR_HINTS.join(", "),
        VALID_RELATIONSHIP_MIGRATION_HINTS.join(", "),
    ))
}

pub async fn ai_semantic_judge(
    input: &IdentityJudgeInput,
    claim: &ClaimRecord,
    ai_model_service: &AiModelService,
    pool: &SqlitePool,
) -> anyhow::Result<IdentityJudgeOutput> {
    use crate::model::ai_model::AiModelKind;
    use crate::model::ai_proxy::{ai_proxy_timeout, build_ai_proxy_url};
    use crate::service::ai_book_generation_service::extract_model_content;

    let prompt = build_judge_prompt(input)?;
    let config = ai_model_service
        .get()
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let endpoint = config.resolve(AiModelKind::Text);
    if !endpoint.enabled {
        anyhow::bail!("AI text model is disabled");
    }

    let chapter_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM chapters WHERE book_id = ? AND chapter_index = ? LIMIT 1",
    )
    .bind(&claim.book_id)
    .bind(claim.chapter_index)
    .fetch_optional(pool)
    .await?;
    let ai_run_repo = AiRunRepo::new(pool.clone());
    let ai_run = ai_run_repo
        .create_run(
            &claim.book_id,
            &chapter_id.unwrap_or_default(),
            None,
            "identity_judge",
            &endpoint.model,
            "v1",
            1,
            &md5_hex(&prompt),
        )
        .await?;

    let result = async {
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
        let output = parse_judge_output_with_repair(&content)?;
        validate_judge_decision(&output, true)?;
        Ok(output)
    }
    .await;

    match &result {
        Ok(output) => {
            ai_run_repo
                .update_run_status(
                    &ai_run.id,
                    "success",
                    Some(&serde_json::to_string(output)?),
                    None,
                )
                .await?;
        }
        Err(err) => {
            ai_run_repo
                .update_run_status(&ai_run.id, "failed", None, Some(&err.to_string()))
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
            "model": model,
            "max_tokens": 4096,
            "temperature": 0.2,
            "system": JUDGE_SYSTEM_PROMPT,
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
                { "role": "system", "content": JUDGE_SYSTEM_PROMPT },
                { "role": "user", "content": prompt }
            ]
        });
    }
    serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": JUDGE_SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    })
}

pub struct RealAiIdentityJudge {
    ai_model_service: std::sync::Arc<AiModelService>,
    pool: SqlitePool,
}

impl RealAiIdentityJudge {
    pub fn new(ai_model_service: std::sync::Arc<AiModelService>, pool: SqlitePool) -> Self {
        Self {
            ai_model_service,
            pool,
        }
    }
}

#[axum::async_trait]
impl IdentityJudge for RealAiIdentityJudge {
    async fn judge(&self, claim: &ClaimRecord) -> anyhow::Result<IdentityJudgeOutput> {
        let entity_repo = EntityRepo::new(self.pool.clone());
        let property_repo = PropertyRepo::new(self.pool.clone());
        let relationship_repo = RelationshipRepo::new(self.pool.clone());
        let identity_repo = IdentityRepo::new(self.pool.clone());
        let claim_repo = ClaimRepo::new(self.pool.clone());

        let entity_a = match &claim.subject_entity_id {
            Some(id) => entity_repo
                .get_by_id(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Entity A not found: {}", id))?,
            None => anyhow::bail!("Claim has no resolved entity_a"),
        };
        let entity_b = match &claim.object_entity_id {
            Some(id) => entity_repo
                .get_by_id(id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Entity B not found: {}", id))?,
            None => anyhow::bail!("Claim has no resolved entity_b"),
        };

        let entity_a_aliases = entity_repo.list_aliases_by_entity(&entity_a.id).await?;
        let entity_b_aliases = entity_repo.list_aliases_by_entity(&entity_b.id).await?;
        let entity_a_props = property_repo
            .list_current_properties(&claim.book_id, &entity_a.id)
            .await?;
        let entity_b_props = property_repo
            .list_current_properties(&claim.book_id, &entity_b.id)
            .await?;
        let source_spans = claim_repo
            .list_claim_spans(&claim.id)
            .await
            .unwrap_or_default();
        let existing_links = identity_repo
            .list_identity_links_by_book(&claim.book_id, Some("active"))
            .await?
            .into_iter()
            .filter(|link| {
                let ids = [&entity_a.id, &entity_b.id];
                ids.contains(&&link.entity_a_id) || ids.contains(&&link.entity_b_id)
            })
            .collect::<Vec<_>>();
        let related_relationships = collect_related_relationships(
            &relationship_repo,
            &claim.book_id,
            &entity_a.id,
            &entity_b.id,
        )
        .await?;
        let blocked_reason = existing_links.iter().find_map(|link| {
            if link.link_type == "not_same_identity" && link.status == "active" {
                Some("active not_same_identity link already exists".to_string())
            } else {
                None
            }
        });

        let input = build_identity_judge_input(
            claim,
            &entity_a,
            &entity_b,
            &entity_a_aliases,
            &entity_b_aliases,
            &entity_a_props,
            &entity_b_props,
            &source_spans,
            &existing_links,
            blocked_reason,
            &related_relationships,
        );

        ai_semantic_judge(&input, claim, &self.ai_model_service, &self.pool).await
    }
}

fn build_identity_judge_input(
    claim: &ClaimRecord,
    entity_a: &EntityRecord,
    entity_b: &EntityRecord,
    entity_a_aliases: &[crate::storage::db::v4::entity_repo::AliasRecord],
    entity_b_aliases: &[crate::storage::db::v4::entity_repo::AliasRecord],
    entity_a_props: &[CurrentPropertyRecord],
    entity_b_props: &[CurrentPropertyRecord],
    source_spans: &[SourceSpanRecord],
    existing_links: &[IdentityLinkRecord],
    blocked_reason: Option<String>,
    related_relationships: &[RelationshipRecord],
) -> IdentityJudgeInput {
    IdentityJudgeInput {
        claim_id: claim.id.clone(),
        claim_type: claim.claim_type.clone(),
        claim_text_summary: claim
            .value_text
            .clone()
            .unwrap_or_else(|| claim.predicate.clone()),
        entity_a: build_entity_input(
            entity_a,
            entity_a_aliases,
            entity_a_props,
            related_relationships,
        ),
        entity_b: build_entity_input(
            entity_b,
            entity_b_aliases,
            entity_b_props,
            related_relationships,
        ),
        evidence_spans: source_spans
            .iter()
            .map(|span| span.text_excerpt.clone())
            .collect(),
        existing_identity_links: existing_links
            .iter()
            .map(|link| IdentityJudgeLinkInput {
                entity_a_id: link.entity_a_id.clone(),
                entity_b_id: link.entity_b_id.clone(),
                link_type: link.link_type.clone(),
                status: link.status.clone(),
                confidence: link.confidence,
            })
            .collect(),
        blocked_reason,
        related_relationships: related_relationships
            .iter()
            .map(|rel| IdentityJudgeRelationshipInput {
                other_entity_id: if rel.subject_character_id == entity_a.id {
                    rel.object_character_id.clone()
                } else {
                    rel.subject_character_id.clone()
                },
                relation_group: rel.relation_group.clone(),
                relation_label: rel.relation_label.clone(),
            })
            .collect(),
    }
}

fn build_entity_input(
    entity: &EntityRecord,
    aliases: &[crate::storage::db::v4::entity_repo::AliasRecord],
    props: &[CurrentPropertyRecord],
    relationships: &[RelationshipRecord],
) -> IdentityJudgeEntityInput {
    IdentityJudgeEntityInput {
        id: entity.id.clone(),
        canonical_name: entity.canonical_name.clone(),
        display_name: entity.display_name.clone(),
        aliases: aliases.iter().map(|alias| alias.alias.clone()).collect(),
        current_properties: props
            .iter()
            .map(|prop| IdentityJudgePropertyInput {
                dimension_key: prop.dimension_key.clone(),
                value_text: prop.value_text.clone(),
            })
            .collect(),
        recent_relationships: relationships
            .iter()
            .filter_map(|rel| {
                if rel.subject_character_id == entity.id {
                    Some(IdentityJudgeRelationshipInput {
                        other_entity_id: rel.object_character_id.clone(),
                        relation_group: rel.relation_group.clone(),
                        relation_label: rel.relation_label.clone(),
                    })
                } else if rel.object_character_id == entity.id {
                    Some(IdentityJudgeRelationshipInput {
                        other_entity_id: rel.subject_character_id.clone(),
                        relation_group: rel.relation_group.clone(),
                        relation_label: rel.relation_label.clone(),
                    })
                } else {
                    None
                }
            })
            .collect(),
    }
}

async fn collect_related_relationships(
    relationship_repo: &RelationshipRepo,
    book_id: &str,
    entity_a_id: &str,
    entity_b_id: &str,
) -> anyhow::Result<Vec<RelationshipRecord>> {
    let mut rels = relationship_repo
        .list_by_character(book_id, entity_a_id)
        .await?;
    rels.extend(
        relationship_repo
            .list_by_character(book_id, entity_b_id)
            .await?,
    );
    rels.sort_by(|left, right| left.id.cmp(&right.id));
    rels.dedup_by(|left, right| left.id == right.id);
    Ok(rels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::claim_repo::ClaimRecord;
    use crate::storage::db::v4::entity_repo::EntityRepo;
    use crate::storage::db::v4::identity_repo::IdentityRepo;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, EntityRepo, IdentityRepo) {
        let dir =
            std::env::temp_dir().join(format!("reader-v4-identity-judge-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        (
            pool.clone(),
            EntityRepo::new(pool.clone()),
            IdentityRepo::new(pool),
        )
    }

    fn make_claim(
        subject_entity_id: Option<&str>,
        object_entity_id: Option<&str>,
        primary_source_span_id: &str,
    ) -> ClaimRecord {
        ClaimRecord {
            id: "claim-1".to_string(),
            book_id: "b1".to_string(),
            chapter_index: 1,
            claim_type: "identity_reveal".to_string(),
            subject_mention: Some("黑衣人".to_string()),
            object_mention: Some("张三".to_string()),
            subject_entity_id: subject_entity_id.map(|s| s.to_string()),
            object_entity_id: object_entity_id.map(|s| s.to_string()),
            predicate: "identity reveal".to_string(),
            value_text: Some("摘下面具".to_string()),
            value_json: Some(
                serde_json::json!({
                    "identity_kind": "same_identity",
                    "reveal_type": "disguise",
                    "reason_hint": "摘下面具"
                })
                .to_string(),
            ),
            primary_source_span_id: primary_source_span_id.to_string(),
            ai_run_id: "run-1".to_string(),
            confidence: 0.95,
            risk_level: "high".to_string(),
            status: "proposed".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[tokio::test]
    async fn structural_gate_rejects_active_not_same_identity_block() {
        let (_pool, entity_repo, _identity_repo) = setup().await;
        let entity_a = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.7, 1)
            .await
            .unwrap();
        let entity_b = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let claim = make_claim(Some(&entity_a.id), Some(&entity_b.id), "span-1");
        let result = structural_gate(&claim, Some(&entity_a), Some(&entity_b), true);
        assert_eq!(
            result,
            IdentityGateResult::Reject(
                "active not_same_identity link already blocks this pair".to_string()
            )
        );
    }

    #[tokio::test]
    async fn structural_gate_passes_character_pair_with_evidence() {
        let (_pool, entity_repo, _identity_repo) = setup().await;
        let entity_a = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.7, 1)
            .await
            .unwrap();
        let entity_b = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        let claim = make_claim(Some(&entity_a.id), Some(&entity_b.id), "span-1");
        let result = structural_gate(&claim, Some(&entity_a), Some(&entity_b), false);
        assert_eq!(result, IdentityGateResult::Pass);
    }

    #[test]
    fn parse_identity_judge_output_accepts_merge_schema() {
        let raw = r#"{
            "decision": "merge",
            "link_type": "same_identity",
            "survivor_hint": "entity_b",
            "confidence": 0.96,
            "reason_code": "explicit_reveal",
            "explanation_for_log": "explicit reveal evidence",
            "property_conflicts": [],
            "relationship_migration_hint": "safe"
        }"#;

        let output = parse_judge_output(raw).unwrap();
        assert_eq!(output.decision, IdentityJudgeDecision::Merge);
        assert_eq!(output.link_type, "same_identity");
        assert_eq!(output.survivor_hint, "entity_b");
    }
}
