/// Observation types extracted from text by the AI extractor.

/// Trait for extracting observations from chapter context.
///
/// Implementations:
/// - `MockExtractor`: returns pre-configured observations for testing
/// - `AiExtractor`: placeholder for real AI extraction (TODO)
#[axum::async_trait]
pub trait Extractor: Send + Sync {
    async fn extract(&self, context: &str, span_ids: &[String])
        -> anyhow::Result<Vec<Observation>>;
}

/// Mock extractor that returns pre-configured observations.
/// Used for testing the pipeline end-to-end without AI calls.
pub struct MockExtractor {
    observations: Vec<Observation>,
}

impl MockExtractor {
    pub fn new(observations: Vec<Observation>) -> Self {
        Self { observations }
    }
}

#[axum::async_trait]
impl Extractor for MockExtractor {
    async fn extract(
        &self,
        _context: &str,
        span_ids: &[String],
    ) -> anyhow::Result<Vec<Observation>> {
        // Inject real span_ids into observations that have evidence spans
        Ok(self
            .observations
            .iter()
            .map(|obs| obs.clone().with_evidence_span_ids(span_ids.to_vec()))
            .collect())
    }
}

/// Placeholder AI extractor. Returns empty observations until AI service is integrated.
pub struct AiExtractor;

impl AiExtractor {
    pub fn new() -> Self {
        Self
    }
}

