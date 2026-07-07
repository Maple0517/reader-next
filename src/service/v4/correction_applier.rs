use crate::service::v4::correction::{CorrectionCommand, CorrectionValidationService};
use crate::service::v4::place_reducer::{
    apply_place_edge_status_write_with_conn, PlaceEdgeStatusWriteCommand, PlaceReductionResult,
};
use crate::service::v4::reducer::{
    apply_knowledge_assertion_status_write_with_conn, apply_relationship_status_write_with_conn,
    KnowledgeAssertionStatus, KnowledgeAssertionStatusWriteCommand, KnowledgeReductionResult,
    RelationshipReductionResult, RelationshipStatusWriteCommand,
};
use crate::storage::db::v4::claim_repo::ClaimRepo;
use crate::storage::db::v4::quality_repo::{NewCorrectionEvent, QualityRepo, UserCorrectionRecord};
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
                let mut result = RelationshipReductionResult::default();
                apply_relationship_status_write_with_conn(
                    &RelationshipStatusWriteCommand {
                        book_id: correction.book_id.clone(),
                        relationship_id: correction.target_id.clone(),
                        status: "inactive".to_string(),
                    },
                    conn,
                    &mut result,
                )
                .await?;
                Ok(format!(
                    "{{\"relationshipId\":\"{}\",\"status\":\"inactive\"}}",
                    correction.target_id
                ))
            }
            "mark_knowledge_assertion_false" => {
                let mut result = KnowledgeReductionResult::default();
                apply_knowledge_assertion_status_write_with_conn(
                    &KnowledgeAssertionStatusWriteCommand {
                        book_id: correction.book_id.clone(),
                        assertion_id: correction.target_id.clone(),
                        status: KnowledgeAssertionStatus::FalseInWorld,
                    },
                    conn,
                    &mut result,
                )
                .await?;
                Ok(format!(
                    "{{\"assertionId\":\"{}\",\"status\":\"false_in_world\"}}",
                    correction.target_id
                ))
            }
            "deactivate_place_edge" => {
                let mut result = PlaceReductionResult::default();
                apply_place_edge_status_write_with_conn(
                    &PlaceEdgeStatusWriteCommand {
                        book_id: correction.book_id.clone(),
                        edge_id: correction.target_id.clone(),
                        status: "deprecated".to_string(),
                    },
                    conn,
                    &mut result,
                )
                .await?;
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
    use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
    use crate::storage::db::v4::place_repo::PlaceRepo;
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

    async fn seed_knowledge_assertion(pool: &SqlitePool) -> String {
        seed_claim(pool, "knowledge_claim").await;
        let repo = KnowledgeRepo::new(pool.clone());
        let card = repo
            .find_or_create_card(
                "b1",
                "world_rule",
                "灵气",
                "灵气",
                Some("旧摘要"),
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();
        repo.find_or_create_assertion(
            "b1",
            &card.id,
            "knowledge_claim",
            "灵气充盈。",
            "active",
            0.8,
            0.7,
            1,
        )
        .await
        .unwrap()
        .id
    }

    async fn seed_place_edge(pool: &SqlitePool) -> String {
        seed_claim(pool, "place_claim").await;
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('p1', 'b1', 'place', '青云城', '青云城', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entities (id, book_id, entity_type, canonical_name, display_name, importance_score, first_seen_chapter, last_seen_chapter, status, created_at, updated_at) VALUES ('p2', 'b1', 'place', '黑风谷', '黑风谷', 0.5, 1, 1, 'active', datetime('now'), datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        PlaceRepo::new(pool.clone())
            .find_or_create_edge(
                "b1",
                "p1",
                "p2",
                "north_of",
                None,
                None,
                0.8,
                "place_claim",
                1,
            )
            .await
            .unwrap()
            .id
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

    #[test]
    fn correction_applier_does_not_embed_direct_canonical_status_update_sql() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/service/v4/correction_applier.rs"
        ))
        .unwrap();

        for forbidden in [
            concat!("UPDATE ", "relationships SET status"),
            concat!("UPDATE ", "knowledge_assertions SET status"),
            concat!("UPDATE ", "place_edges SET status"),
        ] {
            assert!(
                !source.contains(forbidden),
                "CorrectionApplier must route canonical status writes through reducer appliers, found {forbidden}"
            );
        }
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
    async fn mark_knowledge_assertion_false_updates_status_and_invalidates_knowledge_cache() {
        let pool = setup_test_db().await;
        let assertion_id = seed_knowledge_assertion(&pool).await;
        let cache_repo = CacheRepo::new(pool.clone());
        cache_repo
            .set_cached("b1", "knowledge", "__book__", 10, "{\"old\":true}")
            .await
            .unwrap();
        let correction = create_correction(
            &pool,
            CorrectionCommand {
                book_id: "b1".to_string(),
                target_type: "knowledge_assertion".to_string(),
                target_id: assertion_id.clone(),
                correction_type: "mark_knowledge_assertion_false".to_string(),
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
            sqlx::query_as("SELECT status FROM knowledge_assertions WHERE id = ?")
                .bind(&assertion_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status.0, "false_in_world");
        assert!(cache_repo
            .get_cached("b1", "knowledge", "__book__", 10)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn deactivate_place_edge_updates_status_and_invalidates_map_cache() {
        let pool = setup_test_db().await;
        let edge_id = seed_place_edge(&pool).await;
        let cache_repo = CacheRepo::new(pool.clone());
        cache_repo
            .set_cached("b1", "map_graph", "__book__", 10, "{\"old\":true}")
            .await
            .unwrap();
        let correction = create_correction(
            &pool,
            CorrectionCommand {
                book_id: "b1".to_string(),
                target_type: "place_edge".to_string(),
                target_id: edge_id.clone(),
                correction_type: "deactivate_place_edge".to_string(),
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
        let status: (String,) = sqlx::query_as("SELECT status FROM place_edges WHERE id = ?")
            .bind(&edge_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status.0, "deprecated");
        assert!(cache_repo
            .get_cached("b1", "map_graph", "__book__", 10)
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
