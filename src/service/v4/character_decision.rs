use crate::service::v4::reducer::{CharacterWriteCommand, CharacterWriteProvenance};
use crate::storage::db::v4::claim_repo::ClaimRecord;

#[derive(Debug, Clone)]
pub struct LedgerCharacterClaim {
    pub claim_id: String,
    pub book_id: String,
    pub chapter_index: i64,
    pub claim_type: String,
    pub subject_mention: Option<String>,
    pub object_mention: Option<String>,
    pub subject_entity_id: Option<String>,
    pub predicate: String,
    pub value_text: Option<String>,
    pub value_json: Option<serde_json::Value>,
    pub primary_source_span_id: String,
    pub confidence: f64,
    pub status: String,
    pub risk_level: String,
}

#[derive(Debug, Clone)]
pub enum CharacterDecision {
    Invalid(CharacterDecisionRejection),
    NoWrite(CharacterNoWrite),
    Quarantine(CharacterQuarantine),
    Write(CharacterWriteCommand),
}

#[derive(Debug, Clone)]
pub struct CharacterDecisionRejection {
    pub claim_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct CharacterNoWrite {
    pub claim_id: String,
    pub status: CharacterNoWriteStatus,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacterNoWriteStatus {
    Rejected,
    Uncertain,
}

#[derive(Debug, Clone)]
pub struct CharacterQuarantine {
    pub claim_id: String,
    pub reason: String,
}

impl LedgerCharacterClaim {
    pub fn from_claim(claim: &ClaimRecord) -> anyhow::Result<Self> {
        if !matches!(
            claim.claim_type.as_str(),
            "entity_introduction" | "alias" | "property_update"
        ) {
            anyhow::bail!(
                "claim {} is not a character/profile claim: {}",
                claim.id,
                claim.claim_type
            );
        }

        let value_json = match claim.value_json.as_deref() {
            Some(raw) => Some(serde_json::from_str(raw).map_err(|err| {
                anyhow::anyhow!(
                    "character claim {} has invalid value_json: {}",
                    claim.id,
                    err
                )
            })?),
            None => None,
        };

        Ok(Self {
            claim_id: claim.id.clone(),
            book_id: claim.book_id.clone(),
            chapter_index: claim.chapter_index,
            claim_type: claim.claim_type.clone(),
            subject_mention: claim.subject_mention.clone(),
            object_mention: claim.object_mention.clone(),
            subject_entity_id: claim.subject_entity_id.clone(),
            predicate: claim.predicate.clone(),
            value_text: claim.value_text.clone(),
            value_json,
            primary_source_span_id: claim.primary_source_span_id.clone(),
            confidence: claim.confidence,
            status: claim.status.clone(),
            risk_level: claim.risk_level.clone(),
        })
    }
}

pub fn materialize_character_decision(
    claim: LedgerCharacterClaim,
) -> anyhow::Result<CharacterDecision> {
    if claim.status != "proposed" {
        return Ok(CharacterDecision::NoWrite(CharacterNoWrite {
            claim_id: claim.claim_id,
            status: CharacterNoWriteStatus::Uncertain,
            reason_code: format!("ledger_status_{}", claim.status),
        }));
    }

    if claim.risk_level == "high" {
        if is_critical_high_risk_character_claim(&claim) {
            return Ok(CharacterDecision::Quarantine(CharacterQuarantine {
                claim_id: claim.claim_id,
                reason: "high risk critical character claim".to_string(),
            }));
        }
        return Ok(CharacterDecision::NoWrite(CharacterNoWrite {
            claim_id: claim.claim_id,
            status: CharacterNoWriteStatus::Uncertain,
            reason_code: "high_risk_character_claim".to_string(),
        }));
    }

    let claim_id = claim.claim_id.clone();
    match character_write_command_from_ledger(claim) {
        Ok(command) => Ok(CharacterDecision::Write(command)),
        Err(err) => Ok(classify_character_write_failure(claim_id, err)),
    }
}

fn is_critical_high_risk_character_claim(claim: &LedgerCharacterClaim) -> bool {
    if claim.claim_type != "property_update" {
        return false;
    }
    let value = claim
        .value_text
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    (claim.predicate.starts_with("life_status")
        && (value.contains("死") || value.contains("亡") || value.contains("复活")))
        || (claim.predicate.starts_with("identity") && value.contains("真实身份"))
}

fn classify_character_write_failure(claim_id: String, err: anyhow::Error) -> CharacterDecision {
    let reason = err.to_string();
    if reason.contains("missing resolved entity id") {
        return CharacterDecision::Quarantine(CharacterQuarantine { claim_id, reason });
    }

    CharacterDecision::Invalid(CharacterDecisionRejection { claim_id, reason })
}

fn character_write_command_from_ledger(
    claim: LedgerCharacterClaim,
) -> anyhow::Result<CharacterWriteCommand> {
    match claim.claim_type.as_str() {
        "entity_introduction" => entity_introduction_command(claim),
        "alias" => alias_command(claim),
        "property_update" => property_update_command(claim),
        other => anyhow::bail!("unsupported character claim type {}", other),
    }
}

fn entity_introduction_command(
    claim: LedgerCharacterClaim,
) -> anyhow::Result<CharacterWriteCommand> {
    let canonical_name = claim.subject_mention.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "character claim {} missing subject mention for entity introduction",
            claim.claim_id
        )
    })?;
    let payload = claim.value_json.as_ref();
    let entity_type =
        string_field(payload, "entity_type").unwrap_or_else(|| "character".to_string());
    let short_summary = string_field(payload, "short_summary").or(claim.value_text.clone());
    let aliases = string_array_field(payload, "aliases");
    let provenance = provenance(&claim);

    Ok(CharacterWriteCommand::IntroduceEntity {
        book_id: claim.book_id,
        entity_type,
        canonical_name: canonical_name.clone(),
        display_name: canonical_name,
        short_summary,
        aliases,
        chapter_index: claim.chapter_index,
        confidence: claim.confidence,
        provenance,
    })
}

