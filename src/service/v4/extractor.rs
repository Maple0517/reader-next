/// Observation types extracted from text by the AI extractor.

/// Trait for extracting observations from chapter context.
///
/// Implementations:
/// - `MockExtractor`: returns pre-configured observations for testing
/// - `AiExtractor`: placeholder for real AI extraction (TODO)
#[axum::async_trait]
pub trait Extractor: Send + Sync {
    async fn extract(&self, context: &str, span_ids: &[String]) -> anyhow::Result<Vec<Observation>>;
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
    async fn extract(&self, _context: &str, span_ids: &[String]) -> anyhow::Result<Vec<Observation>> {
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
    async fn extract(&self, _context: &str, _span_ids: &[String]) -> anyhow::Result<Vec<Observation>> {
        // TODO: Integrate actual AI extraction when AI service is available.
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq)]
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
    "identity", "life_status", "affiliation", "rank", "occupation", "realm",
    "ability", "equipment", "location", "mental_state", "goal", "injury",
    "appearance", "background",
];

/// Real AI extractor that calls the configured LLM provider.
pub struct RealAiExtractor {
    ai_model_service: std::sync::Arc<crate::service::ai_model_service::AiModelService>,
}

impl RealAiExtractor {
    pub fn new(ai_model_service: std::sync::Arc<crate::service::ai_model_service::AiModelService>) -> Self {
        Self { ai_model_service }
    }
}

#[axum::async_trait]
impl Extractor for RealAiExtractor {
    async fn extract(&self, context: &str, span_ids: &[String]) -> anyhow::Result<Vec<Observation>> {
        use crate::model::ai_model::AiModelKind;
        use crate::model::ai_proxy::{build_ai_proxy_url, ai_proxy_timeout};
        use crate::service::ai_book_generation_service::extract_model_content;

        // 1. Load AI config
        let config = self.ai_model_service.get().await.map_err(|e| anyhow::anyhow!("{}", e))?;
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
             - summary: 章节摘要（summary, key_points, has_important_changes）\n\n\
             dimension_key 必须是以下之一：{}\n\n\
             evidence_span_ids 必须使用以下可用 span IDs：{}\n\n\
             输出严格 JSON 数组，每个元素包含 \"type\" 字段。",
            context,
            VALID_DIMENSION_KEYS.join(", "),
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
        let client = reqwest::Client::builder().timeout(ai_proxy_timeout()).build()?;
        let use_gemini = crate::service::ai_book_generation_service::is_gemini_generate_content_path(path)
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
    let is_gemini = crate::service::ai_book_generation_service::is_gemini_generate_content_path(path);
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
        "model": model, "temperature": 0.2, "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": V4_SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    })
}

/// Parse observations from model JSON output.
/// Handles markdown fences, validates evidence_span_ids, dimension_key, confidence.
pub fn parse_observations_from_json(raw: &str, valid_span_ids: &[String]) -> anyhow::Result<Vec<Observation>> {
    let cleaned = strip_markdown_fences(raw);
    let arr: Vec<serde_json::Value> = serde_json::from_str(&cleaned)
        .or_else(|_| {
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
        match obs_type {
            "entity_introduction" => {
                let subject_mention = required_str(item, "subject_mention")?;
                let entity_type = optional_str(item, "entity_type").unwrap_or("character");
                let aliases = optional_str_array(item, "aliases");
                let short_summary = optional_str(item, "short_summary").unwrap_or_default().to_string();
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
            "summary" => {
                let summary = required_str(item, "summary")?;
                let key_points = optional_str_array(item, "key_points");
                let has_important_changes = item.get("has_important_changes").and_then(|v| v.as_bool()).unwrap_or(false);
                observations.push(Observation::Summary {
                    summary,
                    key_points,
                    has_important_changes,
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
        let after_open = trimmed.find('\n').map(|i| &trimmed[i + 1..]).unwrap_or(trimmed);
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
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

fn validate_evidence_spans(item: &serde_json::Value, valid: &std::collections::HashSet<&String>) -> anyhow::Result<Vec<String>> {
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
    let c = item.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5);
    if c < 0.0 || c > 1.0 {
        anyhow::bail!("confidence out of range: {}", c);
    }
    Ok(c)
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
            Observation::EntityIntroduction { subject_mention, aliases, .. } => {
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
            Observation::PropertyUpdate { dimension_key, value_text, .. } => {
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
            Observation::Summary { summary, key_points, has_important_changes } => {
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
        assert!(result.unwrap_err().to_string().contains("Invalid dimension_key"));
    }

    #[test]
    fn parse_missing_evidence_span_ids_rejected() {
        let json = r#"[{"type":"alias","subject_mention":"张三","alias":"小张","alias_type":"nickname","evidence_span_ids":[],"confidence":0.8}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids);
        assert!(result.is_err(), "empty evidence_span_ids should be rejected");
    }

    #[test]
    fn parse_invalid_evidence_span_id_rejected() {
        let json = r#"[{"type":"alias","subject_mention":"张三","alias":"小张","alias_type":"nickname","evidence_span_ids":["bad_id"],"confidence":0.8}]"#;
        let span_ids = vec!["s1".to_string()];
        let result = parse_observations_from_json(json, &span_ids);
        assert!(result.is_err(), "invalid evidence_span_id should be rejected");
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
        let json = r#"[{"type":"summary","summary":"test","key_points":[],"has_important_changes":false}"#;
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
}
