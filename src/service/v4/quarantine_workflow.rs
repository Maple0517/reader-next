use crate::storage::db::v4::ai_run_repo::{AiRunRecord, AiRunRepo};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo, SourceSpanRecord};
use crate::storage::db::v4::quality_repo::{
    NewCorrectionEvent, NewReprocessJob, NewUserCorrection, QualityRepo, QuarantinedClaimRecord,
    ReprocessJobRecord, UserCorrectionRecord,
};
use serde_json::json;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct QuarantineListFilter {
    pub status: Option<String>,
    pub reason_code: Option<String>,
    pub claim_type: Option<String>,
    pub limit: i64,
}

impl Default for QuarantineListFilter {
    fn default() -> Self {
        Self {
            status: Some("open".to_string()),
            reason_code: None,
            claim_type: None,
            limit: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuarantinedClaimView {
    pub workflow: QuarantinedClaimRecord,
    pub claim: ClaimRecord,
    pub source_spans: Vec<SourceSpanRecord>,
    pub ai_run: Option<AiRunRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineActionKind {
    Accept,
    Reject,
    Retry,
    Reclassify,
    Ignore,
}

#[derive(Debug, Clone)]
pub struct QuarantineActionRequest {
    pub action: QuarantineActionKind,
    pub actor: String,
    pub note: Option<String>,
    pub reason_code: Option<String>,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone)]
pub enum QuarantineActionResult {
    CorrectionCreated(UserCorrectionRecord),
    ReprocessJobCreated(ReprocessJobRecord),
    WorkflowUpdated(QuarantinedClaimRecord),
}

pub struct QuarantineWorkflowService {
    pool: SqlitePool,
    quality_repo: QualityRepo,
    claim_repo: ClaimRepo,
    ai_run_repo: AiRunRepo,
}

impl QuarantineWorkflowService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            quality_repo: QualityRepo::new(pool.clone()),
            claim_repo: ClaimRepo::new(pool.clone()),
            ai_run_repo: AiRunRepo::new(pool.clone()),
            pool,
        }
    }

    pub async fn list(
        &self,
        book_id: &str,
        filter: QuarantineListFilter,
    ) -> anyhow::Result<Vec<QuarantinedClaimView>> {
        let workflows = self
            .quality_repo
            .list_quarantined_claims(
                book_id,
                filter.status.as_deref(),
                filter.reason_code.as_deref(),
                filter.claim_type.as_deref(),
                filter.limit,
            )
            .await?;

        let mut views = Vec::with_capacity(workflows.len());
        for workflow in workflows {
            views.push(self.build_view(workflow).await?);
        }
        Ok(views)
    }

