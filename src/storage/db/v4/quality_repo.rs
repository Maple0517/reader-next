use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QuarantinedClaimRecord {
    pub id: String,
    pub book_id: String,
    pub claim_id: String,
    pub reason_code: String,
    pub reason_text: Option<String>,
    pub suggested_action: String,
    pub status: String,
    pub priority: i64,
    pub assigned_to: Option<String>,
    pub reviewed_at: Option<String>,
    pub review_decision: Option<String>,
    pub review_note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserCorrectionRecord {
    pub id: String,
    pub book_id: String,
    pub target_type: String,
    pub target_id: String,
    pub correction_type: String,
    pub correction_json: String,
    pub status: String,
    pub source: String,
    pub source_claim_id: Option<String>,
    pub source_span_id: Option<String>,
    pub created_by: String,
    pub applied_by: Option<String>,
    pub created_at: String,
    pub applied_at: Option<String>,
    pub reverted_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CorrectionEventRecord {
    pub id: String,
    pub book_id: String,
    pub correction_id: String,
    pub event_type: String,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub actor: String,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QualityAuditRunRecord {
    pub id: String,
    pub book_id: String,
    pub audit_type: String,
    pub scope_json: String,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub summary_json: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QualityAuditFindingRecord {
    pub id: String,
    pub book_id: String,
    pub audit_run_id: String,
    pub finding_type: String,
    pub severity: String,
    pub target_type: String,
    pub target_id: String,
    pub related_target_type: Option<String>,
    pub related_target_id: Option<String>,
    pub reason_code: String,
    pub reason_text: Option<String>,
    pub evidence_json: Option<String>,
    pub suggested_action: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QualityMetricRecord {
    pub id: String,
    pub book_id: String,
    pub metric_type: String,
    pub metric_value: f64,
    pub metric_json: Option<String>,
    pub measured_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReprocessJobRecord {
    pub id: String,
    pub book_id: String,
    pub scope_type: String,
    pub scope_json: String,
    pub mode: String,
    pub status: String,
    pub requested_by: String,
    pub reason: Option<String>,
    pub dry_run: i64,
    pub prompt_version: Option<String>,
    pub schema_version: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub result_json: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PromptRegressionRunRecord {
    pub id: String,
    pub book_id: String,
    pub prompt_version: String,
    pub schema_version: String,
    pub model: String,
    pub fixture_set: String,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub summary_json: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PromptRegressionResultRecord {
    pub id: String,
    pub run_id: String,
    pub case_id: String,
    pub case_name: String,
    pub domain: String,
    pub expected_json: String,
    pub actual_json: String,
    pub pass: i64,
    pub diff_json: Option<String>,
    pub created_at: String,
}

pub struct NewQuarantinedClaim<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub claim_id: &'a str,
    pub reason_code: &'a str,
    pub reason_text: Option<&'a str>,
    pub suggested_action: &'a str,
    pub priority: i64,
}

pub struct NewUserCorrection<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub target_type: &'a str,
    pub target_id: &'a str,
    pub correction_type: &'a str,
    pub correction_json: &'a str,
    pub source: &'a str,
    pub source_claim_id: Option<&'a str>,
    pub source_span_id: Option<&'a str>,
    pub created_by: &'a str,
}

pub struct NewCorrectionEvent<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub correction_id: &'a str,
    pub event_type: &'a str,
    pub before_json: Option<&'a str>,
    pub after_json: Option<&'a str>,
    pub actor: &'a str,
    pub reason: Option<&'a str>,
}

pub struct NewAuditRun<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub audit_type: &'a str,
    pub scope_json: &'a str,
    pub status: &'a str,
}

pub struct NewAuditFinding<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub audit_run_id: &'a str,
    pub finding_type: &'a str,
    pub severity: &'a str,
    pub target_type: &'a str,
    pub target_id: &'a str,
    pub related_target_type: Option<&'a str>,
    pub related_target_id: Option<&'a str>,
    pub reason_code: &'a str,
    pub reason_text: Option<&'a str>,
    pub evidence_json: Option<&'a str>,
    pub suggested_action: &'a str,
}

pub struct NewQualityMetric<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub metric_type: &'a str,
    pub metric_value: f64,
    pub metric_json: Option<&'a str>,
}

pub struct NewReprocessJob<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub scope_type: &'a str,
    pub scope_json: &'a str,
    pub mode: &'a str,
    pub requested_by: &'a str,
    pub reason: Option<&'a str>,
    pub dry_run: bool,
    pub prompt_version: Option<&'a str>,
    pub schema_version: Option<&'a str>,
}

pub struct NewPromptRegressionRun<'a> {
    pub id: Option<&'a str>,
    pub book_id: &'a str,
    pub prompt_version: &'a str,
    pub schema_version: &'a str,
    pub model: &'a str,
    pub fixture_set: &'a str,
    pub status: &'a str,
}