#[axum::async_trait]
impl Extractor for AiExtractor {
    async fn extract(
        &self,
        _context: &str,
        _span_ids: &[String],
    ) -> anyhow::Result<Vec<Observation>> {
        // TODO: Integrate actual AI extraction when AI service is available.
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct KnowledgeEntityMention {
    pub mention: String,
    pub entity_type_hint: Option<String>,
    pub role: String,
    pub confidence: f64,
    pub resolved_entity_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum Observation {
    EntityIntroduction {
        subject_mention: String,
        entity_type: String,
        aliases: Vec<String>,
        short_summary: String,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    Alias {
        subject_mention: String,
        alias: String,
        alias_type: String,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    PropertyUpdate {
        subject_mention: String,
        dimension_key: String,
        value_text: Option<String>,
        value_json: Option<serde_json::Value>,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    MinorEvent {
        description: String,
        involved_mentions: Vec<String>,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    RelationshipUpdate {
        subject_mention: String,
        object_mention: String,
        relation_hint: String,
        relation_group: String,
        relation_label: String,
        directionality: String,
        evidence_span_ids: Vec<String>,
        confidence: f64,
        importance_hint: f64,
        is_long_term_or_significant_hint: bool,
    },
    KnowledgeAssertion {
        category: String,
        topic: String,
        assertion_text: String,
        confidence: f64,
        importance_score: f64,
        evidence_span_ids: Vec<String>,
        referenced_entity_mentions: Vec<KnowledgeEntityMention>,
        status_hint: Option<String>,
        reason_hint: Option<String>,
    },
    IdentityReveal {
        revealed_mention: String,
        canonical_mention: String,
        reveal_type: String,
        reason_hint: Option<String>,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    EntityMergeCandidate {
        entity_a_mention: String,
        entity_b_mention: String,
        reason_hint: Option<String>,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    EntitySplitCandidate {
        entity_a_mention: String,
        entity_b_mention: String,
        reason_hint: Option<String>,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    NotSameIdentity {
        entity_a_mention: String,
        entity_b_mention: String,
        reason_hint: Option<String>,
        evidence_span_ids: Vec<String>,
        confidence: f64,
    },
    Summary {
        summary: String,
        key_points: Vec<String>,
        has_important_changes: bool,
    },
}

impl Observation {
    /// Returns the subject mention if this observation has one.
    pub fn subject_mention(&self) -> Option<&str> {
        match self {
            Observation::EntityIntroduction {
                subject_mention, ..
            } => Some(subject_mention),
            Observation::Alias {
                subject_mention, ..
            } => Some(subject_mention),
            Observation::PropertyUpdate {
                subject_mention, ..
            } => Some(subject_mention),
            Observation::RelationshipUpdate {
                subject_mention, ..
            } => Some(subject_mention),
            Observation::IdentityReveal {
                revealed_mention, ..
            } => Some(revealed_mention),
            Observation::EntityMergeCandidate {
                entity_a_mention, ..
            }
            | Observation::EntitySplitCandidate {
                entity_a_mention, ..
            }
            | Observation::NotSameIdentity {
                entity_a_mention, ..
            } => Some(entity_a_mention),
            Observation::KnowledgeAssertion { .. } => None,
            Observation::MinorEvent { .. } => None,
            Observation::Summary { .. } => None,
        }
    }

    /// Returns the evidence span IDs.
    pub fn evidence_span_ids(&self) -> &[String] {
        match self {
            Observation::EntityIntroduction {
                evidence_span_ids, ..
            } => evidence_span_ids,
            Observation::Alias {
                evidence_span_ids, ..
            } => evidence_span_ids,
            Observation::PropertyUpdate {
                evidence_span_ids, ..
            } => evidence_span_ids,
            Observation::MinorEvent {
                evidence_span_ids, ..
            } => evidence_span_ids,
            Observation::RelationshipUpdate {
                evidence_span_ids, ..
            } => evidence_span_ids,
            Observation::IdentityReveal {
                evidence_span_ids, ..
            }
            | Observation::EntityMergeCandidate {
                evidence_span_ids, ..
            }
            | Observation::EntitySplitCandidate {
                evidence_span_ids, ..
            }
            | Observation::NotSameIdentity {
                evidence_span_ids, ..
            }
            | Observation::KnowledgeAssertion {
                evidence_span_ids, ..
            } => evidence_span_ids,
            Observation::Summary { .. } => &[],
        }
    }

    /// Replace evidence span IDs with the given ones. Used by MockExtractor
    /// to inject real span IDs from the pipeline into pre-configured observations.
    pub fn with_evidence_span_ids(mut self, span_ids: Vec<String>) -> Self {
        match &mut self {
            Observation::EntityIntroduction {
                evidence_span_ids, ..
            }
            | Observation::Alias {
                evidence_span_ids, ..
            }
            | Observation::PropertyUpdate {
                evidence_span_ids, ..
            }
            | Observation::MinorEvent {
                evidence_span_ids, ..
            }
            | Observation::RelationshipUpdate {
                evidence_span_ids, ..
            }
            | Observation::IdentityReveal {
                evidence_span_ids, ..
            }
            | Observation::EntityMergeCandidate {
                evidence_span_ids, ..
            }
            | Observation::EntitySplitCandidate {
                evidence_span_ids, ..
            }
            | Observation::NotSameIdentity {
                evidence_span_ids, ..
            }
            | Observation::KnowledgeAssertion {
                evidence_span_ids, ..
            } => *evidence_span_ids = span_ids,
            Observation::Summary { .. } => {}
        }
        self
    }
}

/// Risk level for an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }
}

/// Classify the risk level of an observation.
///
/// Rules:
/// - High: death, resurrection, identity_reveal, entity_merge_candidate, major life_status change
/// - Medium: important character state change, new important entity, affiliation change
/// - Low: normal property update, alias, entity introduction, location
pub fn classify_risk(observation: &Observation) -> RiskLevel {
    match observation {
        Observation::PropertyUpdate {
            dimension_key,
            value_text,
            ..
        } => classify_property_risk(dimension_key, value_text.as_deref()),
        Observation::EntityIntroduction {
            entity_type,
            confidence,
            ..
        } => {
            // New entities are low risk by default; medium if high confidence character
            if entity_type == "character" && *confidence >= 0.8 {
                RiskLevel::Medium
            } else {
                RiskLevel::Low
            }
        }
        Observation::Alias { .. } => RiskLevel::Low,
        Observation::MinorEvent { .. } => RiskLevel::Low,
        Observation::RelationshipUpdate { .. } => RiskLevel::High,
        Observation::KnowledgeAssertion { .. } => RiskLevel::High,
        Observation::IdentityReveal { .. }
        | Observation::EntityMergeCandidate { .. }
        | Observation::EntitySplitCandidate { .. }
        | Observation::NotSameIdentity { .. } => RiskLevel::High,
        Observation::Summary { .. } => RiskLevel::Low,
    }
}

fn classify_property_risk(dimension_key: &str, value_text: Option<&str>) -> RiskLevel {
    let value_lower = value_text.unwrap_or("").to_lowercase();

    // High risk: death/resurrection keywords in life_status
    if dimension_key == "life_status" {
        if contains_any(
            &value_lower,
            &[
                "死亡",
                "去世",
                "死去",
                "dead",
                "died",
                "killed",
                "death",
                "deceased",
                "perished",
                "slain",
                "passed away",
                "assassinated",
            ],
        ) {
            return RiskLevel::High;
        }
        if contains_any(
            &value_lower,
            &["复活", "重生", "resurrected", "revived", "resurrection"],
        ) {
            return RiskLevel::High;
        }
        // Other life_status changes are important
        return RiskLevel::Medium;
    }

    // High risk: identity reveal or merge candidate hints
    if dimension_key == "identity" {
        if contains_any(
            &value_lower,
            &["身份揭示", "真实身份", "identity reveal", "unmasked"],
        ) {
            return RiskLevel::High;
        }
        if contains_any(
            &value_lower,
            &["entity_merge_candidate", "合并", "同一个人", "merge"],
        ) {
            return RiskLevel::High;
        }
        return RiskLevel::Medium;
    }

    // Medium risk: affiliation changes
    if dimension_key == "affiliation" {
        return RiskLevel::Medium;
    }

    // Medium risk: realm/rank changes (important state changes)
    if dimension_key == "realm" || dimension_key == "rank" {
        return RiskLevel::Medium;
    }

    // Default: low risk
    RiskLevel::Low
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

// --- Real AI Extractor ---

const V4_SYSTEM_PROMPT: &str = "你是小说人物信息抽取 agent。从章节片段中抽取结构化 observations。\n不要输出 Markdown，不要输出解释，只输出严格 JSON 数组。";

const VALID_DIMENSION_KEYS: &[&str] = &[
    "identity",
    "life_status",
    "affiliation",
    "rank",
    "occupation",
    "realm",
    "ability",
    "equipment",
    "location",
    "mental_state",
    "goal",
    "injury",
    "appearance",
    "background",
];

pub const VALID_RELATION_GROUPS: &[&str] = &[
    "family",
    "romance",
    "friendship",
    "mentorship",
    "hierarchy",
    "alliance",
    "rivalry",
    "hostility",
    "debt_obligation",
    "contract",
    "acquaintance",
    "other_social",
    "unknown_significant",
];

pub const VALID_DIRECTIONALITIES: &[&str] = &["directed", "undirected"];

pub const VALID_POLARITIES: &[&str] = &["positive", "negative", "mixed", "neutral", "unknown"];

pub const VALID_IDENTITY_REVEAL_TYPES: &[&str] = &[
    "true_name",
    "disguise",
    "title_reveal",
    "alias_reveal",
    "identity_confirmation",
    "mistaken_identity_correction",
];

pub const VALID_KNOWLEDGE_CATEGORIES: &[&str] = &[
    "power_system",
    "faction_structure",
    "world_rule",
    "history",
    "secret",
    "prophecy",
    "politics",
    "geography",
    "custom",
];

pub const VALID_KNOWLEDGE_STATUS_HINTS: &[&str] = &["fact", "rumor", "uncertain", "false_belief"];

pub const VALID_KNOWLEDGE_ENTITY_ROLES: &[&str] = &[
    "subject", "related", "source", "target", "location", "faction", "ability", "realm",
];

/// Real AI extractor that calls the configured LLM provider.
pub struct RealAiExtractor {
    ai_model_service: std::sync::Arc<crate::service::ai_model_service::AiModelService>,
}

impl RealAiExtractor {
    pub fn new(
        ai_model_service: std::sync::Arc<crate::service::ai_model_service::AiModelService>,
    ) -> Self {
        Self { ai_model_service }
    }
}

#[axum::async_trait]
impl Extractor for RealAiExtractor {
    async fn extract(
        &self,
        context: &str,
        span_ids: &[String],
    ) -> anyhow::Result<Vec<Observation>> {
        use crate::model::ai_model::AiModelKind;
        use crate::model::ai_proxy::{ai_proxy_timeout, build_ai_proxy_url};
        use crate::service::ai_book_generation_service::extract_model_content;

        // 1. Load AI config
        let config = self
            .ai_model_service
            .get()
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let endpoint = config.resolve(AiModelKind::Text);

        if !endpoint.enabled {
            anyhow::bail!("AI text model is disabled");
        }

        // 2. Build prompt
        let span_ids_json = serde_json::to_string(span_ids)?;
        let user_prompt = format!(
            "## Context\n{}\n\n## Task\n从上述章节片段中抽取以下类型的 observations：\n\
             - entity_introduction: 新人物出场（subject_mention, entity_type, aliases, short_summary, evidence_span_ids, confidence）\n\
             - alias: 已知人物的新称呼（subject_mention, alias, alias_type, evidence_span_ids, confidence）\n\
             - property_update: 人物属性变化（subject_mention, dimension_key, value_text, evidence_span_ids, confidence）\n\
             - minor_event: 值得注意的事件（description, involved_mentions, evidence_span_ids, confidence）\n\
             - relationship_update: 重要人物关系（subject_mention, object_mention, relation_hint, relation_group, relation_label, directionality, evidence_span_ids, confidence, importance_hint, is_long_term_or_significant_hint）\n\
             - knowledge_assertion: 长期世界观知识（category, topic, assertion_text, evidence_span_ids, referenced_entity_mentions, status_hint, reason_hint, confidence, importance_score）\n\
             - identity_reveal: 明确身份揭示（revealed_mention, canonical_mention, reveal_type, reason_hint, evidence_span_ids, confidence）\n\
             - entity_merge_candidate: 明确同一人候选（entity_a_mention, entity_b_mention, reason_hint, evidence_span_ids, confidence）\n\
             - entity_split_candidate: 可能误合并候选（entity_a_mention, entity_b_mention, reason_hint, evidence_span_ids, confidence）\n\
             - not_same_identity: 明确不是同一人（entity_a_mention, entity_b_mention, reason_hint, evidence_span_ids, confidence）\n\
             - summary: 章节摘要（summary, key_points, has_important_changes）\n\n\
             dimension_key 必须是以下之一：{}\n\n\
             relation_group 必须是以下之一：{}\n\n\
             identity reveal 的 reveal_type 必须是以下之一：{}\n\n\
             knowledge category 必须是以下之一：{}\n\n\
             knowledge status_hint 只能是 fact / rumor / uncertain / false_belief；secret / prophecy 必须用 category 表达。\n\n\
             evidence_span_ids 必须使用以下可用 span IDs：{}\n\n\
             knowledge_assertion 只抽长期有用的世界知识：修炼体系、势力结构、世界规则、历史、秘密、预言、政治、地理概况。\n\
             不要把人物当前状态、人物关系、身份揭露、地点方向/地图边抽成 knowledge：\n\
             - “张三突破金丹” -> property_update(realm)，不是 knowledge_assertion\n\
             - “张三和李四结盟” -> relationship_update，不是 knowledge_assertion\n\
             - “黑衣人其实是张三” -> identity_reveal，不是 knowledge_assertion\n\
             - “青云门在东域以北” -> Phase 5 map/location edge；Phase 4 只可抽地理概况，不写拓扑边\n\n\
             普通 alias / title 不是 identity_reveal：\n\
             - “张三又名张三丰” -> alias / name property, not merge by default\n\
             - “张三被称为剑魔” -> title / alias, not merge\n\
             - “黑衣人摘下面具，竟是张三” -> identity_reveal, merge candidate\n\
             - “白发老者正是李真人” -> identity_reveal, merge candidate\n\
             - “此张三并非彼张三” -> not_same_identity\n\n\
             输出严格 JSON 数组，每个元素包含 \"type\" 字段。",
            context,
            VALID_DIMENSION_KEYS.join(", "),
            VALID_RELATION_GROUPS.join(", "),
            VALID_IDENTITY_REVEAL_TYPES.join(", "),
            VALID_KNOWLEDGE_CATEGORIES.join(", "),
            span_ids_json
        );

        // 3. Call model
        let path = if endpoint.path.trim().is_empty() {
            "/v1/chat/completions"
        } else {
            endpoint.path.trim()
        };
        let target = build_ai_proxy_url(&endpoint.base_url, path, endpoint.use_full_url)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let body = build_v4_model_body(path, &endpoint.model, &user_prompt);
        std::fs::write(
            "/tmp/v4_debug_prompt.txt",
            format!("len={}", user_prompt.len()),
        )
        .ok();
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

        // 4. Parse + validate
        let observations = parse_observations_from_json(&content, span_ids)?;

        Ok(observations)
    }
}

fn build_v4_model_body(path: &str, model: &str, prompt: &str) -> serde_json::Value {
    let is_gemini =
        crate::service::ai_book_generation_service::is_gemini_generate_content_path(path);
    let is_anthropic = crate::service::ai_book_generation_service::is_anthropic_messages_path(path);
    let is_responses = crate::service::ai_book_generation_service::is_responses_path(path);

    if is_gemini {
        return serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            "systemInstruction": { "parts": [{ "text": V4_SYSTEM_PROMPT }] },
            "generationConfig": { "temperature": 0.2, "maxOutputTokens": 8192, "responseMimeType": "application/json" }
        });
    }
    if is_anthropic {
        return serde_json::json!({
            "model": model, "max_tokens": 8192, "temperature": 0.2,
            "system": V4_SYSTEM_PROMPT,
            "messages": [{ "role": "user", "content": prompt }]
        });
    }
    if is_responses {
        return serde_json::json!({
            "model": model, "temperature": 0.2, "max_output_tokens": 8192, "stream": false,
            "text": { "format": { "type": "json_object" } },
            "input": [
                { "role": "system", "content": V4_SYSTEM_PROMPT },
                { "role": "user", "content": prompt }
            ]
        });
    }
    serde_json::json!({
        "model": model, "temperature": 0.2, "max_tokens": 8192,
        "messages": [
            { "role": "system", "content": V4_SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    })
}

/// Parse observations from model JSON output.
/// Handles markdown fences, validates evidence_span_ids, dimension_key, confidence.
pub fn parse_observations_from_json(
    raw: &str,
    valid_span_ids: &[String],
) -> anyhow::Result<Vec<Observation>> {
    let cleaned = strip_markdown_fences(raw);

    // Try parsing as array first, then as object with "observations" key
    let arr: Vec<serde_json::Value> = serde_json::from_str(&cleaned)
        .or_else(|_| {
            // Try extracting array from wrapper object
            let obj: serde_json::Value = serde_json::from_str(&cleaned)?;
            if let Some(arr) = obj.get("observations").and_then(|v| v.as_array()) {
                Ok(arr.clone())
            } else if let Some(arr) = obj.get("data").and_then(|v| v.as_array()) {
                Ok(arr.clone())
            } else if let Some(arr) = obj.get("results").and_then(|v| v.as_array()) {
                Ok(arr.clone())
            } else {
                // Single object → wrap in array
                if obj.is_object() {
                    Ok(vec![obj])
                } else {
                    Err(serde_json::from_str::<serde_json::Value>("").unwrap_err())
                }
            }
        })
        .or_else(|_: serde_json::Error| {
            // Try to repair truncated JSON by finding the last complete object
            let trimmed = cleaned.trim_end();
            if let Some(pos) = trimmed.rfind('}') {
                let attempt = format!("{}]", &trimmed[..=pos]);
                serde_json::from_str(&attempt)
            } else {
                Err(serde_json::from_str::<serde_json::Value>(&cleaned).unwrap_err())
            }
        })?;

    let mut observations = Vec::new();
    let span_set: std::collections::HashSet<&String> = valid_span_ids.iter().collect();

    for item in &arr {
        let obs_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        // Normalize type: accept both snake_case and PascalCase
        let normalized_type = match obs_type {
            "entity_introduction" | "EntityIntroduction" | "entityIntroduction" => {
                "entity_introduction"
            }
            "alias" | "Alias" => "alias",
            "property_update" | "PropertyUpdate" | "propertyUpdate" => "property_update",
            "minor_event" | "MinorEvent" | "minorEvent" => "minor_event",
            "relationship_update" | "RelationshipUpdate" | "relationshipUpdate" => {
                "relationship_update"
            }
            "knowledge_assertion" | "KnowledgeAssertion" | "knowledgeAssertion" => {
                "knowledge_assertion"
            }
            "identity_reveal" | "IdentityReveal" | "identityReveal" => "identity_reveal",
            "entity_merge_candidate" | "EntityMergeCandidate" | "entityMergeCandidate" => {
                "entity_merge_candidate"
            }
            "entity_split_candidate" | "EntitySplitCandidate" | "entitySplitCandidate" => {
                "entity_split_candidate"
            }
            "not_same_identity" | "NotSameIdentity" | "notSameIdentity" => "not_same_identity",
            "summary" | "Summary" => "summary",
            other => other,
        };
        match normalized_type {
            "entity_introduction" => {
                let subject_mention = required_str(item, "subject_mention")?;
                let entity_type = optional_str(item, "entity_type").unwrap_or("character");
                let aliases = optional_str_array(item, "aliases");
                let short_summary = optional_str(item, "short_summary")
                    .unwrap_or_default()
                    .to_string();
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::EntityIntroduction {
                    subject_mention,
                    entity_type: entity_type.to_string(),
                    aliases,
                    short_summary,
                    evidence_span_ids,
                    confidence,
                });
            }
            "alias" => {
                let subject_mention = required_str(item, "subject_mention")?;
                let alias = required_str(item, "alias")?;
                let alias_type = optional_str(item, "alias_type").unwrap_or("alias");
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::Alias {
                    subject_mention,
                    alias,
                    alias_type: alias_type.to_string(),
                    evidence_span_ids,
                    confidence,
                });
            }
            "property_update" => {
                let subject_mention = required_str(item, "subject_mention")?;
                let dimension_key = required_str(item, "dimension_key")?;
                if !VALID_DIMENSION_KEYS.contains(&dimension_key.as_str()) {
                    anyhow::bail!("Invalid dimension_key: {}", dimension_key);
                }
                let value_text = optional_str(item, "value_text").map(|s| s.to_string());
                let value_json = item.get("value_json").cloned();
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::PropertyUpdate {
                    subject_mention,
                    dimension_key,
                    value_text,
                    value_json,
                    evidence_span_ids,
                    confidence,
                });
            }
            "minor_event" => {
                let description = required_str(item, "description")?;
                let involved_mentions = optional_str_array(item, "involved_mentions");
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::MinorEvent {
                    description,
                    involved_mentions,
                    evidence_span_ids,
                    confidence,
                });
            }
            "relationship_update" => {
                let subject_mention = required_str(item, "subject_mention")?;
                if subject_mention.trim().is_empty() {
                    anyhow::bail!("subject_mention is empty");
                }
                let object_mention = required_str(item, "object_mention")?;
                if object_mention.trim().is_empty() {
                    anyhow::bail!("object_mention is empty");
                }
                let relation_hint = required_str(item, "relation_hint")?;
                let relation_group = required_str(item, "relation_group")?;
                // Lenient validation: fallback to "unknown_significant" if invalid
                let relation_group = if VALID_RELATION_GROUPS.contains(&relation_group.as_str()) {
                    relation_group
                } else {
                    "unknown_significant".to_string()
                };
                let relation_label = required_str(item, "relation_label")?;
                let directionality = required_str(item, "directionality")?;
                // Lenient validation: fallback to "directed" if invalid
                let directionality = if VALID_DIRECTIONALITIES.contains(&directionality.as_str()) {
                    directionality
                } else {
                    "directed".to_string()
                };
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                let importance_hint = item
                    .get("importance_hint")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5);
                let is_long_term_or_significant_hint = item
                    .get("is_long_term_or_significant_hint")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                observations.push(Observation::RelationshipUpdate {
                    subject_mention,
                    object_mention,
                    relation_hint,
                    relation_group,
                    relation_label,
                    directionality,
                    evidence_span_ids,
                    confidence,
                    importance_hint,
                    is_long_term_or_significant_hint,
                });
            }
            "knowledge_assertion" => {
                reject_mistyped_knowledge_shape(item)?;
                let category = required_str(item, "category")?;
                if !VALID_KNOWLEDGE_CATEGORIES.contains(&category.as_str()) {
                    anyhow::bail!("Invalid knowledge category: {}", category);
                }
                let topic = required_str(item, "topic")?;
                if topic.trim().is_empty() {
                    anyhow::bail!("knowledge topic is empty");
                }
                let assertion_text = required_str(item, "assertion_text")?;
                if assertion_text.trim().is_empty() {
                    anyhow::bail!("knowledge assertion_text is empty");
                }
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                let importance_score = validate_unit_f64(item, "importance_score", 0.5)?;
                let status_hint = optional_str(item, "status_hint").map(|s| s.to_string());
                if let Some(status_hint) = status_hint.as_deref() {
                    if !VALID_KNOWLEDGE_STATUS_HINTS.contains(&status_hint) {
                        anyhow::bail!("Invalid knowledge status_hint: {}", status_hint);
                    }
                }
                let referenced_entity_mentions = parse_knowledge_entity_mentions(item)?;
                let reason_hint = optional_str(item, "reason_hint").map(|s| s.to_string());
                observations.push(Observation::KnowledgeAssertion {
                    category,
                    topic,
                    assertion_text,
                    confidence,
                    importance_score,
                    evidence_span_ids,
                    referenced_entity_mentions,
                    status_hint,
                    reason_hint,
                });
            }
            "summary" => {
                let summary = required_str(item, "summary")?;
                let key_points = optional_str_array(item, "key_points");
                let has_important_changes = item
                    .get("has_important_changes")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                observations.push(Observation::Summary {
                    summary,
                    key_points,
                    has_important_changes,
                });
            }
            "identity_reveal" => {
                let revealed_mention = required_str(item, "revealed_mention")?;
                let canonical_mention = required_str(item, "canonical_mention")?;
                let reveal_type = required_str(item, "reveal_type")?;
                if !VALID_IDENTITY_REVEAL_TYPES.contains(&reveal_type.as_str()) {
                    anyhow::bail!("Invalid reveal_type: {}", reveal_type);
                }
                let reason_hint = optional_str(item, "reason_hint").map(|s| s.to_string());
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::IdentityReveal {
                    revealed_mention,
                    canonical_mention,
                    reveal_type,
                    reason_hint,
                    evidence_span_ids,
                    confidence,
                });
            }
            "entity_merge_candidate" => {
                let entity_a_mention = required_str(item, "entity_a_mention")?;
                let entity_b_mention = required_str(item, "entity_b_mention")?;
                let reason_hint = optional_str(item, "reason_hint").map(|s| s.to_string());
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::EntityMergeCandidate {
                    entity_a_mention,
                    entity_b_mention,
                    reason_hint,
                    evidence_span_ids,
                    confidence,
                });
            }
            "entity_split_candidate" => {
                let entity_a_mention = required_str(item, "entity_a_mention")?;
                let entity_b_mention = required_str(item, "entity_b_mention")?;
                let reason_hint = optional_str(item, "reason_hint").map(|s| s.to_string());
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::EntitySplitCandidate {
                    entity_a_mention,
                    entity_b_mention,
                    reason_hint,
                    evidence_span_ids,
                    confidence,
                });
            }
            "not_same_identity" => {
                let entity_a_mention = required_str(item, "entity_a_mention")?;
                let entity_b_mention = required_str(item, "entity_b_mention")?;
                let reason_hint = optional_str(item, "reason_hint").map(|s| s.to_string());
                let evidence_span_ids = validate_evidence_spans(item, &span_set)?;
                let confidence = validate_confidence(item)?;
                observations.push(Observation::NotSameIdentity {
                    entity_a_mention,
                    entity_b_mention,
                    reason_hint,
                    evidence_span_ids,
                    confidence,
                });
            }
            other => {
                anyhow::bail!("Unknown observation type: {}", other);
            }
        }
    }

    Ok(observations)
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

fn required_str(item: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    item.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Missing required field: {}", key))
}

fn optional_str<'a>(item: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    item.get(key).and_then(|v| v.as_str())
}

fn optional_str_array(item: &serde_json::Value, key: &str) -> Vec<String> {
    item.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_knowledge_entity_mentions(
    item: &serde_json::Value,
) -> anyhow::Result<Vec<KnowledgeEntityMention>> {
    let Some(arr) = item
        .get("referenced_entity_mentions")
        .and_then(|v| v.as_array())
    else {
        return Ok(Vec::new());
    };

    let mut mentions = Vec::new();
    for entity in arr {
        let Some(mention) = optional_str(entity, "mention")
            .map(str::trim)
            .filter(|mention| !mention.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        let role = optional_str(entity, "role")
            .unwrap_or("related")
            .to_string();
        if !VALID_KNOWLEDGE_ENTITY_ROLES.contains(&role.as_str()) {
            anyhow::bail!("Invalid knowledge entity role: {}", role);
        }
        let confidence = validate_unit_f64(entity, "confidence", 0.5)?;
        mentions.push(KnowledgeEntityMention {
            mention,
            entity_type_hint: optional_str(entity, "entity_type_hint").map(|s| s.to_string()),
            role,
            confidence,
            resolved_entity_id: None,
        });
    }
    Ok(mentions)
}

fn reject_mistyped_knowledge_shape(item: &serde_json::Value) -> anyhow::Result<()> {
    let forbidden = [
        "subject_mention",
        "dimension_key",
        "object_mention",
        "relation_group",
        "revealed_mention",
        "canonical_mention",
        "reveal_type",
        "from_place_mention",
        "to_place_mention",
        "edge_type",
    ];
    for key in forbidden {
        if item.get(key).is_some() {
            anyhow::bail!("knowledge_assertion contains non-knowledge field: {}", key);
        }
    }
    Ok(())
}

fn validate_evidence_spans(
    item: &serde_json::Value,
    valid: &std::collections::HashSet<&String>,
) -> anyhow::Result<Vec<String>> {
    let spans = optional_str_array(item, "evidence_span_ids");
    if spans.is_empty() {
        anyhow::bail!("evidence_span_ids is empty");
    }
    for s in &spans {
        if !valid.contains(s) {
            anyhow::bail!("Invalid evidence_span_id: {}", s);
        }
    }
    Ok(spans)
}

fn validate_confidence(item: &serde_json::Value) -> anyhow::Result<f64> {
    let c = item
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    if c < 0.0 || c > 1.0 {
        anyhow::bail!("confidence out of range: {}", c);
    }
    Ok(c)
}

fn validate_unit_f64(item: &serde_json::Value, key: &str, default: f64) -> anyhow::Result<f64> {
    let value = item.get(key).and_then(|v| v.as_f64()).unwrap_or(default);
    if !(0.0..=1.0).contains(&value) {
        anyhow::bail!("{} out of range: {}", key, value);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_risk_death_is_high() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "张三".to_string(),
            dimension_key: "life_status".to_string(),
            value_text: Some("死亡".to_string()),
            value_json: None,
            evidence_span_ids: vec!["span1".to_string()],
            confidence: 0.9,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::High);
    }

    #[test]
    fn classify_risk_death_english_is_high() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "John".to_string(),
            dimension_key: "life_status".to_string(),
            value_text: Some("was killed in battle".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.8,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::High);
    }

    #[test]
    fn classify_risk_death_euphemisms_are_high() {
        for text in &[
            "passed away",
            "deceased",
            "perished",
            "slain",
            "assassinated",
        ] {
            let obs = Observation::PropertyUpdate {
                subject_mention: "John".to_string(),
                dimension_key: "life_status".to_string(),
                value_text: Some(text.to_string()),
                value_json: None,
                evidence_span_ids: vec![],
                confidence: 0.8,
            };
            assert_eq!(
                classify_risk(&obs),
                RiskLevel::High,
                "expected High for '{}'",
                text
            );
        }
    }

    #[test]
    fn classify_risk_resurrection_is_high() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "张三".to_string(),
            dimension_key: "life_status".to_string(),
            value_text: Some("复活".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.7,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::High);
    }

    #[test]
    fn classify_risk_identity_reveal_is_high() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "神秘人".to_string(),
            dimension_key: "identity".to_string(),
            value_text: Some("真实身份揭示：张三".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.9,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::High);
    }