    pub async fn get_by_claim(
        &self,
        book_id: &str,
        claim_id: &str,
    ) -> anyhow::Result<Option<QuarantinedClaimView>> {
        let Some(workflow) = self
            .quality_repo
            .get_quarantined_claim_by_claim_id(book_id, claim_id)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(self.build_view(workflow).await?))
    }

    pub async fn act_on_claim(
        &self,
        book_id: &str,
        claim_id: &str,
        request: QuarantineActionRequest,
    ) -> anyhow::Result<QuarantineActionResult> {
        let workflow = self
            .quality_repo
            .get_quarantined_claim_by_claim_id(book_id, claim_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("quarantined claim workflow not found"))?;
        let claim = self
            .claim_repo
            .get_claim(claim_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("claim not found"))?;

        match request.action {
            QuarantineActionKind::Accept => self.accept(workflow, claim, request).await,
            QuarantineActionKind::Reject => self.reject(workflow, request).await,
            QuarantineActionKind::Retry => self.retry(workflow, claim, request).await,
            QuarantineActionKind::Reclassify => self.reclassify(workflow, request).await,
            QuarantineActionKind::Ignore => self.ignore(workflow, request).await,
        }
    }

    async fn build_view(
        &self,
        workflow: QuarantinedClaimRecord,
    ) -> anyhow::Result<QuarantinedClaimView> {
        let claim = self
            .claim_repo
            .get_claim(&workflow.claim_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("claim not found for quarantine workflow"))?;
        let source_spans = self.claim_repo.list_claim_spans(&claim.id).await?;
        let ai_run = self.ai_run_repo.get_run(&claim.ai_run_id).await?;
        Ok(QuarantinedClaimView {
            workflow,
            claim,
            source_spans,
            ai_run,
        })
    }

    async fn accept(
        &self,
        workflow: QuarantinedClaimRecord,
        claim: ClaimRecord,
        request: QuarantineActionRequest,
    ) -> anyhow::Result<QuarantineActionResult> {
        let mut tx = self.pool.begin().await?;
        let correction_json = json!({
            "claimId": claim.id,
            "workflowId": workflow.id,
            "reviewNote": request.note,
            "requiresReducerApply": true
        })
        .to_string();
        let correction = QualityRepo::create_user_correction_with_conn(
            &mut tx,
            NewUserCorrection {
                id: None,
                book_id: &workflow.book_id,
                target_type: "claim",
                target_id: &claim.id,
                correction_type: "accept_quarantined_claim",
                correction_json: &correction_json,
                source: "user",
                source_claim_id: Some(&claim.id),
                source_span_id: Some(&claim.primary_source_span_id),
                created_by: &request.actor,
            },
        )
        .await?;
        QualityRepo::append_correction_event_with_conn(
            &mut tx,
            NewCorrectionEvent {
                id: None,
                book_id: &workflow.book_id,
                correction_id: &correction.id,
                event_type: "proposed",
                before_json: None,
                after_json: Some(&correction_json),
                actor: &request.actor,
                reason: request.note.as_deref(),
            },
        )
        .await?;
        QualityRepo::update_quarantine_status_with_conn(
            &mut tx,
            &workflow.id,
            "accepted",
            Some("accept_quarantined_claim"),
            request.note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        Ok(QuarantineActionResult::CorrectionCreated(correction))
    }

    async fn reject(
        &self,
        workflow: QuarantinedClaimRecord,
        request: QuarantineActionRequest,
    ) -> anyhow::Result<QuarantineActionResult> {
        let mut tx = self.pool.begin().await?;
        ClaimRepo::update_claim_status_with_conn(&mut tx, &workflow.claim_id, "rejected").await?;
        QualityRepo::update_quarantine_status_with_conn(
            &mut tx,
            &workflow.id,
            "rejected",
            Some("reject_claim"),
            request.note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        let updated = self
            .quality_repo
            .get_quarantined_claim_by_claim_id(&workflow.book_id, &workflow.claim_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("updated quarantine workflow missing"))?;
        Ok(QuarantineActionResult::WorkflowUpdated(updated))
    }

    async fn retry(
        &self,
        workflow: QuarantinedClaimRecord,
        claim: ClaimRecord,
        request: QuarantineActionRequest,
    ) -> anyhow::Result<QuarantineActionResult> {
        let mut tx = self.pool.begin().await?;
        let scope_json = json!({
            "claimId": claim.id,
            "chapterIndex": claim.chapter_index,
            "claimType": claim.claim_type
        })
        .to_string();
        let job = QualityRepo::create_reprocess_job_with_conn(
            &mut tx,
            NewReprocessJob {
                id: None,
                book_id: &workflow.book_id,
                scope_type: "claim",
                scope_json: &scope_json,
                mode: "retry_failed",
                requested_by: &request.actor,
                reason: request.note.as_deref(),
                dry_run: true,
                prompt_version: None,
                schema_version: None,
            },
        )
        .await?;
        QualityRepo::update_quarantine_status_with_conn(
            &mut tx,
            &workflow.id,
            "retried",
            Some("retry_claim"),
            request.note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        Ok(QuarantineActionResult::ReprocessJobCreated(job))
    }

    async fn reclassify(
        &self,
        workflow: QuarantinedClaimRecord,
        request: QuarantineActionRequest,
    ) -> anyhow::Result<QuarantineActionResult> {
        let reason_code = request
            .reason_code
            .as_deref()
            .unwrap_or(&workflow.reason_code)
            .to_string();
        let suggested_action = request
            .suggested_action
            .as_deref()
            .unwrap_or(&workflow.suggested_action)
            .to_string();
        let updated = self
            .quality_repo
            .upsert_quarantined_claim(crate::storage::db::v4::quality_repo::NewQuarantinedClaim {
                id: Some(&workflow.id),
                book_id: &workflow.book_id,
                claim_id: &workflow.claim_id,
                reason_code: &reason_code,
                reason_text: request.note.as_deref().or(workflow.reason_text.as_deref()),
                suggested_action: &suggested_action,
                priority: workflow.priority,
            })
            .await?;
        self.quality_repo
            .update_quarantine_status(
                &updated.id,
                "reclassified",
                Some("reclassify_claim"),
                request.note.as_deref(),
            )
            .await?;
        let updated = self
            .quality_repo
            .get_quarantined_claim_by_claim_id(&workflow.book_id, &workflow.claim_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("updated quarantine workflow missing"))?;
        Ok(QuarantineActionResult::WorkflowUpdated(updated))
    }

    async fn ignore(
        &self,
        workflow: QuarantinedClaimRecord,
        request: QuarantineActionRequest,
    ) -> anyhow::Result<QuarantineActionResult> {
        self.quality_repo
            .update_quarantine_status(
                &workflow.id,
                "ignored",
                Some("ignore"),
                request.note.as_deref(),
            )
            .await?;
        let updated = self
            .quality_repo
            .get_quarantined_claim_by_claim_id(&workflow.book_id, &workflow.claim_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("updated quarantine workflow missing"))?;
        Ok(QuarantineActionResult::WorkflowUpdated(updated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::quality_repo::NewQuarantinedClaim;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!(
            "reader-quarantine-workflow-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    async fn seed_quarantined_claim(pool: &SqlitePool, claim_id: &str, claim_type: &str) {
        seed_quarantined_claim_at(pool, claim_id, claim_type, 1).await;
    }

    async fn seed_quarantined_claim_at(
        pool: &SqlitePool,
        claim_id: &str,
        claim_type: &str,
        chapter_index: i64,
    ) {
        let chapter_id = format!("ch-{claim_id}");
        let segment_id = format!("seg-{claim_id}");
        let span_id = format!("span-{claim_id}");
        let run_id = format!("run-{claim_id}");
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', ?, 'text', 'hash', datetime('now'))")
            .bind(&chapter_id)
            .bind(chapter_index)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))")
            .bind(&segment_id)
            .bind(&chapter_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, 'text', datetime('now'))")
            .bind(&span_id)
            .bind(&chapter_id)
            .bind(&segment_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, output_json, status, started_at, finished_at) VALUES (?, 'b1', ?, 'extract', 'test', 'v1', 1, 'input', '{\"ok\":true}', 'completed', datetime('now'), datetime('now'))")
            .bind(&run_id)
            .bind(&chapter_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, value_json, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', ?, ?, 'test', '{}', ?, ?, 0.4, 'high', 'quarantined', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .bind(chapter_index)
            .bind(claim_type)
            .bind(&span_id)
            .bind(&run_id)
            .execute(pool)
            .await
            .unwrap();
        ClaimRepo::new(pool.clone())
            .add_claim_source_span(claim_id, &span_id, "primary")
            .await
            .unwrap();
        QualityRepo::new(pool.clone())
            .upsert_quarantined_claim(NewQuarantinedClaim {
                id: Some(&format!("q-{claim_id}")),
                book_id: "b1",
                claim_id,
                reason_code: "low_confidence",
                reason_text: Some("weak evidence"),
                suggested_action: "needs_manual_review",
                priority: 5,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn list_includes_claim_source_span_and_ai_run_context() {
        let pool = setup_test_db().await;
        seed_quarantined_claim(&pool, "c1", "relationship_update").await;
        let service = QuarantineWorkflowService::new(pool);

        let views = service
            .list(
                "b1",
                QuarantineListFilter {
                    status: Some("open".to_string()),
                    reason_code: Some("low_confidence".to_string()),
                    claim_type: Some("relationship_update".to_string()),
                    limit: 10,
                },
            )
            .await
            .unwrap();

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].claim.id, "c1");
        assert_eq!(views[0].source_spans.len(), 1);
        assert_eq!(views[0].ai_run.as_ref().unwrap().status, "completed");
    }

    #[tokio::test]
    async fn accept_creates_correction_without_accepting_claim() {
        let pool = setup_test_db().await;
        seed_quarantined_claim(&pool, "c1", "property_update").await;
        let service = QuarantineWorkflowService::new(pool.clone());

        let result = service
            .act_on_claim(
                "b1",
                "c1",
                QuarantineActionRequest {
                    action: QuarantineActionKind::Accept,
                    actor: "tester".to_string(),
                    note: Some("accept original evidence".to_string()),
                    reason_code: None,
                    suggested_action: None,
                },
            )
            .await
            .unwrap();

        let QuarantineActionResult::CorrectionCreated(correction) = result else {
            panic!("accept should create correction");
        };
        assert_eq!(correction.correction_type, "accept_quarantined_claim");
        let claim = ClaimRepo::new(pool.clone())
            .get_claim("c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claim.status, "quarantined");
        let workflow = QualityRepo::new(pool)
            .get_quarantined_claim_by_claim_id("b1", "c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(workflow.status, "accepted");
    }

    #[tokio::test]
    async fn reject_marks_claim_rejected_and_retry_creates_dry_run_job() {
        let pool = setup_test_db().await;
        seed_quarantined_claim_at(&pool, "c1", "property_update", 1).await;
        seed_quarantined_claim_at(&pool, "c2", "relationship_update", 2).await;
        let service = QuarantineWorkflowService::new(pool.clone());

        service
            .act_on_claim(
                "b1",
                "c1",
                QuarantineActionRequest {
                    action: QuarantineActionKind::Reject,
                    actor: "tester".to_string(),
                    note: Some("bad claim".to_string()),
                    reason_code: None,
                    suggested_action: None,
                },
            )
            .await
            .unwrap();
        let claim = ClaimRepo::new(pool.clone())
            .get_claim("c1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claim.status, "rejected");

        let result = service
            .act_on_claim(
                "b1",
                "c2",
                QuarantineActionRequest {
                    action: QuarantineActionKind::Retry,
                    actor: "tester".to_string(),
                    note: Some("retry with new prompt".to_string()),
                    reason_code: None,
                    suggested_action: None,
                },
            )
            .await
            .unwrap();
        let QuarantineActionResult::ReprocessJobCreated(job) = result else {
            panic!("retry should create reprocess job");
        };
        assert_eq!(job.scope_type, "claim");
        assert_eq!(job.mode, "retry_failed");
        assert_eq!(job.dry_run, 1);
    }
}
