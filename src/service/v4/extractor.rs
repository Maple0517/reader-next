/// Observation types extracted from text by the AI extractor.

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
}