    #[test]
    fn classify_risk_entity_merge_candidate_is_high() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "神秘人".to_string(),
            dimension_key: "identity".to_string(),
            value_text: Some("entity_merge_candidate: 可能与张三是同一个人".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.85,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::High);
    }

    #[test]
    fn classify_risk_normal_property_is_low() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "张三".to_string(),
            dimension_key: "location".to_string(),
            value_text: Some("大殿".to_string()),
            value_json: None,
            evidence_span_ids: vec!["span1".to_string()],
            confidence: 0.8,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Low);
    }

    #[test]
    fn classify_risk_normal_realm_is_medium() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "张三".to_string(),
            dimension_key: "realm".to_string(),
            value_text: Some("筑基期".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.8,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Medium);
    }

    #[test]
    fn classify_risk_affiliation_change_is_medium() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "李四".to_string(),
            dimension_key: "affiliation".to_string(),
            value_text: Some("魔道".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.7,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Medium);
    }

    #[test]
    fn classify_risk_life_status_general_is_medium() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "张三".to_string(),
            dimension_key: "life_status".to_string(),
            value_text: Some("重伤昏迷".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.8,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Medium);
    }

    #[test]
    fn classify_risk_alias_is_low() {
        let obs = Observation::Alias {
            subject_mention: "张三".to_string(),
            alias: "小三".to_string(),
            alias_type: "nickname".to_string(),
            evidence_span_ids: vec![],
            confidence: 0.8,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Low);
    }

    #[test]
    fn classify_risk_entity_intro_character_is_medium() {
        let obs = Observation::EntityIntroduction {
            subject_mention: "新角色".to_string(),
            entity_type: "character".to_string(),
            aliases: vec![],
            short_summary: "一个强大的修士".to_string(),
            evidence_span_ids: vec![],
            confidence: 0.9,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Medium);
    }

    #[test]
    fn classify_risk_entity_intro_low_confidence_is_low() {
        let obs = Observation::EntityIntroduction {
            subject_mention: "路人甲".to_string(),
            entity_type: "character".to_string(),
            aliases: vec![],
            short_summary: "路人".to_string(),
            evidence_span_ids: vec![],
            confidence: 0.5,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Low);
    }

    #[test]
    fn classify_risk_entity_intro_non_character_is_low() {
        let obs = Observation::EntityIntroduction {
            subject_mention: "天剑宗".to_string(),
            entity_type: "faction".to_string(),
            aliases: vec![],
            short_summary: "一个修仙门派".to_string(),
            evidence_span_ids: vec![],
            confidence: 0.9,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Low);
    }

    #[test]
    fn classify_risk_minor_event_is_low() {
        let obs = Observation::MinorEvent {
            description: "张三与李四交谈".to_string(),
            involved_mentions: vec!["张三".to_string(), "李四".to_string()],
            evidence_span_ids: vec![],
            confidence: 0.7,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Low);
    }

    #[test]
    fn classify_risk_summary_is_low() {
        let obs = Observation::Summary {
            summary: "本章张三突破到筑基期".to_string(),
            key_points: vec!["突破".to_string()],
            has_important_changes: true,
        };
        assert_eq!(classify_risk(&obs), RiskLevel::Low);
    }

    #[test]
    fn risk_level_as_str() {
        assert_eq!(RiskLevel::Low.as_str(), "low");
        assert_eq!(RiskLevel::Medium.as_str(), "medium");
        assert_eq!(RiskLevel::High.as_str(), "high");
    }

    #[test]
    fn observation_subject_mention() {
        let obs = Observation::PropertyUpdate {
            subject_mention: "张三".to_string(),
            dimension_key: "realm".to_string(),
            value_text: Some("筑基".to_string()),
            value_json: None,
            evidence_span_ids: vec![],
            confidence: 0.8,
        };
        assert_eq!(obs.subject_mention(), Some("张三"));

        let event = Observation::MinorEvent {
            description: "战斗".to_string(),
            involved_mentions: vec![],
            evidence_span_ids: vec![],
            confidence: 0.5,
        };
        assert_eq!(event.subject_mention(), None);
    }

    #[test]
    fn observation_evidence_span_ids() {
        let obs = Observation::Alias {
            subject_mention: "张三".to_string(),
            alias: "小三".to_string(),
            alias_type: "nickname".to_string(),
            evidence_span_ids: vec!["s1".to_string(), "s2".to_string()],
            confidence: 0.8,
        };
        assert_eq!(obs.evidence_span_ids().len(), 2);

        let summary = Observation::Summary {
            summary: "test".to_string(),
            key_points: vec![],
            has_important_changes: false,
        };
        assert!(summary.evidence_span_ids().is_empty());
    }

    // --- parse_observations_from_json tests ---

    #[test]
    fn parse_valid_entity_introduction() {
        let json = r#"[{"type":"entity_introduction","subject_mention":"张三","entity_type":"character","aliases":["小张"],"short_summary":"主角","evidence_span_ids":["s1"],"confidence":0.9}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Observation::EntityIntroduction {
                subject_mention,
                aliases,
                ..
            } => {
                assert_eq!(subject_mention, "张三");
                assert_eq!(aliases, &vec!["小张".to_string()]);
            }
            _ => panic!("expected EntityIntroduction"),
        }
    }

    #[test]
    fn parse_valid_alias() {
        let json = r#"[{"type":"alias","subject_mention":"张三","alias":"张真人","alias_type":"title","evidence_span_ids":["s1"],"confidence":0.8}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Observation::Alias { alias, .. } => assert_eq!(alias, "张真人"),
            _ => panic!("expected Alias"),
        }
    }

    #[test]
    fn parse_valid_property_update() {
        let json = r#"[{"type":"property_update","subject_mention":"张三","dimension_key":"realm","value_text":"金丹","evidence_span_ids":["s1"],"confidence":0.85}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Observation::PropertyUpdate {
                dimension_key,
                value_text,
                ..
            } => {
                assert_eq!(dimension_key, "realm");
                assert_eq!(value_text.as_deref(), Some("金丹"));
            }
            _ => panic!("expected PropertyUpdate"),
        }
    }

    #[test]
    fn parse_valid_summary() {
        let json = r#"[{"type":"summary","summary":"本章介绍张三","key_points":["突破"],"has_important_changes":true}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Observation::Summary {
                summary,
                key_points,
                has_important_changes,
            } => {
                assert_eq!(summary, "本章介绍张三");
                assert_eq!(key_points, &vec!["突破".to_string()]);
                assert!(*has_important_changes);
            }
            _ => panic!("expected Summary"),
        }
    }

    #[test]
    fn parse_invalid_dimension_key_rejected() {
        let json = r#"[{"type":"property_update","subject_mention":"张三","dimension_key":"invalid_key","value_text":"test","evidence_span_ids":["s1"],"confidence":0.8}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids);
        assert!(result.is_err(), "invalid dimension_key should be rejected");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid dimension_key"));
    }

    #[test]
    fn parse_missing_evidence_span_ids_rejected() {
        let json = r#"[{"type":"alias","subject_mention":"张三","alias":"小张","alias_type":"nickname","evidence_span_ids":[],"confidence":0.8}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids);
        assert!(
            result.is_err(),
            "empty evidence_span_ids should be rejected"
        );
    }

    #[test]
    fn parse_invalid_evidence_span_id_rejected() {
        let json = r#"[{"type":"alias","subject_mention":"张三","alias":"小张","alias_type":"nickname","evidence_span_ids":["bad_id"],"confidence":0.8}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids);
        assert!(
            result.is_err(),
            "invalid evidence_span_id should be rejected"
        );
    }

    #[test]
    fn parse_confidence_out_of_range_rejected() {
        let json = r#"[{"type":"alias","subject_mention":"张三","alias":"小张","alias_type":"nickname","evidence_span_ids":["s1"],"confidence":1.5}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids);
        assert!(result.is_err(), "confidence > 1.0 should be rejected");
    }

    #[test]
    fn parse_markdown_fences_stripped() {
        let json = "```json\n[{\"type\":\"summary\",\"summary\":\"test\",\"key_points\":[],\"has_important_changes\":false}]\n```";
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_truncated_json_repair() {
        // Last object is complete but array is not closed
        let json =
            r#"[{"type":"summary","summary":"test","key_points":[],"has_important_changes":false}"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_unknown_type_rejected() {
        let json = r#"[{"type":"unknown_type","data":"test"}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids);
        assert!(result.is_err(), "unknown type should be rejected");
    }

    #[test]
    fn parse_mixed_observations() {
        let json = r#"[
            {"type":"entity_introduction","subject_mention":"张三","entity_type":"character","aliases":[],"short_summary":"主角","evidence_span_ids":["s1"],"confidence":0.9},
            {"type":"alias","subject_mention":"张三","alias":"小张","alias_type":"nickname","evidence_span_ids":["s1"],"confidence":0.8},
            {"type":"property_update","subject_mention":"张三","dimension_key":"realm","value_text":"金丹","evidence_span_ids":["s1"],"confidence":0.85},
            {"type":"summary","summary":"本章介绍张三","key_points":[],"has_important_changes":false}
        ]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn classify_risk_identity_variants_are_high() {
        let observations = vec![
            Observation::IdentityReveal {
                revealed_mention: "黑衣人".to_string(),
                canonical_mention: "张三".to_string(),
                reveal_type: "disguise".to_string(),
                reason_hint: Some("摘下面具".to_string()),
                evidence_span_ids: vec!["s1".to_string()],
                confidence: 0.95,
            },
            Observation::EntityMergeCandidate {
                entity_a_mention: "黑衣人".to_string(),
                entity_b_mention: "张三".to_string(),
                reason_hint: Some("明示同一人".to_string()),
                evidence_span_ids: vec!["s1".to_string()],
                confidence: 0.8,
            },
            Observation::EntitySplitCandidate {
                entity_a_mention: "张三".to_string(),
                entity_b_mention: "另一个张三".to_string(),
                reason_hint: Some("两人同时出现".to_string()),
                evidence_span_ids: vec!["s1".to_string()],
                confidence: 0.75,
            },
            Observation::NotSameIdentity {
                entity_a_mention: "此张三".to_string(),
                entity_b_mention: "彼张三".to_string(),
                reason_hint: Some("并非同一人".to_string()),
                evidence_span_ids: vec!["s1".to_string()],
                confidence: 0.9,
            },
        ];

        for obs in observations {
            assert_eq!(classify_risk(&obs), RiskLevel::High);
        }
    }

    #[test]
    fn parse_valid_identity_observations() {
        let json = r#"[
            {"type":"identity_reveal","revealed_mention":"黑衣人","canonical_mention":"张三","reveal_type":"disguise","reason_hint":"摘下面具","evidence_span_ids":["s1","s2"],"confidence":0.95},
            {"type":"entity_merge_candidate","entity_a_mention":"黑衣人","entity_b_mention":"张三","reason_hint":"明示同一人","evidence_span_ids":["s1"],"confidence":0.8},
            {"type":"entity_split_candidate","entity_a_mention":"此张三","entity_b_mention":"彼张三","reason_hint":"同时出现","evidence_span_ids":["s2"],"confidence":0.7},
            {"type":"not_same_identity","entity_a_mention":"此张三","entity_b_mention":"彼张三","reason_hint":"并非同一人","evidence_span_ids":["s1","s2"],"confidence":0.9}
        ]"#;
        let span_ids = vec!["s1".to_string(), "s2".to_string()];
        let result = parse_observations_from_json(json, &span_ids).unwrap();
        assert_eq!(result.len(), 4);

        match &result[0] {
            Observation::IdentityReveal {
                revealed_mention,
                canonical_mention,
                evidence_span_ids,
                ..
            } => {
                assert_eq!(revealed_mention, "黑衣人");
                assert_eq!(canonical_mention, "张三");
                assert_eq!(evidence_span_ids, &vec!["s1".to_string(), "s2".to_string()]);
            }
            _ => panic!("expected IdentityReveal"),
        }

        match &result[3] {
            Observation::NotSameIdentity {
                entity_a_mention,
                entity_b_mention,
                ..
            } => {
                assert_eq!(entity_a_mention, "此张三");
                assert_eq!(entity_b_mention, "彼张三");
            }
            _ => panic!("expected NotSameIdentity"),
        }
    }

    #[test]
    fn parse_valid_knowledge_assertion() {
        let json = r#"[
            {
              "type": "knowledge_assertion",
              "category": "power_system",
              "topic": "修炼境界",
              "assertion_text": "修炼境界分为炼气、筑基、金丹。",
              "confidence": 0.9,
              "importance_score": 0.8,
              "evidence_span_ids": ["s1"],
              "referenced_entity_mentions": [
                {"mention":"金丹","entity_type_hint":"realm","role":"realm","confidence":0.9}
              ],
              "status_hint": "fact",
              "reason_hint": "旁白说明境界体系"
            }
        ]"#;
        let parsed = parse_observations_from_json(json, &["s1".to_string()]).unwrap();
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            Observation::KnowledgeAssertion {
                category,
                topic,
                assertion_text,
                importance_score,
                referenced_entity_mentions,
                status_hint,
                ..
            } => {
                assert_eq!(category, "power_system");
                assert_eq!(topic, "修炼境界");
                assert_eq!(assertion_text, "修炼境界分为炼气、筑基、金丹。");
                assert_eq!(*importance_score, 0.8);
                assert_eq!(referenced_entity_mentions.len(), 1);
                assert_eq!(referenced_entity_mentions[0].mention, "金丹");
                assert_eq!(status_hint.as_deref(), Some("fact"));
            }
            _ => panic!("expected knowledge assertion"),
        }
    }

    #[test]
    fn parse_knowledge_assertion_skips_incomplete_entity_refs() {
        let json = r#"[
            {
              "type": "knowledge_assertion",
              "category": "world_rule",
              "topic": "奥术师考核",
              "assertion_text": "正式奥术师必须通过基础课程考核。",
              "confidence": 0.9,
              "importance_score": 0.8,
              "evidence_span_ids": ["s1"],
              "referenced_entity_mentions": [
                {},
                {"role":"related"},
                {"mention":"知识议会","entity_type_hint":"organization","role":"faction","confidence":0.9}
              ],
              "status_hint": "fact"
            }
        ]"#;
        let parsed = parse_observations_from_json(json, &["s1".to_string()]).unwrap();
        match &parsed[0] {
            Observation::KnowledgeAssertion {
                referenced_entity_mentions,
                ..
            } => {
                assert_eq!(referenced_entity_mentions.len(), 1);
                assert_eq!(referenced_entity_mentions[0].mention, "知识议会");
            }
            _ => panic!("expected knowledge assertion"),
        }
    }

    #[test]
    fn parse_knowledge_assertion_invalid_category_rejected() {
        let json = r#"[{"type":"knowledge_assertion","category":"temporary_event","topic":"x","assertion_text":"x","evidence_span_ids":["s1"],"confidence":0.8,"importance_score":0.7}]"#;
        assert!(parse_observations_from_json(json, &["s1".to_string()]).is_err());
    }

    #[test]
    fn parse_knowledge_assertion_missing_evidence_rejected() {
        let json = r#"[{"type":"knowledge_assertion","category":"history","topic":"x","assertion_text":"x","confidence":0.8,"importance_score":0.7}]"#;
        assert!(parse_observations_from_json(json, &["s1".to_string()]).is_err());
    }

    #[test]
    fn parse_knowledge_assertion_importance_out_of_range_rejected() {
        let json = r#"[{"type":"knowledge_assertion","category":"history","topic":"x","assertion_text":"x","evidence_span_ids":["s1"],"confidence":0.8,"importance_score":1.7}]"#;
        assert!(parse_observations_from_json(json, &["s1".to_string()]).is_err());
    }

    #[test]
    fn parse_knowledge_assertion_rejects_mistyped_property_shape() {
        let json = r#"[{"type":"knowledge_assertion","category":"power_system","topic":"张三境界","assertion_text":"张三突破金丹。","subject_mention":"张三","dimension_key":"realm","evidence_span_ids":["s1"],"confidence":0.8,"importance_score":0.7}]"#;
        assert!(parse_observations_from_json(json, &["s1".to_string()]).is_err());
    }

    #[test]
    fn parse_knowledge_assertion_rejects_mistyped_relationship_shape() {
        let json = r#"[{"type":"knowledge_assertion","category":"faction_structure","topic":"张三李四","assertion_text":"张三和李四结盟。","subject_mention":"张三","object_mention":"李四","relation_group":"alliance","evidence_span_ids":["s1"],"confidence":0.8,"importance_score":0.7}]"#;
        assert!(parse_observations_from_json(json, &["s1".to_string()]).is_err());
    }

    #[test]
    fn parse_knowledge_assertion_rejects_mistyped_identity_shape() {
        let json = r#"[{"type":"knowledge_assertion","category":"secret","topic":"黑衣人身份","assertion_text":"黑衣人其实是张三。","revealed_mention":"黑衣人","canonical_mention":"张三","reveal_type":"disguise","evidence_span_ids":["s1"],"confidence":0.8,"importance_score":0.7}]"#;
        assert!(parse_observations_from_json(json, &["s1".to_string()]).is_err());
    }

    #[test]
    fn parse_knowledge_assertion_rejects_mistyped_map_edge_shape() {
        let json = r#"[{"type":"knowledge_assertion","category":"geography","topic":"青云门方位","assertion_text":"青云门在东域以北。","from_place_mention":"青云门","to_place_mention":"东域","edge_type":"north_of","evidence_span_ids":["s1"],"confidence":0.8,"importance_score":0.7}]"#;
        assert!(parse_observations_from_json(json, &["s1".to_string()]).is_err());
    }
}
