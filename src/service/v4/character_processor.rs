use crate::service::v4::character_decision::{
    self, CharacterDecision, CharacterNoWriteStatus, LedgerCharacterClaim,
};
use crate::service::v4::claim_lifecycle::{
    accept_claim, mark_uncertain_claim, quarantine_claim, reject_claim,
};
use crate::service::v4::reducer;
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use sqlx::SqlitePool;

#[derive(Debug, Default)]
pub struct CharacterSegmentProcessResult {
    pub claims_accepted: usize,
    pub claims_rejected: usize,
    pub claims_quarantined: usize,
    pub claims_uncertain: usize,
}

pub(crate) async fn process_character_claims_for_segment(
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    character_claims: &[ClaimRecord],
) -> anyhow::Result<CharacterSegmentProcessResult> {
    let mut result = CharacterSegmentProcessResult::default();

    for claim in character_claims {
        let claim = re_resolve_character_subject(pool, claim).await?;
        let decision = match LedgerCharacterClaim::from_claim(&claim) {
            Ok(ledger_claim) => character_decision::materialize_character_decision(ledger_claim)?,
            Err(err) => {
                tracing::warn!(
                    "Character claim {} could not become a ledger claim: {}",
                    claim.id,
                    err
                );
                reject_claim(claim_repo, &claim.id, &mut result.claims_rejected).await?;
                continue;
            }
        };

        apply_character_decision_lifecycle(pool, claim_repo, decision, &mut result).await?;
    }

    Ok(result)
}

async fn re_resolve_character_subject(
    pool: &SqlitePool,
    claim: &ClaimRecord,
) -> anyhow::Result<ClaimRecord> {
    if claim.subject_entity_id.is_some() {
        return Ok(claim.clone());
    }
    let Some(subject_mention) = claim.subject_mention.as_deref() else {
        return Ok(claim.clone());
    };

    let entity_repo = EntityRepo::new(pool.clone());
    let Some(entity) = entity_repo
        .find_entity_by_alias(&claim.book_id, subject_mention)
        .await?
    else {
        return Ok(claim.clone());
    };

    let mut resolved = claim.clone();
    resolved.subject_entity_id = Some(entity.id);
    Ok(resolved)
}

async fn apply_character_decision_lifecycle(
    pool: &SqlitePool,
    claim_repo: &ClaimRepo,
    decision: CharacterDecision,
    result: &mut CharacterSegmentProcessResult,
) -> anyhow::Result<()> {
    match decision {
        CharacterDecision::Write(command) => {
            let claim_id = command_provenance_claim_id(&command);
            match reducer::apply_character_write(command, pool).await {
                Ok(_) => {
                    accept_claim(claim_repo, &claim_id, &mut result.claims_accepted).await?;
                }
                Err(err) => {
                    tracing::warn!(
                        "Character command for claim {} failed: {}. Marking uncertain.",
                        claim_id,
                        err
                    );
                    mark_uncertain_claim(claim_repo, &claim_id, &mut result.claims_uncertain)
                        .await?;
                }
            }
        }
        CharacterDecision::NoWrite(no_write) => match no_write.status {
            CharacterNoWriteStatus::Rejected => {
                reject_claim(claim_repo, &no_write.claim_id, &mut result.claims_rejected).await?;
            }
            CharacterNoWriteStatus::Uncertain => {
                mark_uncertain_claim(claim_repo, &no_write.claim_id, &mut result.claims_uncertain)
                    .await?;
            }
        },
        CharacterDecision::Invalid(rejection) => {
            tracing::warn!(
                "Character decision rejected claim {}: {}",
                rejection.claim_id,
                rejection.reason
            );
            reject_claim(claim_repo, &rejection.claim_id, &mut result.claims_rejected).await?;
        }
        CharacterDecision::Quarantine(quarantine) => {
            tracing::warn!(
                "Character decision quarantined claim {}: {}",
                quarantine.claim_id,
                quarantine.reason
            );
            quarantine_claim(
                claim_repo,
                &quarantine.claim_id,
                &mut result.claims_quarantined,
            )
            .await?;
        }
    }

    Ok(())
}