fn alias_command(claim: LedgerCharacterClaim) -> anyhow::Result<CharacterWriteCommand> {
    let entity_id = claim.subject_entity_id.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "character claim {} missing resolved entity id for alias",
            claim.claim_id
        )
    })?;
    let alias = claim.object_mention.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "character claim {} missing alias object mention",
            claim.claim_id
        )
    })?;
    let alias_type = string_field(claim.value_json.as_ref(), "alias_type")
        .unwrap_or_else(|| "ai_extracted".to_string());
    let provenance = provenance(&claim);

    Ok(CharacterWriteCommand::AddAlias {
        book_id: claim.book_id,
        entity_id,
        alias,
        alias_type,
        chapter_index: claim.chapter_index,
        confidence: claim.confidence,
        provenance,
    })
}

fn property_update_command(claim: LedgerCharacterClaim) -> anyhow::Result<CharacterWriteCommand> {
    let entity_id = claim.subject_entity_id.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "character claim {} missing resolved entity id for property update",
            claim.claim_id
        )
    })?;
    let dimension_key = extract_dimension_from_predicate(&claim.predicate).ok_or_else(|| {
        anyhow::anyhow!(
            "character claim {} missing property dimension in predicate",
            claim.claim_id
        )
    })?;
    let provenance = provenance(&claim);

    Ok(CharacterWriteCommand::UpdateProperty {
        book_id: claim.book_id,
        entity_id,
        dimension_key,
        value_text: claim.value_text,
        value_json: claim.value_json,
        chapter_index: claim.chapter_index,
        confidence: claim.confidence,
        provenance,
    })
}

fn provenance(claim: &LedgerCharacterClaim) -> CharacterWriteProvenance {
    CharacterWriteProvenance {
        claim_id: claim.claim_id.clone(),
        evidence_span_ids: vec![claim.primary_source_span_id.clone()],
    }
}

fn string_field(value: Option<&serde_json::Value>, key: &str) -> Option<String> {
    value?
        .get(key)
        .and_then(|field| field.as_str())
        .map(str::to_string)
}

