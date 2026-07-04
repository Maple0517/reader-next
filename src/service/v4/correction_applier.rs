use crate::service::v4::correction::{CorrectionCommand, CorrectionValidationService};
use crate::storage::db::v4::claim_repo::ClaimRepo;
use crate::storage::db::v4::quality_repo::{NewCorrectionEvent, QualityRepo, UserCorrectionRecord};
use crate::storage::db::v4::relationship_repo::{RelationshipRecord, RelationshipRepo};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionApplyStatus {
    Applied,
    AlreadyApplied,
    Failed,
}

#[derive(Debug, Clone)]
pub struct CorrectionApplyResult {
    pub correction_id: String,
    pub status: CorrectionApplyStatus,
    pub message: Option<String>,
}

pub struct CorrectionApplier {
    pool: SqlitePool,
    quality_repo: QualityRepo,
    validation: CorrectionValidationService,
}

impl CorrectionApplier {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            quality_repo: QualityRepo::new(pool.clone()),
            validation: CorrectionValidationService::new(pool.clone()),
            pool,
        }
    }

    pub async fn apply(
        &self,
        correction_id: &str,
        actor: &str,
    ) -> anyhow::Result<CorrectionApplyResult> {
        let correction = self
            .quality_repo
            .get_user_correction(correction_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("correction not found"))?;

        if correction.status == "applied" {
            return Ok(CorrectionApplyResult {
                correction_id: correction.id,
                status: CorrectionApplyStatus::AlreadyApplied,
                message: Some("correction already applied".to_string()),
            });
        }

        if let Err(err) = self.validate_existing(&correction).await {
            self.record_failed(&correction, actor, &err.to_string())
                .await?;
            return Ok(CorrectionApplyResult {
                correction_id: correction.id,
                status: CorrectionApplyStatus::Failed,
                message: Some(err.to_string()),
            });
        }

        let mut tx = self.pool.begin().await?;
        let apply_result = self.apply_inner(&correction, &mut tx).await;
        match apply_result {
            Ok(after_json) => {
                QualityRepo::update_user_correction_status_with_conn(
                    &mut *tx,
                    &correction.id,
                    "applied",
                    Some(actor),
                    None,
                )
                .await?;
                QualityRepo::append_correction_event_with_conn(
                    &mut *tx,
                    NewCorrectionEvent {
                        id: None,
                        book_id: &correction.book_id,
                        correction_id: &correction.id,
                        event_type: "applied",
                        before_json: None,
                        after_json: Some(&after_json),
                        actor,
                        reason: Some("correction applied"),
                    },
                )
                .await?;
                tx.commit().await?;
                Ok(CorrectionApplyResult {
                    correction_id: correction.id,
                    status: CorrectionApplyStatus::Applied,
                    message: None,
                })
            }
            Err(err) => {
                tx.rollback().await?;
                self.record_failed(&correction, actor, &err.to_string())
                    .await?;
                Ok(CorrectionApplyResult {
                    correction_id: correction.id,
                    status: CorrectionApplyStatus::Failed,
                    message: Some(err.to_string()),
                })
            }
        }
    }

    async fn validate_existing(&self, correction: &UserCorrectionRecord) -> anyhow::Result<()> {
        self.validation
            .validate(&CorrectionCommand {
                book_id: correction.book_id.clone(),
                target_type: correction.target_type.clone(),
                target_id: correction.target_id.clone(),
                correction_type: correction.correction_type.clone(),
                correction_json: correction.correction_json.clone(),
                source: correction.source.clone(),
                source_claim_id: correction.source_claim_id.clone(),
                source_span_id: correction.source_span_id.clone(),
                created_by: correction.created_by.clone(),
            })
            .await
    }

    async fn apply_inner(
        &self,
        correction: &UserCorrectionRecord,
        conn: &mut SqliteConnection,
    ) -> anyhow::Result<String> {
        match correction.correction_type.as_str() {
            "reject_claim" => {
                ClaimRepo::update_claim_status_with_conn(conn, &correction.target_id, "rejected")
                    .await?;
                Ok(format!(
                    "{{\"claimId\":\"{}\",\"status\":\"rejected\"}}",
                    correction.target_id
                ))
            }
            "retry_claim" | "reprocess_scope" => Err(anyhow::anyhow!(
                "reprocess corrections must be routed through reprocess job service"
            )),
            "accept_quarantined_claim"
            | "merge_entities"
            | "create_entity_link"
            | "correct_property"
            | "correct_relationship_label"
            | "revise_knowledge_assertion"
            | "correct_place_parent" => Err(anyhow::anyhow!(
                "positive canonical correction requires domain reducer implementation"
            )),
            "mark_not_same_entity" | "split_required" => Err(anyhow::anyhow!(
                "identity correction apply is handled by identity reducer in a later route"
            )),
            "deactivate_relationship" => {
                let relationship = self
                    .get_relationship_for_update(conn, &correction.book_id, &correction.target_id)
                    .await?;
                RelationshipRepo::update_relationship_with_conn(
                    conn,
                    &relationship.id,
                    &relationship.relation_label,
                    relationship.current_state.as_deref(),
                    relationship.strength,
                    &relationship.polarity,
                    relationship.confidence,
                    relationship.importance_score,
                    relationship.last_changed_chapter,
                    relationship.last_seen_chapter,
                    "inactive",
                )
                .await?;
                self.invalidate_cache(conn, &correction.book_id, "relationship_graph", "__book__")
                    .await?;
                self.invalidate_cache(conn, &correction.book_id, "relationship_list", "__book__")
                    .await?;
                Ok(format!(
                    "{{\"relationshipId\":\"{}\",\"status\":\"inactive\"}}",
                    correction.target_id
                ))
            }
            "mark_knowledge_assertion_false" => {
                self.update_status(
                    conn,
                    "UPDATE knowledge_assertions SET status = 'false_in_world', updated_at = datetime('now') WHERE book_id = ? AND id = ?",
                    &correction.book_id,
                    &correction.target_id,
                )
                .await?;
                self.invalidate_all_cache(conn, &correction.book_id).await?;
                Ok(format!(
                    "{{\"assertionId\":\"{}\",\"status\":\"false_in_world\"}}",
                    correction.target_id
                ))
            }
            "deactivate_place_edge" => {
                self.update_status(
                    conn,
                    "UPDATE place_edges SET status = 'deprecated', updated_at = datetime('now') WHERE book_id = ? AND id = ?",
                    &correction.book_id,
                    &correction.target_id,
                )
                .await?;
                self.invalidate_all_cache(conn, &correction.book_id).await?;
                Ok(format!(
                    "{{\"placeEdgeId\":\"{}\",\"status\":\"deprecated\"}}",
                    correction.target_id
                ))
            }
            "mark_place_edge_conflict" => Err(anyhow::anyhow!(
                "mark_place_edge_conflict requires source claim context and map conflict route"
            )),
            "rebuild_projection" => {
                self.invalidate_all_cache(conn, &correction.book_id).await?;
                Ok(format!(
                    "{{\"bookId\":\"{}\",\"projectionCache\":\"invalidated\"}}",
                    correction.book_id
                ))
            }
            "reclassify_claim" => Ok(format!(
                "{{\"claimId\":\"{}\",\"status\":\"workflow-only\"}}",
                correction.target_id
            )),
            other => Err(anyhow::anyhow!("unsupported correction_type: {}", other)),
        }
    }

    async fn get_relationship_for_update(
        &self,
        conn: &mut SqliteConnection,
        book_id: &str,
        relationship_id: &str,
    ) -> anyhow::Result<RelationshipRecord> {
        sqlx::query_as::<_, RelationshipRecord>(
            "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
             FROM relationships WHERE book_id = ? AND id = ?",
        )
        .bind(book_id)
        .bind(relationship_id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| anyhow::anyhow!("relationship not found"))
    }

    async fn update_status(
        &self,
        conn: &mut SqliteConnection,
        sql: &str,
        book_id: &str,
        target_id: &str,
    ) -> anyhow::Result<()> {
        let result = sqlx::query(sql)
            .bind(book_id)
            .bind(target_id)
            .execute(conn)
            .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("correction target was not updated"));
        }
        Ok(())
    }

    async fn invalidate_cache(
        &self,
        conn: &mut SqliteConnection,
        book_id: &str,
        view_type: &str,
        scope_id: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "DELETE FROM view_model_cache WHERE book_id = ? AND view_type = ? AND scope_id = ?",
        )
        .bind(book_id)
        .bind(view_type)
        .bind(scope_id)
        .execute(conn)
        .await?;
        Ok(())
    }

    async fn invalidate_all_cache(
        &self,
        conn: &mut SqliteConnection,
        book_id: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM view_model_cache WHERE book_id = ?")
            .bind(book_id)
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn record_failed(
        &self,
        correction: &UserCorrectionRecord,
        actor: &str,
        error: &str,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        QualityRepo::update_user_correction_status_with_conn(
            &mut tx,
            &correction.id,
            "failed",
            Some(actor),
            Some(error),
        )
        .await?;
        QualityRepo::append_correction_event_with_conn(
            &mut tx,
            NewCorrectionEvent {
                id: None,
                book_id: &correction.book_id,
                correction_id: &correction.id,
                event_type: "failed",
                before_json: None,
                after_json: None,
                actor,
                reason: Some(error),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::v4::correction::{CorrectionCommand, CorrectionValidationService};
    use crate::storage::db;
    use crate::storage::db::v4::cache_repo::CacheRepo;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!(
            "reader-correction-applier-{}",
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

    async fn create_correction(
        pool: &SqlitePool,
        command: CorrectionCommand,
    ) -> UserCorrectionRecord {
        CorrectionValidationService::new(pool.clone())
            .validate_and_create(command)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn reject_claim_apply_is_idempotent() {
        let pool = setup_test_db().await;
        seed_claim(&pool, "c1").await;
        let correction = create_correction(
            &pool,
            CorrectionCommand {
                book_id: "b1".to_string(),
                target_type: "claim".to_string(),
                target_id: "c1".to_string(),
                correction_type: "reject_claim".to_string(),
                correction_json: "{}".to_string(),
                source: "user".to_string(),
                source_claim_id: None,
                source_span_id: None,
                created_by: "tester".to_string(),
            },
        )
        .await;
        let applier = CorrectionApplier::new(pool.clone());
        let first = applier.apply(&correction.id, "tester").await.unwrap();
        let second = applier.apply(&correction.id, "tester").await.unwrap();

        assert_eq!(first.status, CorrectionApplyStatus::Applied);
        assert_eq!(second.status, CorrectionApplyStatus::AlreadyApplied);
        let claim = ClaimRepo::new(pool).get_claim("c1").await.unwrap().unwrap();
        assert_eq!(claim.status, "rejected");
    }

    #[tokio::test]
    async fn deactivate_relationship_preserves_row_and_marks_inactive() {
        let pool = setup_test_db().await;
        seed_relationship(&pool).await;
        let correction = create_correction(
            &pool,
            CorrectionCommand {
                book_id: "b1".to_string(),
                target_type: "relationship".to_string(),
                target_id: "rel1".to_string(),
                correction_type: "deactivate_relationship".to_string(),
                correction_json: "{}".to_string(),
                source: "user".to_string(),
                source_claim_id: None,
                source_span_id: None,
                created_by: "tester".to_string(),
            },
        )
        .await;
        let result = CorrectionApplier::new(pool.clone())
            .apply(&correction.id, "tester")
            .await
            .unwrap();
        assert_eq!(result.status, CorrectionApplyStatus::Applied);

        let status: (String,) =
            sqlx::query_as("SELECT status FROM relationships WHERE id = 'rel1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status.0, "inactive");
    }

    #[tokio::test]
    async fn deactivate_relationship_invalidates_graph_and_list_cache() {
        let pool = setup_test_db().await;
        seed_relationship(&pool).await;
        let cache_repo = CacheRepo::new(pool.clone());
        cache_repo
            .set_cached("b1", "relationship_graph", "__book__", 10, "{\"old\":true}")
            .await
            .unwrap();
        cache_repo
            .set_cached("b1", "relationship_list", "__book__", 10, "{\"old\":true}")
            .await
            .unwrap();
        let correction = create_correction(
            &pool,
            CorrectionCommand {
                book_id: "b1".to_string(),
                target_type: "relationship".to_string(),
                target_id: "rel1".to_string(),
                correction_type: "deactivate_relationship".to_string(),
                correction_json: "{}".to_string(),
                source: "user".to_string(),
                source_claim_id: None,
                source_span_id: None,
                created_by: "tester".to_string(),
            },
        )
        .await;

        let result = CorrectionApplier::new(pool.clone())
            .apply(&correction.id, "tester")
            .await
            .unwrap();

        assert_eq!(result.status, CorrectionApplyStatus::Applied);
        assert!(cache_repo
            .get_cached("b1", "relationship_graph", "__book__", 10)
            .await
            .unwrap()
            .is_none());
        assert!(cache_repo
            .get_cached("b1", "relationship_list", "__book__", 10)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn positive_canonical_apply_fails_without_partial_claim_status_change() {
        let pool = setup_test_db().await;
        seed_claim(&pool, "c1").await;
        let correction = create_correction(
            &pool,
            CorrectionCommand {
                book_id: "b1".to_string(),
                target_type: "claim".to_string(),
                target_id: "c1".to_string(),
                correction_type: "accept_quarantined_claim".to_string(),
                correction_json: "{}".to_string(),
                source: "user".to_string(),
                source_claim_id: Some("c1".to_string()),
                source_span_id: Some("span1".to_string()),
                created_by: "tester".to_string(),
            },
        )
        .await;
        let result = CorrectionApplier::new(pool.clone())
            .apply(&correction.id, "tester")
            .await
            .unwrap();
        assert_eq!(result.status, CorrectionApplyStatus::Failed);
        let claim = ClaimRepo::new(pool.clone())
            .get_claim("c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claim.status, "quarantined");
        let failed = QualityRepo::new(pool)
            .get_user_correction(&correction.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(failed.status, "failed");
    }
}
