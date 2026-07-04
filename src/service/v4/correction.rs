use crate::storage::db::v4::claim_repo::ClaimRepo;
use crate::storage::db::v4::quality_repo::{
    NewCorrectionEvent, NewUserCorrection, QualityRepo, UserCorrectionRecord,
};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct CorrectionCommand {
    pub book_id: String,
    pub target_type: String,
    pub target_id: String,
    pub correction_type: String,
    pub correction_json: String,
    pub source: String,
    pub source_claim_id: Option<String>,
    pub source_span_id: Option<String>,
    pub created_by: String,
}

pub struct CorrectionValidationService {
    pool: SqlitePool,
    quality_repo: QualityRepo,
    claim_repo: ClaimRepo,
}

impl CorrectionValidationService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            quality_repo: QualityRepo::new(pool.clone()),
            claim_repo: ClaimRepo::new(pool.clone()),
            pool,
        }
    }

    pub async fn validate(&self, command: &CorrectionCommand) -> anyhow::Result<()> {
        validate_target_type(&command.target_type)?;
        validate_correction_type(&command.correction_type)?;
        validate_source(&command.source)?;
        validate_compatibility(&command.target_type, &command.correction_type)?;
        validate_manual_override_rule(command)?;
        self.validate_target_exists(command).await?;
        self.validate_source_refs_exist(command).await?;
        Ok(())
    }

    pub async fn validate_and_create(
        &self,
        command: CorrectionCommand,
    ) -> anyhow::Result<UserCorrectionRecord> {
        self.validate(&command).await?;
        let correction = self
            .quality_repo
            .create_user_correction(NewUserCorrection {
                id: None,
                book_id: &command.book_id,
                target_type: &command.target_type,
                target_id: &command.target_id,
                correction_type: &command.correction_type,
                correction_json: &command.correction_json,
                source: &command.source,
                source_claim_id: command.source_claim_id.as_deref(),
                source_span_id: command.source_span_id.as_deref(),
                created_by: &command.created_by,
            })
            .await?;
        self.quality_repo
            .append_correction_event(NewCorrectionEvent {
                id: None,
                book_id: &command.book_id,
                correction_id: &correction.id,
                event_type: "proposed",
                before_json: None,
                after_json: Some(&command.correction_json),
                actor: &command.created_by,
                reason: Some("validated correction command"),
            })
            .await?;
        Ok(correction)
    }

    async fn validate_target_exists(&self, command: &CorrectionCommand) -> anyhow::Result<()> {
        match command.target_type.as_str() {
            "claim" => {
                let claim = self.claim_repo.get_claim(&command.target_id).await?;
                ensure_exists(claim.is_some(), "target claim not found")
            }
            "entity" | "place" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM entities WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target entity not found",
                )
                .await
            }
            "entity_alias" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM entity_aliases WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target alias not found",
                )
                .await
            }
            "entity_property" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM entity_properties WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target property not found",
                )
                .await
            }
            "relationship" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM relationships WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target relationship not found",
                )
                .await
            }
            "relationship_event" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM relationship_events WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target relationship event not found",
                )
                .await
            }
            "identity_link" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM entity_identity_links WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target identity link not found",
                )
                .await
            }
            "knowledge_card" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM knowledge_cards WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target knowledge card not found",
                )
                .await
            }
            "knowledge_assertion" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM knowledge_assertions WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target knowledge assertion not found",
                )
                .await
            }
            "place_edge" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM place_edges WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target place edge not found",
                )
                .await
            }
            "entity_link" => {
                self.ensure_row_exists(
                    "SELECT COUNT(*) FROM entity_links WHERE book_id = ? AND id = ?",
                    &command.book_id,
                    &command.target_id,
                    "target entity link not found",
                )
                .await
            }
            "projection_cache" => Ok(()),
            _ => unreachable!("target type is validated before target lookup"),
        }
    }

    async fn validate_source_refs_exist(&self, command: &CorrectionCommand) -> anyhow::Result<()> {
        if let Some(source_claim_id) = &command.source_claim_id {
            let claim = self.claim_repo.get_claim(source_claim_id).await?;
            ensure_exists(claim.is_some(), "source claim not found")?;
        }
        if let Some(source_span_id) = &command.source_span_id {
            let span = self.claim_repo.get_span(source_span_id).await?;
            ensure_exists(span.is_some(), "source span not found")?;
        }
        Ok(())
    }

    async fn ensure_row_exists(
        &self,
        sql: &str,
        book_id: &str,
        target_id: &str,
        message: &str,
    ) -> anyhow::Result<()> {
        let count: (i64,) = sqlx::query_as(sql)
            .bind(book_id)
            .bind(target_id)
            .fetch_one(&self.pool)
            .await?;
        ensure_exists(count.0 > 0, message)
    }
}