fn command_provenance_claim_id(command: &reducer::CharacterWriteCommand) -> String {
    match command {
        reducer::CharacterWriteCommand::IntroduceEntity { provenance, .. }
        | reducer::CharacterWriteCommand::AddAlias { provenance, .. }
        | reducer::CharacterWriteCommand::UpdateProperty { provenance, .. } => {
            provenance.claim_id.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::test_support::setup_v4_processor_test;
    use crate::storage::db::v4::claim_repo::ClaimRepo;
    use crate::storage::db::v4::entity_repo::EntityRepo;
    use crate::storage::db::v4::property_repo::PropertyRepo;
    use sqlx::SqlitePool;

    async fn setup() -> (
        SqlitePool,
        ClaimRepo,
        EntityRepo,
        PropertyRepo,
        String,
        String,
    ) {
        let ctx = setup_v4_processor_test("character", 7, "text", "test text").await;

        (
            ctx.pool.clone(),
            ClaimRepo::new(ctx.pool.clone()),
            EntityRepo::new(ctx.pool.clone()),
            PropertyRepo::new(ctx.pool.clone()),
            ctx.span_id,
            ctx.run_id,
        )
    }

    #[tokio::test]
    async fn character_segment_processor_applies_typed_entity_command_and_accepts_claim() {
        let (pool, claim_repo, entity_repo, _property_repo, span_id, run_id) = setup().await;
        let claim = claim_repo
            .create_claim(
                "b1",
                7,
                "entity_introduction",
                Some("柳清歌"),
                None,
                None,
                None,
                "ignored by processor",
                Some("百战峰峰主"),
                Some(
                    r#"{
                        "entity_type":"character",
                        "short_summary":"百战峰峰主",
                        "aliases":["柳巨巨"]
                    }"#,
                ),
                &span_id,
                &run_id,
                0.88,
                "low",
            )
            .await
            .unwrap();

        let result = process_character_claims_for_segment(&pool, &claim_repo, &[claim.clone()])
            .await
            .unwrap();

        assert_eq!(result.claims_accepted, 1);
        let stored_claim = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored_claim.status, "accepted");
        let entity = entity_repo
            .find_entity_by_alias("b1", "柳清歌")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entity.display_name, "柳清歌");
        assert_eq!(entity.short_summary.as_deref(), Some("百战峰峰主"));
        let aliases = entity_repo
            .list_aliases_by_entity(&entity.id)
            .await
            .unwrap();
        assert!(aliases.iter().any(|alias| alias.alias == "柳巨巨"));
    }

    #[tokio::test]
    async fn character_segment_processor_applies_typed_alias_and_property_commands() {
        let (pool, claim_repo, entity_repo, property_repo, span_id, run_id) = setup().await;
        let entity = entity_repo
            .create_entity("b1", "character", "沈清秋", "沈清秋", None, 0.6, 1)
            .await
            .unwrap();

        let alias_claim = claim_repo
            .create_claim(
                "b1",
                7,
                "alias",
                Some("沈清秋"),
                Some("师尊"),
                Some(&entity.id),
                None,
                "misleading predicate",
                None,
                None,
                &span_id,
                &run_id,
                0.8,
                "low",
            )
            .await
            .unwrap();
        let property_claim = claim_repo
            .create_claim(
                "b1",
                7,
                "property_update",
                Some("沈清秋"),
                None,
                Some(&entity.id),
                None,
                "realm = 元婴",
                Some("元婴"),
                Some(r#"{"stage":"元婴"}"#),
                &span_id,
                &run_id,
                0.82,
                "low",
            )
            .await
            .unwrap();

        let result = process_character_claims_for_segment(
            &pool,
            &claim_repo,
            &[alias_claim.clone(), property_claim.clone()],
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, 2);
        assert_eq!(
            claim_repo
                .get_claim(&alias_claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "accepted"
        );
        assert_eq!(
            claim_repo
                .get_claim(&property_claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "accepted"
        );
        let aliases = entity_repo
            .list_aliases_by_entity(&entity.id)
            .await
            .unwrap();
        assert!(aliases.iter().any(|alias| alias.alias == "师尊"));
        let property = property_repo
            .get_current_property("b1", &entity.id, "realm")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(property.value_text.as_deref(), Some("元婴"));
        assert_eq!(property.value_json.as_deref(), Some(r#"{"stage":"元婴"}"#));
    }

    #[tokio::test]
    async fn character_segment_processor_re_resolves_alias_after_same_segment_entity_introduction()
    {
        let (pool, claim_repo, entity_repo, _property_repo, span_id, run_id) = setup().await;
        let entity_claim = claim_repo
            .create_claim(
                "b1",
                7,
                "entity_introduction",
                Some("张三"),
                None,
                None,
                None,
                "ignored by processor",
                Some("主角"),
                Some(r#"{"entity_type":"character","aliases":["张真人"]}"#),
                &span_id,
                &run_id,
                0.95,
                "low",
            )
            .await
            .unwrap();
        let alias_claim = claim_repo
            .create_claim(
                "b1",
                7,
                "alias",
                Some("张三"),
                Some("小张"),
                None,
                None,
                "also known as 小张 (nickname)",
                None,
                Some(r#"{"alias_type":"nickname"}"#),
                &span_id,
                &run_id,
                0.85,
                "low",
            )
            .await
            .unwrap();

        let result = process_character_claims_for_segment(
            &pool,
            &claim_repo,
            &[entity_claim.clone(), alias_claim.clone()],
        )
        .await
        .unwrap();

        assert_eq!(result.claims_accepted, 2);
        let entity = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap()
            .unwrap();
        let aliases = entity_repo
            .list_aliases_by_entity(&entity.id)
            .await
            .unwrap();
        assert!(aliases.iter().any(|alias| alias.alias == "小张"));
    }
}