pub struct NewPromptRegressionResult<'a> {
    pub id: Option<&'a str>,
    pub run_id: &'a str,
    pub case_id: &'a str,
    pub case_name: &'a str,
    pub domain: &'a str,
    pub expected_json: &'a str,
    pub actual_json: &'a str,
    pub pass: bool,
    pub diff_json: Option<&'a str>,
}

pub struct QualityRepo {
    pool: SqlitePool,
}

impl QualityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_quarantined_claim(
        &self,
        input: NewQuarantinedClaim<'_>,
    ) -> anyhow::Result<QuarantinedClaimRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::upsert_quarantined_claim_with_conn(&mut conn, input).await
    }

    pub async fn upsert_quarantined_claim_with_conn(
        conn: &mut SqliteConnection,
        input: NewQuarantinedClaim<'_>,
    ) -> anyhow::Result<QuarantinedClaimRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        let now = now();
        sqlx::query(
            "INSERT INTO quarantined_claims (id, book_id, claim_id, reason_code, reason_text, suggested_action, status, priority, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, 'open', ?, ?, ?)
             ON CONFLICT(book_id, claim_id) DO UPDATE SET
               reason_code = excluded.reason_code,
               reason_text = excluded.reason_text,
               suggested_action = excluded.suggested_action,
               priority = excluded.priority,
               updated_at = excluded.updated_at",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.claim_id)
        .bind(input.reason_code)
        .bind(input.reason_text)
        .bind(input.suggested_action)
        .bind(input.priority)
        .bind(&now)
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        Self::get_quarantined_claim_by_claim_id_with_conn(conn, input.book_id, input.claim_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("quarantined claim was not persisted"))
    }

    pub async fn get_quarantined_claim_by_claim_id(
        &self,
        book_id: &str,
        claim_id: &str,
    ) -> anyhow::Result<Option<QuarantinedClaimRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_quarantined_claim_by_claim_id_with_conn(&mut conn, book_id, claim_id).await
    }

    pub async fn get_quarantined_claim_by_claim_id_with_conn(
        conn: &mut SqliteConnection,
        book_id: &str,
        claim_id: &str,
    ) -> anyhow::Result<Option<QuarantinedClaimRecord>> {
        sqlx::query_as::<_, QuarantinedClaimRecord>(
            "SELECT id, book_id, claim_id, reason_code, reason_text, suggested_action, status, priority, assigned_to, reviewed_at, review_decision, review_note, created_at, updated_at
             FROM quarantined_claims WHERE book_id = ? AND claim_id = ?",
        )
        .bind(book_id)
        .bind(claim_id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn list_quarantined_claims(
        &self,
        book_id: &str,
        status: Option<&str>,
        reason_code: Option<&str>,
        claim_type: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<QuarantinedClaimRecord>> {
        sqlx::query_as::<_, QuarantinedClaimRecord>(
            "SELECT q.id, q.book_id, q.claim_id, q.reason_code, q.reason_text, q.suggested_action, q.status, q.priority, q.assigned_to, q.reviewed_at, q.review_decision, q.review_note, q.created_at, q.updated_at
             FROM quarantined_claims q
             JOIN claims c ON c.id = q.claim_id
             WHERE q.book_id = ?
               AND (? IS NULL OR q.status = ?)
               AND (? IS NULL OR q.reason_code = ?)
               AND (? IS NULL OR c.claim_type = ?)
             ORDER BY q.priority DESC, q.created_at ASC
             LIMIT ?",
        )
        .bind(book_id)
        .bind(status)
        .bind(status)
        .bind(reason_code)
        .bind(reason_code)
        .bind(claim_type)
        .bind(claim_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn update_quarantine_status(
        &self,
        id: &str,
        status: &str,
        review_decision: Option<&str>,
        review_note: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut conn = self.pool.acquire().await?;
        Self::update_quarantine_status_with_conn(
            &mut conn,
            id,
            status,
            review_decision,
            review_note,
        )
        .await
    }

    pub async fn update_quarantine_status_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
        status: &str,
        review_decision: Option<&str>,
        review_note: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = now();
        sqlx::query(
            "UPDATE quarantined_claims
             SET status = ?, review_decision = ?, review_note = ?, reviewed_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(review_decision)
        .bind(review_note)
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn create_user_correction(
        &self,
        input: NewUserCorrection<'_>,
    ) -> anyhow::Result<UserCorrectionRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::create_user_correction_with_conn(&mut conn, input).await
    }

    pub async fn create_user_correction_with_conn(
        conn: &mut SqliteConnection,
        input: NewUserCorrection<'_>,
    ) -> anyhow::Result<UserCorrectionRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        let now = now();
        sqlx::query(
            "INSERT OR IGNORE INTO user_corrections (id, book_id, target_type, target_id, correction_type, correction_json, status, source, source_claim_id, source_span_id, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 'proposed', ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.target_type)
        .bind(input.target_id)
        .bind(input.correction_type)
        .bind(input.correction_json)
        .bind(input.source)
        .bind(input.source_claim_id)
        .bind(input.source_span_id)
        .bind(input.created_by)
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        Self::get_user_correction_with_conn(conn, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("user correction was not persisted"))
    }

    pub async fn get_user_correction(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<UserCorrectionRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_user_correction_with_conn(&mut conn, id).await
    }

    pub async fn get_user_correction_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> anyhow::Result<Option<UserCorrectionRecord>> {
        sqlx::query_as::<_, UserCorrectionRecord>(
            "SELECT id, book_id, target_type, target_id, correction_type, correction_json, status, source, source_claim_id, source_span_id, created_by, applied_by, created_at, applied_at, reverted_at, error
             FROM user_corrections WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn list_user_corrections(
        &self,
        book_id: &str,
        status: Option<&str>,
        correction_type: Option<&str>,
        target_type: Option<&str>,
        target_id: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<UserCorrectionRecord>> {
        sqlx::query_as::<_, UserCorrectionRecord>(
            "SELECT id, book_id, target_type, target_id, correction_type, correction_json, status, source, source_claim_id, source_span_id, created_by, applied_by, created_at, applied_at, reverted_at, error
             FROM user_corrections
             WHERE book_id = ?
               AND (? IS NULL OR status = ?)
               AND (? IS NULL OR correction_type = ?)
               AND (? IS NULL OR target_type = ?)
               AND (? IS NULL OR target_id = ?)
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(book_id)
        .bind(status)
        .bind(status)
        .bind(correction_type)
        .bind(correction_type)
        .bind(target_type)
        .bind(target_type)
        .bind(target_id)
        .bind(target_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn update_user_correction_status_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
        status: &str,
        applied_by: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = now();
        sqlx::query(
            "UPDATE user_corrections
             SET status = ?, applied_by = ?, applied_at = CASE WHEN ? = 'applied' THEN ? ELSE applied_at END, error = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(applied_by)
        .bind(status)
        .bind(&now)
        .bind(error)
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn append_correction_event(
        &self,
        input: NewCorrectionEvent<'_>,
    ) -> anyhow::Result<CorrectionEventRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::append_correction_event_with_conn(&mut conn, input).await
    }

    pub async fn append_correction_event_with_conn(
        conn: &mut SqliteConnection,
        input: NewCorrectionEvent<'_>,
    ) -> anyhow::Result<CorrectionEventRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        let now = now();
        sqlx::query(
            "INSERT INTO correction_events (id, book_id, correction_id, event_type, before_json, after_json, actor, reason, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.correction_id)
        .bind(input.event_type)
        .bind(input.before_json)
        .bind(input.after_json)
        .bind(input.actor)
        .bind(input.reason)
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        Self::get_correction_event_with_conn(conn, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("correction event was not persisted"))
    }

    async fn get_correction_event_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> anyhow::Result<Option<CorrectionEventRecord>> {
        sqlx::query_as::<_, CorrectionEventRecord>(
            "SELECT id, book_id, correction_id, event_type, before_json, after_json, actor, reason, created_at
             FROM correction_events WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn list_correction_events(
        &self,
        correction_id: &str,
    ) -> anyhow::Result<Vec<CorrectionEventRecord>> {
        sqlx::query_as::<_, CorrectionEventRecord>(
            "SELECT id, book_id, correction_id, event_type, before_json, after_json, actor, reason, created_at
             FROM correction_events WHERE correction_id = ? ORDER BY created_at ASC, id ASC",
        )
        .bind(correction_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn create_audit_run(
        &self,
        input: NewAuditRun<'_>,
    ) -> anyhow::Result<QualityAuditRunRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::create_audit_run_with_conn(&mut conn, input).await
    }

    pub async fn create_audit_run_with_conn(
        conn: &mut SqliteConnection,
        input: NewAuditRun<'_>,
    ) -> anyhow::Result<QualityAuditRunRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        sqlx::query(
            "INSERT OR IGNORE INTO quality_audit_runs (id, book_id, audit_type, scope_json, status, started_at)
             VALUES (?, ?, ?, ?, ?, datetime('now'))",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.audit_type)
        .bind(input.scope_json)
        .bind(input.status)
        .execute(&mut *conn)
        .await?;
        Self::get_audit_run_with_conn(conn, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("audit run was not persisted"))
    }

    pub async fn get_audit_run(&self, id: &str) -> anyhow::Result<Option<QualityAuditRunRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_audit_run_with_conn(&mut conn, id).await
    }

    pub async fn get_audit_run_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> anyhow::Result<Option<QualityAuditRunRecord>> {
        sqlx::query_as::<_, QualityAuditRunRecord>(
            "SELECT id, book_id, audit_type, scope_json, status, started_at, finished_at, summary_json, error
             FROM quality_audit_runs WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn update_audit_run_status_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
        status: &str,
        summary_json: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE quality_audit_runs
             SET status = ?, finished_at = CASE WHEN ? IN ('completed', 'failed', 'cancelled') THEN datetime('now') ELSE finished_at END, summary_json = ?, error = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(status)
        .bind(summary_json)
        .bind(error)
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn list_audit_runs(
        &self,
        book_id: &str,
        audit_type: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<QualityAuditRunRecord>> {
        sqlx::query_as::<_, QualityAuditRunRecord>(
            "SELECT id, book_id, audit_type, scope_json, status, started_at, finished_at, summary_json, error
             FROM quality_audit_runs
             WHERE book_id = ?
               AND (? IS NULL OR audit_type = ?)
               AND (? IS NULL OR status = ?)
             ORDER BY COALESCE(started_at, '') DESC, id DESC
             LIMIT ?",
        )
        .bind(book_id)
        .bind(audit_type)
        .bind(audit_type)
        .bind(status)
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn create_audit_finding(
        &self,
        input: NewAuditFinding<'_>,
    ) -> anyhow::Result<QualityAuditFindingRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::create_audit_finding_with_conn(&mut conn, input).await
    }

    pub async fn create_audit_finding_with_conn(
        conn: &mut SqliteConnection,
        input: NewAuditFinding<'_>,
    ) -> anyhow::Result<QualityAuditFindingRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        let now = now();
        sqlx::query(
            "INSERT OR IGNORE INTO quality_audit_findings (id, book_id, audit_run_id, finding_type, severity, target_type, target_id, related_target_type, related_target_id, reason_code, reason_text, evidence_json, suggested_action, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open', ?)",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.audit_run_id)
        .bind(input.finding_type)
        .bind(input.severity)
        .bind(input.target_type)
        .bind(input.target_id)
        .bind(input.related_target_type)
        .bind(input.related_target_id)
        .bind(input.reason_code)
        .bind(input.reason_text)
        .bind(input.evidence_json)
        .bind(input.suggested_action)
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        Self::get_audit_finding_with_conn(conn, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("audit finding was not persisted"))
    }

    pub async fn get_audit_finding(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<QualityAuditFindingRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_audit_finding_with_conn(&mut conn, id).await
    }

    pub async fn get_audit_finding_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> anyhow::Result<Option<QualityAuditFindingRecord>> {
        sqlx::query_as::<_, QualityAuditFindingRecord>(
            "SELECT id, book_id, audit_run_id, finding_type, severity, target_type, target_id, related_target_type, related_target_id, reason_code, reason_text, evidence_json, suggested_action, status, created_at, resolved_at
             FROM quality_audit_findings WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn list_audit_findings(
        &self,
        book_id: &str,
        status: Option<&str>,
        finding_type: Option<&str>,
        severity: Option<&str>,
        target_type: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<QualityAuditFindingRecord>> {
        sqlx::query_as::<_, QualityAuditFindingRecord>(
            "SELECT id, book_id, audit_run_id, finding_type, severity, target_type, target_id, related_target_type, related_target_id, reason_code, reason_text, evidence_json, suggested_action, status, created_at, resolved_at
             FROM quality_audit_findings
             WHERE book_id = ?
               AND (? IS NULL OR status = ?)
               AND (? IS NULL OR finding_type = ?)
               AND (? IS NULL OR severity = ?)
               AND (? IS NULL OR target_type = ?)
             ORDER BY CASE severity WHEN 'critical' THEN 5 WHEN 'high' THEN 4 WHEN 'medium' THEN 3 WHEN 'low' THEN 2 ELSE 1 END DESC, created_at DESC
             LIMIT ?",
        )
        .bind(book_id)
        .bind(status)
        .bind(status)
        .bind(finding_type)
        .bind(finding_type)
        .bind(severity)
        .bind(severity)
        .bind(target_type)
        .bind(target_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn update_audit_finding_status_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE quality_audit_findings
             SET status = ?, resolved_at = CASE WHEN ? IN ('dismissed', 'converted_to_correction', 'resolved') THEN datetime('now') ELSE resolved_at END
             WHERE id = ?",
        )
        .bind(status)
        .bind(status)
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn insert_metric(
        &self,
        input: NewQualityMetric<'_>,
    ) -> anyhow::Result<QualityMetricRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::insert_metric_with_conn(&mut conn, input).await
    }

    pub async fn insert_metric_with_conn(
        conn: &mut SqliteConnection,
        input: NewQualityMetric<'_>,
    ) -> anyhow::Result<QualityMetricRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        let measured_at = now();
        sqlx::query(
            "INSERT OR IGNORE INTO quality_metrics (id, book_id, metric_type, metric_value, metric_json, measured_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.metric_type)
        .bind(input.metric_value)
        .bind(input.metric_json)
        .bind(&measured_at)
        .execute(&mut *conn)
        .await?;
        Self::get_metric_with_conn(conn, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("quality metric was not persisted"))
    }

    async fn get_metric_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> anyhow::Result<Option<QualityMetricRecord>> {
        sqlx::query_as::<_, QualityMetricRecord>(
            "SELECT id, book_id, metric_type, metric_value, metric_json, measured_at
             FROM quality_metrics WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn latest_metric(
        &self,
        book_id: &str,
        metric_type: &str,
    ) -> anyhow::Result<Option<QualityMetricRecord>> {
        sqlx::query_as::<_, QualityMetricRecord>(
            "SELECT id, book_id, metric_type, metric_value, metric_json, measured_at
             FROM quality_metrics WHERE book_id = ? AND metric_type = ?
             ORDER BY measured_at DESC, id DESC LIMIT 1",
        )
        .bind(book_id)
        .bind(metric_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn latest_metrics(&self, book_id: &str) -> anyhow::Result<Vec<QualityMetricRecord>> {
        sqlx::query_as::<_, QualityMetricRecord>(
            "SELECT m.id, m.book_id, m.metric_type, m.metric_value, m.metric_json, m.measured_at
             FROM quality_metrics m
             JOIN (
               SELECT metric_type, MAX(measured_at) AS measured_at
               FROM quality_metrics
               WHERE book_id = ?
               GROUP BY metric_type
             ) latest ON latest.metric_type = m.metric_type AND latest.measured_at = m.measured_at
             WHERE m.book_id = ?
             ORDER BY m.metric_type ASC, m.id DESC",
        )
        .bind(book_id)
        .bind(book_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn create_reprocess_job(
        &self,
        input: NewReprocessJob<'_>,
    ) -> anyhow::Result<ReprocessJobRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::create_reprocess_job_with_conn(&mut conn, input).await
    }

    pub async fn create_reprocess_job_with_conn(
        conn: &mut SqliteConnection,
        input: NewReprocessJob<'_>,
    ) -> anyhow::Result<ReprocessJobRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        sqlx::query(
            "INSERT OR IGNORE INTO reprocess_jobs (id, book_id, scope_type, scope_json, mode, status, requested_by, reason, dry_run, prompt_version, schema_version)
             VALUES (?, ?, ?, ?, ?, 'queued', ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.scope_type)
        .bind(input.scope_json)
        .bind(input.mode)
        .bind(input.requested_by)
        .bind(input.reason)
        .bind(if input.dry_run { 1 } else { 0 })
        .bind(input.prompt_version)
        .bind(input.schema_version)
        .execute(&mut *conn)
        .await?;
        Self::get_reprocess_job_with_conn(conn, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("reprocess job was not persisted"))
    }

    pub async fn get_reprocess_job(&self, id: &str) -> anyhow::Result<Option<ReprocessJobRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_reprocess_job_with_conn(&mut conn, id).await
    }

    pub async fn get_reprocess_job_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> anyhow::Result<Option<ReprocessJobRecord>> {
        sqlx::query_as::<_, ReprocessJobRecord>(
            "SELECT id, book_id, scope_type, scope_json, mode, status, requested_by, reason, dry_run, prompt_version, schema_version, started_at, finished_at, result_json, error
             FROM reprocess_jobs WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn update_reprocess_job_status_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
        status: &str,
        result_json: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE reprocess_jobs
             SET status = ?, started_at = CASE WHEN ? = 'running' AND started_at IS NULL THEN datetime('now') ELSE started_at END,
                 finished_at = CASE WHEN ? IN ('completed', 'failed', 'cancelled') THEN datetime('now') ELSE finished_at END,
                 result_json = ?, error = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(status)
        .bind(status)
        .bind(result_json)
        .bind(error)
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn list_reprocess_jobs(
        &self,
        book_id: &str,
        status: Option<&str>,
        mode: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<ReprocessJobRecord>> {
        sqlx::query_as::<_, ReprocessJobRecord>(
            "SELECT id, book_id, scope_type, scope_json, mode, status, requested_by, reason, dry_run, prompt_version, schema_version, started_at, finished_at, result_json, error
             FROM reprocess_jobs
             WHERE book_id = ?
               AND (? IS NULL OR status = ?)
               AND (? IS NULL OR mode = ?)
             ORDER BY COALESCE(started_at, '') DESC, id DESC
             LIMIT ?",
        )
        .bind(book_id)
        .bind(status)
        .bind(status)
        .bind(mode)
        .bind(mode)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn create_prompt_regression_run(
        &self,
        input: NewPromptRegressionRun<'_>,
    ) -> anyhow::Result<PromptRegressionRunRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::create_prompt_regression_run_with_conn(&mut conn, input).await
    }

    pub async fn create_prompt_regression_run_with_conn(
        conn: &mut SqliteConnection,
        input: NewPromptRegressionRun<'_>,
    ) -> anyhow::Result<PromptRegressionRunRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        sqlx::query(
            "INSERT OR IGNORE INTO prompt_regression_runs (id, book_id, prompt_version, schema_version, model, fixture_set, status, started_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))",
        )
        .bind(&id)
        .bind(input.book_id)
        .bind(input.prompt_version)
        .bind(input.schema_version)
        .bind(input.model)
        .bind(input.fixture_set)
        .bind(input.status)
        .execute(&mut *conn)
        .await?;
        Self::get_prompt_regression_run_with_conn(conn, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("prompt regression run was not persisted"))
    }

    pub async fn get_prompt_regression_run(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<PromptRegressionRunRecord>> {
        let mut conn = self.pool.acquire().await?;
        Self::get_prompt_regression_run_with_conn(&mut conn, id).await
    }

    pub async fn get_prompt_regression_run_with_conn(
        conn: &mut SqliteConnection,
        id: &str,
    ) -> anyhow::Result<Option<PromptRegressionRunRecord>> {
        sqlx::query_as::<_, PromptRegressionRunRecord>(
            "SELECT id, book_id, prompt_version, schema_version, model, fixture_set, status, started_at, finished_at, summary_json, error
             FROM prompt_regression_runs WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn insert_prompt_regression_result(
        &self,
        input: NewPromptRegressionResult<'_>,
    ) -> anyhow::Result<PromptRegressionResultRecord> {
        let mut conn = self.pool.acquire().await?;
        Self::insert_prompt_regression_result_with_conn(&mut conn, input).await
    }

    pub async fn insert_prompt_regression_result_with_conn(
        conn: &mut SqliteConnection,
        input: NewPromptRegressionResult<'_>,
    ) -> anyhow::Result<PromptRegressionResultRecord> {
        let id = input.id.map(str::to_string).unwrap_or_else(new_id);
        let now = now();
        sqlx::query(
            "INSERT INTO prompt_regression_results (id, run_id, case_id, case_name, domain, expected_json, actual_json, pass, diff_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(run_id, case_id) DO UPDATE SET
               case_name = excluded.case_name,
               domain = excluded.domain,
               expected_json = excluded.expected_json,
               actual_json = excluded.actual_json,
               pass = excluded.pass,
               diff_json = excluded.diff_json,
               created_at = excluded.created_at",
        )
        .bind(&id)
        .bind(input.run_id)
        .bind(input.case_id)
        .bind(input.case_name)
        .bind(input.domain)
        .bind(input.expected_json)
        .bind(input.actual_json)
        .bind(if input.pass { 1 } else { 0 })
        .bind(input.diff_json)
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        Self::get_prompt_regression_result_with_conn(conn, input.run_id, input.case_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("prompt regression result was not persisted"))
    }

    pub async fn get_prompt_regression_result_with_conn(
        conn: &mut SqliteConnection,
        run_id: &str,
        case_id: &str,
    ) -> anyhow::Result<Option<PromptRegressionResultRecord>> {
        sqlx::query_as::<_, PromptRegressionResultRecord>(
            "SELECT id, run_id, case_id, case_name, domain, expected_json, actual_json, pass, diff_json, created_at
             FROM prompt_regression_results WHERE run_id = ? AND case_id = ?",
        )
        .bind(run_id)
        .bind(case_id)
        .fetch_optional(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn list_prompt_regression_runs(
        &self,
        book_id: &str,
        fixture_set: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<PromptRegressionRunRecord>> {
        sqlx::query_as::<_, PromptRegressionRunRecord>(
            "SELECT id, book_id, prompt_version, schema_version, model, fixture_set, status, started_at, finished_at, summary_json, error
             FROM prompt_regression_runs
             WHERE book_id = ?
               AND (? IS NULL OR fixture_set = ?)
               AND (? IS NULL OR status = ?)
             ORDER BY COALESCE(started_at, '') DESC, id DESC
             LIMIT ?",
        )
        .bind(book_id)
        .bind(fixture_set)
        .bind(fixture_set)
        .bind(status)
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn list_prompt_regression_results(
        &self,
        run_id: &str,
    ) -> anyhow::Result<Vec<PromptRegressionResultRecord>> {
        sqlx::query_as::<_, PromptRegressionResultRecord>(
            "SELECT id, run_id, case_id, case_name, domain, expected_json, actual_json, pass, diff_json, created_at
             FROM prompt_regression_results WHERE run_id = ? ORDER BY case_id ASC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir =
            std::env::temp_dir().join(format!("reader-quality-repo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    async fn insert_source_claim(
        pool: &SqlitePool,
        book_id: &str,
        claim_id: &str,
        claim_type: &str,
    ) {
        let chapter_id = format!("ch-{book_id}-{claim_id}");
        let segment_id = format!("seg-{book_id}-{claim_id}");
        let span_id = format!("span-{book_id}-{claim_id}");
        let run_id = format!("run-{book_id}-{claim_id}");

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, ?, 1, 'text', ?, datetime('now'))")
            .bind(&chapter_id)
            .bind(book_id)
            .bind(format!("hash-{claim_id}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, ?, ?, ?, 0, datetime('now'))")
            .bind(&segment_id)
            .bind(book_id)
            .bind(&chapter_id)
            .bind(format!("hash-{claim_id}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, ?, ?, ?, ?, 0, 0, 10, 'text', datetime('now'))")
            .bind(&span_id)
            .bind(book_id)
            .bind(&chapter_id)
            .bind(format!("hash-{claim_id}"))
            .bind(&segment_id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, ?, ?, 'extract', 'test', 'v1', 1, ?, 'completed', datetime('now'))")
            .bind(&run_id)
            .bind(book_id)
            .bind(&chapter_id)
            .bind(format!("input-{claim_id}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, ?, 1, ?, 'test', ?, ?, 0.4, 'high', 'quarantined', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .bind(book_id)
            .bind(claim_type)
            .bind(&span_id)
            .bind(&run_id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn quarantine_upsert_is_idempotent_and_filterable() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1", "relationship_update").await;
        let repo = QualityRepo::new(pool.clone());

        let first = repo
            .upsert_quarantined_claim(NewQuarantinedClaim {
                id: Some("q1"),
                book_id: "b1",
                claim_id: "c1",
                reason_code: "low_confidence",
                reason_text: Some("weak evidence"),
                suggested_action: "needs_manual_review",
                priority: 3,
            })
            .await
            .unwrap();
        let second = repo
            .upsert_quarantined_claim(NewQuarantinedClaim {
                id: Some("q2"),
                book_id: "b1",
                claim_id: "c1",
                reason_code: "relationship_pollution",
                reason_text: Some("looks like location"),
                suggested_action: "create_correction",
                priority: 8,
            })
            .await
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.reason_code, "relationship_pollution");
        assert_eq!(second.priority, 8);

        let listed = repo
            .list_quarantined_claims(
                "b1",
                Some("open"),
                Some("relationship_pollution"),
                Some("relationship_update"),
                10,
            )
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].claim_id, "c1");
    }

    #[tokio::test]
    async fn correction_events_are_append_only_and_listed_in_order() {
        let pool = setup_test_db().await;
        insert_source_claim(&pool, "b1", "c1", "property_update").await;
        let repo = QualityRepo::new(pool);
        let correction = repo
            .create_user_correction(NewUserCorrection {
                id: Some("uc1"),
                book_id: "b1",
                target_type: "claim",
                target_id: "c1",
                correction_type: "reject_claim",
                correction_json: "{}",
                source: "user",
                source_claim_id: Some("c1"),
                source_span_id: Some("span-b1-c1"),
                created_by: "test",
            })
            .await
            .unwrap();

        repo.append_correction_event(NewCorrectionEvent {
            id: Some("event1"),
            book_id: "b1",
            correction_id: &correction.id,
            event_type: "proposed",
            before_json: None,
            after_json: Some("{}"),
            actor: "test",
            reason: Some("created"),
        })
        .await
        .unwrap();
        repo.append_correction_event(NewCorrectionEvent {
            id: Some("event2"),
            book_id: "b1",
            correction_id: &correction.id,
            event_type: "validated",
            before_json: Some("{}"),
            after_json: Some("{}"),
            actor: "test",
            reason: Some("checked"),
        })
        .await
        .unwrap();

        let events = repo.list_correction_events(&correction.id).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "proposed");
        assert_eq!(events[1].event_type, "validated");

        let corrections = repo
            .list_user_corrections(
                "b1",
                Some("proposed"),
                Some("reject_claim"),
                Some("claim"),
                Some("c1"),
                10,
            )
            .await
            .unwrap();
        assert_eq!(corrections.len(), 1);
    }

    #[tokio::test]
    async fn audit_run_finding_and_reprocess_job_roundtrip() {
        let pool = setup_test_db().await;
        let repo = QualityRepo::new(pool);
        let run = repo
            .create_audit_run(NewAuditRun {
                id: Some("audit1"),
                book_id: "b1",
                audit_type: "duplicate_entities",
                scope_json: "{}",
                status: "running",
            })
            .await
            .unwrap();
        let finding = repo
            .create_audit_finding(NewAuditFinding {
                id: Some("finding1"),
                book_id: "b1",
                audit_run_id: &run.id,
                finding_type: "duplicate_entity_candidate",
                severity: "medium",
                target_type: "entity",
                target_id: "e1",
                related_target_type: Some("entity"),
                related_target_id: Some("e2"),
                reason_code: "same_name",
                reason_text: Some("same canonical name"),
                evidence_json: Some("{}"),
                suggested_action: "merge_entities",
            })
            .await
            .unwrap();
        let job = repo
            .create_reprocess_job(NewReprocessJob {
                id: Some("job1"),
                book_id: "b1",
                scope_type: "claim",
                scope_json: "{\"claimId\":\"c1\"}",
                mode: "retry_failed",
                requested_by: "test",
                reason: Some("retry claim"),
                dry_run: true,
                prompt_version: Some("v1"),
                schema_version: Some("v1"),
            })
            .await
            .unwrap();

        assert_eq!(finding.audit_run_id, run.id);
        assert_eq!(job.status, "queued");
        assert_eq!(job.dry_run, 1);

        let findings = repo
            .list_audit_findings(
                "b1",
                Some("open"),
                Some("duplicate_entity_candidate"),
                None,
                Some("entity"),
                10,
            )
            .await
            .unwrap();
        assert_eq!(findings.len(), 1);

        let jobs = repo
            .list_reprocess_jobs("b1", Some("queued"), Some("retry_failed"), 10)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[tokio::test]
    async fn latest_metrics_return_newest_per_metric_type() {
        let pool = setup_test_db().await;
        let repo = QualityRepo::new(pool);
        repo.insert_metric(NewQualityMetric {
            id: Some("m1"),
            book_id: "b1",
            metric_type: "quarantined_claim_count",
            metric_value: 1.0,
            metric_json: Some("{\"count\":1}"),
        })
        .await
        .unwrap();
        repo.insert_metric(NewQualityMetric {
            id: Some("m2"),
            book_id: "b1",
            metric_type: "quarantined_claim_count",
            metric_value: 2.0,
            metric_json: Some("{\"count\":2}"),
        })
        .await
        .unwrap();
        repo.insert_metric(NewQualityMetric {
            id: Some("m3"),
            book_id: "b1",
            metric_type: "failed_reprocess_count",
            metric_value: 3.0,
            metric_json: None,
        })
        .await
        .unwrap();

        let latest = repo
            .latest_metric("b1", "quarantined_claim_count")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, "m2");

        let all_latest = repo.latest_metrics("b1").await.unwrap();
        assert_eq!(all_latest.len(), 2);
    }

    #[tokio::test]
    async fn prompt_regression_result_upsert_is_idempotent_by_run_and_case() {
        let pool = setup_test_db().await;
        let repo = QualityRepo::new(pool);
        let run = repo
            .create_prompt_regression_run(NewPromptRegressionRun {
                id: Some("run1"),
                book_id: "b1",
                prompt_version: "v1",
                schema_version: "v1",
                model: "test",
                fixture_set: "core",
                status: "running",
            })
            .await
            .unwrap();

        let first = repo
            .insert_prompt_regression_result(NewPromptRegressionResult {
                id: Some("result1"),
                run_id: &run.id,
                case_id: "case1",
                case_name: "Case 1",
                domain: "phase1",
                expected_json: "{}",
                actual_json: "{}",
                pass: false,
                diff_json: Some("{\"failed\":true}"),
            })
            .await
            .unwrap();
        let second = repo
            .insert_prompt_regression_result(NewPromptRegressionResult {
                id: Some("result2"),
                run_id: &run.id,
                case_id: "case1",
                case_name: "Case 1 updated",
                domain: "phase1",
                expected_json: "{}",
                actual_json: "{\"ok\":true}",
                pass: true,
                diff_json: None,
            })
            .await
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.pass, 1);
        let results = repo.list_prompt_regression_results(&run.id).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].case_name, "Case 1 updated");
    }

    #[tokio::test]
    async fn with_conn_writes_roll_back_with_transaction() {
        let pool = setup_test_db().await;
        let repo = QualityRepo::new(pool.clone());
        let mut tx = pool.begin().await.unwrap();
        QualityRepo::insert_metric_with_conn(
            &mut tx,
            NewQualityMetric {
                id: Some("m1"),
                book_id: "b1",
                metric_type: "quarantined_claim_count",
                metric_value: 1.0,
                metric_json: None,
            },
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();

        assert!(repo
            .latest_metric("b1", "quarantined_claim_count")
            .await
            .unwrap()
            .is_none());
    }
}