fn validate_target_type(target_type: &str) -> anyhow::Result<()> {
    ensure_allowed(
        target_type,
        &[
            "claim",
            "entity",
            "entity_alias",
            "entity_property",
            "relationship",
            "relationship_event",
            "identity_link",
            "knowledge_card",
            "knowledge_assertion",
            "place",
            "place_edge",
            "entity_link",
            "projection_cache",
        ],
        "invalid correction target_type",
    )
}

fn validate_correction_type(correction_type: &str) -> anyhow::Result<()> {
    ensure_allowed(
        correction_type,
        &[
            "reject_claim",
            "accept_quarantined_claim",
            "retry_claim",
            "reclassify_claim",
            "merge_entities",
            "mark_not_same_entity",
            "split_required",
            "create_entity_link",
            "correct_property",
            "deactivate_relationship",
            "correct_relationship_label",
            "mark_knowledge_assertion_false",
            "revise_knowledge_assertion",
            "deactivate_place_edge",
            "mark_place_edge_conflict",
            "correct_place_parent",
            "rebuild_projection",
            "reprocess_scope",
        ],
        "invalid correction_type",
    )
}

fn validate_source(source: &str) -> anyhow::Result<()> {
    ensure_allowed(
        source,
        &["user", "audit", "system", "test"],
        "invalid correction source",
    )
}

fn validate_compatibility(target_type: &str, correction_type: &str) -> anyhow::Result<()> {
    let allowed = match target_type {
        "claim" => &[
            "reject_claim",
            "accept_quarantined_claim",
            "retry_claim",
            "reclassify_claim",
            "reprocess_scope",
        ][..],
        "entity" => &[
            "merge_entities",
            "mark_not_same_entity",
            "split_required",
            "create_entity_link",
            "reprocess_scope",
        ][..],
        "entity_alias" => &["reprocess_scope"][..],
        "entity_property" => &["correct_property", "reprocess_scope"][..],
        "relationship" => &[
            "deactivate_relationship",
            "correct_relationship_label",
            "reprocess_scope",
        ][..],
        "relationship_event" => &["deactivate_relationship", "reprocess_scope"][..],
        "identity_link" => &["mark_not_same_entity", "split_required", "reprocess_scope"][..],
        "knowledge_card" => &["revise_knowledge_assertion", "reprocess_scope"][..],
        "knowledge_assertion" => &[
            "mark_knowledge_assertion_false",
            "revise_knowledge_assertion",
            "reprocess_scope",
        ][..],
        "place" => &[
            "correct_place_parent",
            "create_entity_link",
            "reprocess_scope",
        ][..],
        "place_edge" => &[
            "deactivate_place_edge",
            "mark_place_edge_conflict",
            "correct_place_parent",
            "reprocess_scope",
        ][..],
        "entity_link" => &["create_entity_link", "reprocess_scope"][..],
        "projection_cache" => &["rebuild_projection", "reprocess_scope"][..],
        _ => return Err(anyhow::anyhow!("invalid correction target_type")),
    };
    ensure_allowed(
        correction_type,
        allowed,
        "correction_type is not compatible with target_type",
    )
}

fn validate_manual_override_rule(command: &CorrectionCommand) -> anyhow::Result<()> {
    let has_source = command.source_claim_id.is_some() || command.source_span_id.is_some();
    if has_source || !is_positive_canonical_fact(&command.correction_type) {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "source-backed correction required for positive canonical fact"
    ))
}

fn is_positive_canonical_fact(correction_type: &str) -> bool {
    matches!(
        correction_type,
        "accept_quarantined_claim"
            | "merge_entities"
            | "create_entity_link"
            | "correct_property"
            | "correct_relationship_label"
            | "revise_knowledge_assertion"
            | "correct_place_parent"
    )
}