fn string_array_field(value: Option<&serde_json::Value>, key: &str) -> Vec<String> {
    value
        .and_then(|payload| payload.get(key))
        .and_then(|field| field.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_dimension_from_predicate(predicate: &str) -> Option<String> {
    predicate
        .split('=')
        .next()
        .map(|value| value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::v4::claim_repo::ClaimRecord;

    fn character_claim(
        claim_type: &str,
        subject_mention: Option<&str>,
        object_mention: Option<&str>,
        subject_entity_id: Option<&str>,
        predicate: &str,
        value_text: Option<&str>,
        value_json: Option<&str>,
    ) -> ClaimRecord {
        ClaimRecord {
            id: "claim-1".to_string(),
            book_id: "b1".to_string(),
            chapter_index: 7,
            claim_type: claim_type.to_string(),
            subject_mention: subject_mention.map(str::to_string),
            object_mention: object_mention.map(str::to_string),
            subject_entity_id: subject_entity_id.map(str::to_string),
            object_entity_id: None,
            predicate: predicate.to_string(),
            value_json: value_json.map(str::to_string),
            value_text: value_text.map(str::to_string),
            primary_source_span_id: "span-1".to_string(),
            ai_run_id: "run-1".to_string(),
            confidence: 0.82,
            status: "proposed".to_string(),
            risk_level: "low".to_string(),
            supersedes_claim_id: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[test]
    fn character_materializer_turns_entity_ledger_claim_into_command() {
        let claim = character_claim(
            "entity_introduction",
            Some("沈清秋"),
            None,
            None,
            "ignored by command",
            Some("苍穹山清静峰峰主"),
            Some(
                r#"{
                    "entity_type":"character",
                    "short_summary":"苍穹山清静峰峰主",
                    "aliases":["沈垣","师尊"]
                }"#,
            ),
        );
        let original_value_json = claim.value_json.clone();
        let ledger_claim = LedgerCharacterClaim::from_claim(&claim).unwrap();

        let decision = materialize_character_decision(ledger_claim).unwrap();

        match decision {
            CharacterDecision::Write(CharacterWriteCommand::IntroduceEntity {
                book_id,
                entity_type,
                canonical_name,
                display_name,
                short_summary,
                aliases,
                chapter_index,
                confidence,
                provenance,
            }) => {
                assert_eq!(book_id, "b1");
                assert_eq!(entity_type, "character");
                assert_eq!(canonical_name, "沈清秋");
                assert_eq!(display_name, "沈清秋");
                assert_eq!(short_summary.as_deref(), Some("苍穹山清静峰峰主"));
                assert_eq!(aliases, vec!["沈垣".to_string(), "师尊".to_string()]);
                assert_eq!(chapter_index, 7);
                assert_eq!(confidence, 0.82);
                assert_eq!(provenance.claim_id, "claim-1");
                assert_eq!(provenance.evidence_span_ids, vec!["span-1".to_string()]);
            }
            other => panic!("expected character introduce command, got {other:?}"),
        }
        assert_eq!(claim.value_json, original_value_json);
    }

    #[test]
    fn character_materializer_turns_alias_ledger_claim_into_command() {
        let claim = character_claim(
            "alias",
            Some("沈清秋"),
            Some("师尊"),
            Some("char-1"),
            "misleading predicate",
            None,
            None,
        );
        let ledger_claim = LedgerCharacterClaim::from_claim(&claim).unwrap();

        let decision = materialize_character_decision(ledger_claim).unwrap();

        match decision {
            CharacterDecision::Write(CharacterWriteCommand::AddAlias {
                book_id,
                entity_id,
                alias,
                alias_type,
                chapter_index,
                confidence,
                provenance,
            }) => {
                assert_eq!(book_id, "b1");
                assert_eq!(entity_id, "char-1");
                assert_eq!(alias, "师尊");
                assert_eq!(alias_type, "ai_extracted");
                assert_eq!(chapter_index, 7);
                assert_eq!(confidence, 0.82);
                assert_eq!(provenance.claim_id, "claim-1");
            }
            other => panic!("expected character alias command, got {other:?}"),
        }
    }

    #[test]
    fn character_materializer_turns_property_ledger_claim_into_command() {
        let claim = character_claim(
            "property_update",
            Some("沈清秋"),
            None,
            Some("char-1"),
            "realm = 元婴",
            Some("元婴"),
            Some(r#"{"stage":"元婴","rank":4}"#),
        );
        let ledger_claim = LedgerCharacterClaim::from_claim(&claim).unwrap();

        let decision = materialize_character_decision(ledger_claim).unwrap();

        match decision {
            CharacterDecision::Write(CharacterWriteCommand::UpdateProperty {
                book_id,
                entity_id,
                dimension_key,
                value_text,
                value_json,
                chapter_index,
                confidence,
                provenance,
            }) => {
                assert_eq!(book_id, "b1");
                assert_eq!(entity_id, "char-1");
                assert_eq!(dimension_key, "realm");
                assert_eq!(value_text.as_deref(), Some("元婴"));
                assert_eq!(value_json.unwrap()["stage"], "元婴");
                assert_eq!(chapter_index, 7);
                assert_eq!(confidence, 0.82);
                assert_eq!(provenance.claim_id, "claim-1");
            }
            other => panic!("expected character property command, got {other:?}"),
        }
    }
}