fn ensure_allowed(value: &str, allowed: &[&str], message: &str) -> anyhow::Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(anyhow::anyhow!("{}: {}", message, value))
    }
}

fn ensure_exists(condition: bool, message: &str) -> anyhow::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(anyhow::anyhow!(message.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!(
            "reader-correction-validation-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    async fn seed_claim(pool: &SqlitePool, claim_id: &str) {
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg1', 'b1', 'ch1', 'hash', 0, datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span1', 'b1', 'ch1', 'hash', 'seg1', 0, 0, 10, 'text', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'ch1', 'extract', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', 1, 'property_update', 'test', 'span1', 'run1', 0.8, 'high', 'quarantined', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn seed_relationship(pool: &SqlitePool) {
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e1', 'b1', 'character', 'A', 'A', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('e2', 'b1', 'character', 'B', 'B', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO relationships (id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('rel1', 'b1', 'e1', 'e2', 'friendship', 'friends', 'undirected', 0.7, 'positive', 0.8, 0.7, 1, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
    }

    fn base_claim_command(correction_type: &str) -> CorrectionCommand {
        CorrectionCommand {
            book_id: "b1".to_string(),
            target_type: "claim".to_string(),
            target_id: "c1".to_string(),
            correction_type: correction_type.to_string(),
            correction_json: "{}".to_string(),
            source: "user".to_string(),
            source_claim_id: Some("c1".to_string()),
            source_span_id: Some("span1".to_string()),
            created_by: "tester".to_string(),
        }
    }

    #[tokio::test]
    async fn valid_source_backed_claim_correction_persists_with_event() {
        let pool = setup_test_db().await;
        seed_claim(&pool, "c1").await;
        let service = CorrectionValidationService::new(pool.clone());

        let correction = service
            .validate_and_create(base_claim_command("accept_quarantined_claim"))
            .await
            .unwrap();

        assert_eq!(correction.correction_type, "accept_quarantined_claim");
        let events = QualityRepo::new(pool)
            .list_correction_events(&correction.id)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "proposed");
    }

    #[tokio::test]
    async fn invalid_target_type_and_incompatible_type_are_rejected() {
        let pool = setup_test_db().await;
        seed_claim(&pool, "c1").await;
        let service = CorrectionValidationService::new(pool);

        let mut invalid_target = base_claim_command("reject_claim");
        invalid_target.target_type = "book".to_string();
        assert!(service.validate(&invalid_target).await.is_err());

        let mut incompatible = base_claim_command("deactivate_relationship");
        incompatible.target_type = "claim".to_string();
        assert!(service.validate(&incompatible).await.is_err());
    }

    #[tokio::test]
    async fn manual_positive_fact_without_source_is_rejected_but_deactivation_is_allowed() {
        let pool = setup_test_db().await;
        seed_claim(&pool, "c1").await;
        seed_relationship(&pool).await;
        let service = CorrectionValidationService::new(pool);

        let mut positive = base_claim_command("accept_quarantined_claim");
        positive.source_claim_id = None;
        positive.source_span_id = None;
        assert!(service.validate(&positive).await.is_err());

        let deactivation = CorrectionCommand {
            book_id: "b1".to_string(),
            target_type: "relationship".to_string(),
            target_id: "rel1".to_string(),
            correction_type: "deactivate_relationship".to_string(),
            correction_json: "{}".to_string(),
            source: "user".to_string(),
            source_claim_id: None,
            source_span_id: None,
            created_by: "tester".to_string(),
        };
        service.validate(&deactivation).await.unwrap();
    }

    #[tokio::test]
    async fn missing_target_or_source_reference_is_rejected() {
        let pool = setup_test_db().await;
        seed_claim(&pool, "c1").await;
        let service = CorrectionValidationService::new(pool);

        let mut missing_target = base_claim_command("reject_claim");
        missing_target.target_id = "missing".to_string();
        assert!(service.validate(&missing_target).await.is_err());

        let mut missing_source = base_claim_command("accept_quarantined_claim");
        missing_source.source_span_id = Some("missing-span".to_string());
        assert!(service.validate(&missing_source).await.is_err());
    }
}
