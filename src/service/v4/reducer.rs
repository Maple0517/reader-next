use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::identity_repo::IdentityRepo;
use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
use crate::storage::db::v4::property_repo::PropertyRepo;
use crate::storage::db::v4::relationship_repo::{
    RelationshipEventRepo, RelationshipRecord, RelationshipRepo,
};
use sqlx::{SqliteConnection, SqlitePool};

/// Result of reducing claims into canonical state.
#[derive(Debug, Default)]
pub struct ReductionResult {
    pub entities_created: Vec<String>,
    pub entities_updated: Vec<String>,
    pub aliases_created: Vec<String>,
    pub properties_inserted: Vec<String>,
    pub properties_superseded: Vec<String>,
    pub current_properties_updated: Vec<String>,
    pub claims_accepted: Vec<String>,
    pub claims_skipped: Vec<String>,
}

/// Result of reducing judge-approved knowledge claims into canonical knowledge tables.
#[derive(Debug, Default)]
pub struct KnowledgeReductionResult {
    pub cards_created_or_reused: Vec<String>,
    pub assertions_inserted: Vec<String>,
    pub assertions_revised: Vec<String>,
    pub assertions_contradicted: Vec<String>,
    pub links_inserted: Vec<String>,
    pub claims_accepted: Vec<String>,
    pub claims_skipped: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeWriteProvenance {
    pub claim_id: String,
    pub evidence_span_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeWriteEntityRef {
    pub entity_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeWriteCardAction {
    CreateNewCard,
    UseExistingCard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeWriteDecision {
    AddNew,
    Supplement,
    ReviseExisting,
    ContradictExisting,
    MarkRumor,
    MarkUncertain,
    MarkFalseInWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeAssertionStatus {
    Active,
    Rumor,
    Uncertain,
    FalseInWorld,
}

impl KnowledgeAssertionStatus {
    pub fn from_write_status(value: &str) -> anyhow::Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "rumor" => Ok(Self::Rumor),
            "uncertain" => Ok(Self::Uncertain),
            "false_in_world" => Ok(Self::FalseInWorld),
            other => anyhow::bail!("invalid knowledge assertion write status: {other}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Rumor => "rumor",
            Self::Uncertain => "uncertain",
            Self::FalseInWorld => "false_in_world",
        }
    }
}

#[derive(Debug, Clone)]
pub struct KnowledgeWriteCommand {
    pub book_id: String,
    pub category: String,
    pub topic_display: String,
    pub assertion_text: String,
    pub importance_score: f64,
    pub referenced_entities: Vec<KnowledgeWriteEntityRef>,
    pub card_action: KnowledgeWriteCardAction,
    pub target_card_id: Option<String>,
    pub decision: KnowledgeWriteDecision,
    pub affected_assertion_ids: Vec<String>,
    pub assertion_status: KnowledgeAssertionStatus,
    pub current_summary: Option<String>,
    pub confidence: f64,
    pub chapter_index: i64,
    pub provenance: KnowledgeWriteProvenance,
}

#[derive(Debug, Clone)]
pub struct KnowledgeAssertionStatusWriteCommand {
    pub book_id: String,
    pub assertion_id: String,
    pub status: KnowledgeAssertionStatus,
}

pub async fn apply_knowledge_write(
    command: KnowledgeWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<KnowledgeReductionResult> {
    let mut result = KnowledgeReductionResult::default();
    let mut tx = pool.begin().await?;
    apply_knowledge_write_with_conn(&command, &mut *tx, &mut result).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn apply_knowledge_assertion_status_write(
    command: KnowledgeAssertionStatusWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<KnowledgeReductionResult> {
    let mut result = KnowledgeReductionResult::default();
    let mut tx = pool.begin().await?;
    apply_knowledge_assertion_status_write_with_conn(&command, &mut *tx, &mut result).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn apply_knowledge_assertion_status_write_with_conn(
    command: &KnowledgeAssertionStatusWriteCommand,
    conn: &mut SqliteConnection,
    result: &mut KnowledgeReductionResult,
) -> anyhow::Result<()> {
    let update_result = sqlx::query(
        "UPDATE knowledge_assertions SET status = ?, updated_at = ? WHERE book_id = ? AND id = ?",
    )
    .bind(command.status.as_str())
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(&command.book_id)
    .bind(&command.assertion_id)
    .execute(&mut *conn)
    .await?;
    if update_result.rows_affected() == 0 {
        anyhow::bail!("knowledge assertion not found for status write");
    }
    invalidate_knowledge_cache_with_conn(conn, &command.book_id).await?;
    result.assertions_revised.push(command.assertion_id.clone());
    Ok(())
}

async fn apply_knowledge_write_with_conn(
    command: &KnowledgeWriteCommand,
    conn: &mut SqliteConnection,
    result: &mut KnowledgeReductionResult,
) -> anyhow::Result<()> {
    let card = match command.card_action {
        KnowledgeWriteCardAction::CreateNewCard => {
            KnowledgeRepo::find_or_create_card_with_conn(
                conn,
                &command.book_id,
                &command.category,
                &command.topic_display,
                &command.topic_display,
                active_summary_for_knowledge_command(command),
                command.confidence,
                command.importance_score,
                command.chapter_index,
            )
            .await?
        }
        KnowledgeWriteCardAction::UseExistingCard => {
            let card_id = command
                .target_card_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("use_existing_card missing target_card_id"))?;
            get_knowledge_card_by_id_with_conn(conn, card_id).await?
        }
    };
    if card.book_id != command.book_id {
        anyhow::bail!(
            "knowledge card {} belongs to book {}, not {}",
            card.id,
            card.book_id,
            command.book_id
        );
    }
    result.cards_created_or_reused.push(card.id.clone());

    let assertion = KnowledgeRepo::find_or_create_assertion_with_conn(
        conn,
        &command.book_id,
        &card.id,
        &command.provenance.claim_id,
        &command.assertion_text,
        command.assertion_status.as_str(),
        command.confidence,
        command.importance_score,
        command.chapter_index,
    )
    .await?;
    result.assertions_inserted.push(assertion.id.clone());

    for entity_ref in &command.referenced_entities {
        KnowledgeRepo::insert_assertion_entity_with_conn(
            conn,
            &command.book_id,
            &assertion.id,
            &entity_ref.entity_id,
            &entity_ref.role,
        )
        .await?;
    }

    apply_knowledge_revision_links_from_command(conn, &card.id, &assertion.id, command, result)
        .await?;

    if let Some(summary) = active_summary_for_knowledge_command(command) {
        update_knowledge_card_summary_with_conn(
            conn,
            &card.id,
            summary,
            command.confidence,
            command.importance_score,
            command.chapter_index,
        )
        .await?;
    }
    invalidate_knowledge_cache_with_conn(conn, &command.book_id).await?;
    result
        .claims_accepted
        .push(command.provenance.claim_id.clone());

    Ok(())
}

fn active_summary_for_knowledge_command(command: &KnowledgeWriteCommand) -> Option<&str> {
    if command.assertion_status == KnowledgeAssertionStatus::Active {
        command
            .current_summary
            .as_deref()
            .filter(|summary| !summary.trim().is_empty())
    } else {
        None
    }
}

async fn apply_knowledge_revision_links_from_command(
    conn: &mut SqliteConnection,
    card_id: &str,
    new_assertion_id: &str,
    command: &KnowledgeWriteCommand,
    result: &mut KnowledgeReductionResult,
) -> anyhow::Result<()> {
    let (old_status, link_type, target_vec) = match command.decision {
        KnowledgeWriteDecision::ReviseExisting => (
            "revised",
            "supersedes",
            Some(&mut result.assertions_revised),
        ),
        KnowledgeWriteDecision::ContradictExisting => (
            "contradicted",
            "contradicts",
            Some(&mut result.assertions_contradicted),
        ),
        _ => ("", "", None),
    };
    let Some(target_vec) = target_vec else {
        return Ok(());
    };

    for old_assertion_id in &command.affected_assertion_ids {
        validate_knowledge_assertion_same_card(conn, &command.book_id, card_id, old_assertion_id)
            .await?;
        KnowledgeRepo::update_assertion_status_with_conn(conn, old_assertion_id, old_status)
            .await?;
        target_vec.push(old_assertion_id.clone());
        let link = KnowledgeRepo::insert_assertion_link_with_conn(
            conn,
            &command.book_id,
            new_assertion_id,
            old_assertion_id,
            link_type,
        )
        .await?;
        result.links_inserted.push(link.id);
    }
    Ok(())
}

async fn validate_knowledge_assertion_same_card(
    conn: &mut SqliteConnection,
    book_id: &str,
    card_id: &str,
    assertion_id: &str,
) -> anyhow::Result<()> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT book_id, card_id FROM knowledge_assertions WHERE id = ?")
            .bind(assertion_id)
            .fetch_optional(&mut *conn)
            .await?;
    let Some((assertion_book_id, assertion_card_id)) = row else {
        anyhow::bail!("affected knowledge assertion not found: {}", assertion_id);
    };
    if assertion_book_id != book_id || assertion_card_id != card_id {
        anyhow::bail!("affected knowledge assertion must belong to same book and card");
    }
    Ok(())
}

async fn get_knowledge_card_by_id_with_conn(
    conn: &mut SqliteConnection,
    card_id: &str,
) -> anyhow::Result<crate::storage::db::v4::knowledge_repo::KnowledgeCardRecord> {
    sqlx::query_as::<_, crate::storage::db::v4::knowledge_repo::KnowledgeCardRecord>(
        "SELECT id, book_id, category, topic_key, topic_display, current_summary, confidence, importance_score, first_seen_chapter, last_updated_chapter, status, created_at, updated_at
         FROM knowledge_cards WHERE id = ?",
    )
    .bind(card_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("knowledge card not found: {}", card_id))
}

async fn update_knowledge_card_summary_with_conn(
    conn: &mut SqliteConnection,
    card_id: &str,
    summary: &str,
    confidence: f64,
    importance_score: f64,
    chapter_index: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE knowledge_cards
         SET current_summary = ?, confidence = ?, importance_score = ?, last_updated_chapter = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(summary.trim())
    .bind(confidence)
    .bind(importance_score)
    .bind(chapter_index)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(card_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn invalidate_knowledge_cache_with_conn(
    conn: &mut SqliteConnection,
    book_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM view_model_cache
         WHERE book_id = ? AND view_type IN ('knowledge', 'knowledge_card', 'knowledge_list')",
    )
    .bind(book_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod knowledge_reducer_tests {
    use super::{
        apply_knowledge_write, KnowledgeAssertionStatus, KnowledgeWriteCardAction,
        KnowledgeWriteCommand, KnowledgeWriteDecision, KnowledgeWriteProvenance,
    };
    use crate::service::v4::knowledge_judge::{
        KnowledgeCardAction, KnowledgeJudgeDecision, KnowledgeJudgeOutput,
    };
    use crate::storage::db;
    use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
    use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, ClaimRepo, KnowledgeRepo, String, String) {
        let dir =
            std::env::temp_dir().join(format!("reader-knowledge-reducer-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('chapter1', 'b1', 1, 'text', 'hash', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('segment1', 'b1', 'chapter1', 'hash', 0, datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span1', 'b1', 'chapter1', 'hash', 'segment1', 0, 0, 10, 'evidence', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run1', 'b1', 'chapter1', 'extract', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();

        (
            pool.clone(),
            ClaimRepo::new(pool.clone()),
            KnowledgeRepo::new(pool),
            "span1".to_string(),
            "run1".to_string(),
        )
    }

    async fn create_knowledge_claim(
        claim_repo: &ClaimRepo,
        span_id: &str,
        run_id: &str,
        topic: &str,
        assertion: &str,
    ) -> ClaimRecord {
        let value_json = serde_json::json!({
            "category": "power_system",
            "raw_topic": topic,
            "topic_display": topic,
            "assertion_text": assertion,
            "importance_score": 0.8,
            "referenced_entity_mentions": [],
            "status_hint": "fact",
            "reason_hint": "test"
        });
        claim_repo
            .create_claim(
                "b1",
                1,
                "knowledge_assertion",
                None,
                None,
                None,
                None,
                "knowledge",
                Some(assertion),
                Some(&value_json.to_string()),
                span_id,
                run_id,
                0.9,
                "high",
            )
            .await
            .unwrap()
    }

    fn output(
        decision: KnowledgeJudgeDecision,
        card_action: KnowledgeCardAction,
        target_card_id: Option<&str>,
        affected_assertion_ids: Vec<String>,
        assertion_status: &str,
        summary: Option<&str>,
    ) -> KnowledgeJudgeOutput {
        KnowledgeJudgeOutput {
            decision,
            card_action,
            target_card_id: target_card_id.map(str::to_string),
            affected_assertion_ids,
            assertion_status: assertion_status.to_string(),
            current_summary: summary.map(str::to_string),
            confidence: 0.9,
            reason_code: "test".to_string(),
            explanation_for_log: "test".to_string(),
        }
    }

    fn knowledge_write_command_from_output(
        claim: &ClaimRecord,
        output: KnowledgeJudgeOutput,
    ) -> KnowledgeWriteCommand {
        let value: serde_json::Value =
            serde_json::from_str(claim.value_json.as_deref().expect("test value_json")).unwrap();
        KnowledgeWriteCommand {
            book_id: claim.book_id.clone(),
            category: value["category"].as_str().unwrap().to_string(),
            topic_display: value["topic_display"].as_str().unwrap().to_string(),
            assertion_text: value["assertion_text"].as_str().unwrap().to_string(),
            importance_score: value["importance_score"].as_f64().unwrap_or(0.5),
            referenced_entities: vec![],
            card_action: match output.card_action {
                KnowledgeCardAction::CreateNewCard => KnowledgeWriteCardAction::CreateNewCard,
                KnowledgeCardAction::UseExistingCard => KnowledgeWriteCardAction::UseExistingCard,
                KnowledgeCardAction::Uncertain => panic!("no-write output cannot become command"),
            },
            target_card_id: output.target_card_id,
            decision: match output.decision {
                KnowledgeJudgeDecision::AddNew => KnowledgeWriteDecision::AddNew,
                KnowledgeJudgeDecision::Supplement => KnowledgeWriteDecision::Supplement,
                KnowledgeJudgeDecision::ReviseExisting => KnowledgeWriteDecision::ReviseExisting,
                KnowledgeJudgeDecision::ContradictExisting => {
                    KnowledgeWriteDecision::ContradictExisting
                }
                KnowledgeJudgeDecision::MarkRumor => KnowledgeWriteDecision::MarkRumor,
                KnowledgeJudgeDecision::MarkUncertain => KnowledgeWriteDecision::MarkUncertain,
                KnowledgeJudgeDecision::MarkFalseInWorld => {
                    KnowledgeWriteDecision::MarkFalseInWorld
                }
                KnowledgeJudgeDecision::Reject => panic!("reject output cannot become command"),
            },
            affected_assertion_ids: output.affected_assertion_ids,
            assertion_status: KnowledgeAssertionStatus::from_write_status(&output.assertion_status)
                .unwrap(),
            current_summary: output.current_summary,
            confidence: output.confidence,
            chapter_index: claim.chapter_index,
            provenance: KnowledgeWriteProvenance {
                claim_id: claim.id.clone(),
                evidence_span_ids: vec![claim.primary_source_span_id.clone()],
            },
        }
    }

    #[test]
    fn knowledge_write_command_uses_typed_assertion_status_contract() {
        let source = include_str!("reducer.rs");
        let raw_string_field = concat!("pub assertion", "_status: String");
        assert!(
            !source.contains(raw_string_field),
            "Knowledge reducer command must carry a typed assertion status, not a raw string"
        );
        assert!(
            source.contains("enum KnowledgeAssertionStatus"),
            "Knowledge reducer must define the typed assertion status enum it consumes"
        );
    }

    #[tokio::test]
    async fn knowledge_reducer_accepts_typed_command_without_claim_value_json() {
        let (pool, claim_repo, knowledge_repo, span_id, run_id) = setup().await;
        let ledger_claim = claim_repo
            .create_claim(
                "b1",
                1,
                "knowledge_assertion",
                None,
                None,
                None,
                None,
                "knowledge ledger provenance only",
                Some("Cultivation has stable realm tiers."),
                None,
                &span_id,
                &run_id,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let command = KnowledgeWriteCommand {
            book_id: "b1".to_string(),
            category: "power_system".to_string(),
            topic_display: "Cultivation Realms".to_string(),
            assertion_text: "Cultivation has stable realm tiers.".to_string(),
            importance_score: 0.8,
            referenced_entities: vec![],
            card_action: KnowledgeWriteCardAction::CreateNewCard,
            target_card_id: None,
            decision: KnowledgeWriteDecision::AddNew,
            affected_assertion_ids: Vec::new(),
            assertion_status: KnowledgeAssertionStatus::Active,
            current_summary: Some("Cultivation realm summary".to_string()),
            confidence: 0.9,
            chapter_index: 1,
            provenance: KnowledgeWriteProvenance {
                claim_id: ledger_claim.id,
                evidence_span_ids: vec![span_id],
            },
        };

        let result = apply_knowledge_write(command, &pool).await.unwrap();

        assert_eq!(result.cards_created_or_reused.len(), 1);
        assert_eq!(result.assertions_inserted.len(), 1);
        let card = knowledge_repo
            .list_cards("b1", None, None)
            .await
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            card.current_summary.as_deref(),
            Some("Cultivation realm summary")
        );
    }

    #[tokio::test]
    async fn new_topic_creates_card_assertion_and_accepts_claim() {
        let (pool, claim_repo, knowledge_repo, span_id, run_id) = setup().await;
        let claim = create_knowledge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "Cultivation Realms",
            "Cultivation has stable realm tiers.",
        )
        .await;
        let command = knowledge_write_command_from_output(
            &claim,
            output(
                KnowledgeJudgeDecision::AddNew,
                KnowledgeCardAction::CreateNewCard,
                None,
                Vec::new(),
                "active",
                Some("Cultivation realm summary"),
            ),
        );

        let result = apply_knowledge_write(command, &pool).await.unwrap();

        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
        assert_eq!(
            knowledge_repo
                .list_cards("b1", None, None)
                .await
                .unwrap()
                .len(),
            1
        );
        let card = knowledge_repo
            .list_cards("b1", None, None)
            .await
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            card.current_summary.as_deref(),
            Some("Cultivation realm summary")
        );
        assert_eq!(
            knowledge_repo
                .list_assertions_for_card(&card.id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
    }

    #[tokio::test]
    async fn supplement_reuses_card_and_keeps_old_assertion_active() {
        let (pool, claim_repo, knowledge_repo, span_id, run_id) = setup().await;
        let old_claim = create_knowledge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "Cultivation Realms",
            "Old assertion.",
        )
        .await;
        let card = knowledge_repo
            .find_or_create_card(
                "b1",
                "power_system",
                "cultivation-realms",
                "Cultivation Realms",
                Some("Old summary"),
                0.8,
                0.8,
                1,
            )
            .await
            .unwrap();
        let old_assertion = knowledge_repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                &old_claim.id,
                "Old assertion.",
                "active",
                0.8,
                0.8,
                1,
            )
            .await
            .unwrap();
        let new_claim = create_knowledge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "Cultivation Realms",
            "Supplemental assertion.",
        )
        .await;
        let command = knowledge_write_command_from_output(
            &new_claim,
            output(
                KnowledgeJudgeDecision::Supplement,
                KnowledgeCardAction::UseExistingCard,
                Some(&card.id),
                Vec::new(),
                "active",
                Some("Updated factual summary"),
            ),
        );

        apply_knowledge_write(command, &pool).await.unwrap();

        let old_after = knowledge_repo
            .get_assertion_by_id(&old_assertion.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(old_after.status, "active");
        assert_eq!(
            knowledge_repo
                .get_card_by_id(&card.id)
                .await
                .unwrap()
                .unwrap()
                .current_summary
                .as_deref(),
            Some("Updated factual summary")
        );
    }

    #[tokio::test]
    async fn revise_and_contradict_update_old_assertions_and_insert_links() {
        let (pool, claim_repo, knowledge_repo, span_id, run_id) = setup().await;
        let old_claim = create_knowledge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "Cultivation Realms",
            "Old assertion.",
        )
        .await;
        let card = knowledge_repo
            .find_or_create_card(
                "b1",
                "power_system",
                "cultivation-realms",
                "Cultivation Realms",
                Some("Old summary"),
                0.8,
                0.8,
                1,
            )
            .await
            .unwrap();
        let old_assertion = knowledge_repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                &old_claim.id,
                "Old assertion.",
                "active",
                0.8,
                0.8,
                1,
            )
            .await
            .unwrap();

        for (decision, expected_status, link_type, text) in [
            (
                KnowledgeJudgeDecision::ReviseExisting,
                "revised",
                "supersedes",
                "Revision assertion.",
            ),
            (
                KnowledgeJudgeDecision::ContradictExisting,
                "contradicted",
                "contradicts",
                "Contradiction assertion.",
            ),
        ] {
            let claim =
                create_knowledge_claim(&claim_repo, &span_id, &run_id, "Cultivation Realms", text)
                    .await;
            let command = knowledge_write_command_from_output(
                &claim,
                output(
                    decision,
                    KnowledgeCardAction::UseExistingCard,
                    Some(&card.id),
                    vec![old_assertion.id.clone()],
                    "active",
                    Some("Revised summary"),
                ),
            );

            let result = apply_knowledge_write(command, &pool).await.unwrap();

            assert_eq!(result.links_inserted.len(), 1);
            assert_eq!(
                knowledge_repo
                    .get_assertion_by_id(&old_assertion.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .status,
                expected_status
            );
            assert_eq!(
                knowledge_repo
                    .list_links_from("b1", &result.assertions_inserted[0], Some(link_type))
                    .await
                    .unwrap()
                    .len(),
                1
            );
        }
    }

    #[tokio::test]
    async fn non_factual_statuses_do_not_overwrite_summary() {
        let (pool, claim_repo, knowledge_repo, span_id, run_id) = setup().await;
        let card = knowledge_repo
            .find_or_create_card(
                "b1",
                "power_system",
                "cultivation-realms",
                "Cultivation Realms",
                Some("Factual summary"),
                0.8,
                0.8,
                1,
            )
            .await
            .unwrap();

        for (decision, status) in [
            (KnowledgeJudgeDecision::MarkRumor, "rumor"),
            (KnowledgeJudgeDecision::MarkUncertain, "uncertain"),
            (KnowledgeJudgeDecision::MarkFalseInWorld, "false_in_world"),
        ] {
            let claim = create_knowledge_claim(
                &claim_repo,
                &span_id,
                &run_id,
                "Cultivation Realms",
                "Non factual assertion.",
            )
            .await;
            let command = knowledge_write_command_from_output(
                &claim,
                output(
                    decision,
                    KnowledgeCardAction::UseExistingCard,
                    Some(&card.id),
                    Vec::new(),
                    status,
                    Some("Should not replace"),
                ),
            );

            apply_knowledge_write(command, &pool).await.unwrap();
            assert_eq!(
                knowledge_repo
                    .get_card_by_id(&card.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .current_summary
                    .as_deref(),
                Some("Factual summary")
            );
        }
    }

    #[tokio::test]
    async fn reducer_rolls_back_on_failure() {
        let (pool, claim_repo, knowledge_repo, span_id, run_id) = setup().await;
        let claim = create_knowledge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "Cultivation Realms",
            "Assertion.",
        )
        .await;
        let command = knowledge_write_command_from_output(
            &claim,
            output(
                KnowledgeJudgeDecision::Supplement,
                KnowledgeCardAction::UseExistingCard,
                Some("missing-card"),
                Vec::new(),
                "active",
                Some("Summary"),
            ),
        );

        assert!(apply_knowledge_write(command, &pool).await.is_err());
        assert!(knowledge_repo
            .list_cards("b1", None, None)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "proposed"
        );
    }

    #[tokio::test]
    async fn repeated_processing_is_idempotent() {
        let (pool, claim_repo, knowledge_repo, span_id, run_id) = setup().await;
        let claim = create_knowledge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "Cultivation Realms",
            "Cultivation has stable realm tiers.",
        )
        .await;
        let command = knowledge_write_command_from_output(
            &claim,
            output(
                KnowledgeJudgeDecision::AddNew,
                KnowledgeCardAction::CreateNewCard,
                None,
                Vec::new(),
                "active",
                Some("Cultivation realm summary"),
            ),
        );

        apply_knowledge_write(command.clone(), &pool).await.unwrap();
        apply_knowledge_write(command, &pool).await.unwrap();

        let card = knowledge_repo
            .list_cards("b1", None, None)
            .await
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            knowledge_repo
                .list_assertions_for_card(&card.id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn knowledge_reducer_invalidates_knowledge_cache() {
        let (pool, claim_repo, _knowledge_repo, span_id, run_id) = setup().await;
        crate::storage::db::v4::cache_repo::CacheRepo::new(pool.clone())
            .set_cached("b1", "knowledge", "__book__", 10, "{}")
            .await
            .unwrap();
        let claim = create_knowledge_claim(
            &claim_repo,
            &span_id,
            &run_id,
            "Cultivation Realms",
            "Cultivation has stable realm tiers.",
        )
        .await;
        let command = knowledge_write_command_from_output(
            &claim,
            output(
                KnowledgeJudgeDecision::AddNew,
                KnowledgeCardAction::CreateNewCard,
                None,
                Vec::new(),
                "active",
                Some("Cultivation realm summary"),
            ),
        );

        apply_knowledge_write(command, &pool).await.unwrap();

        let cached = crate::storage::db::v4::cache_repo::CacheRepo::new(pool)
            .get_cached("b1", "knowledge", "__book__", 10)
            .await
            .unwrap();
        assert!(cached.is_none());
    }
}

/// Reduce claims into canonical state within a transaction.
///
/// Rules:
/// - Only processes claims with status='proposed' and risk_level in ('low', 'medium')
/// - Character/profile claims are owned by `character_processor` -> `apply_character_write`
/// - minor_event: skip (ledger-only)
/// - All claims in the batch commit/rollback together
/// - Successful claims marked as 'accepted'
pub async fn reduce_claims(
    claims: &[ClaimRecord],
    _book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<ReductionResult> {
    let mut result = ReductionResult::default();

    // Filter to only proposed, low/medium risk claims
    let reducible: Vec<&ClaimRecord> = claims
        .iter()
        .filter(|c| c.status == "proposed" && (c.risk_level == "low" || c.risk_level == "medium"))
        .collect();

    if reducible.is_empty() {
        return Ok(result);
    }

    // Start transaction - all operations use the same connection for atomicity
    let mut tx = pool.begin().await?;

    for claim in &reducible {
        match claim.claim_type.as_str() {
            "entity_introduction" | "alias" | "property_update" => {
                result.claims_skipped.push(claim.id.clone());
            }
            // minor_event and unknown types: ledger-only, intentionally not reduced.
            // proposed status does not mean pending canonical work.
            // These claims are preserved in the claims table for audit
            // but do not block processing_progress or trigger retry.
            _ => {
                result.claims_skipped.push(claim.id.clone());
            }
        }
    }

    // Mark only processed claims (not skipped) as accepted
    let accepted_ids: Vec<String> = reducible
        .iter()
        .filter(|c| !result.claims_skipped.contains(&c.id))
        .map(|c| c.id.clone())
        .collect();
    if !accepted_ids.is_empty() {
        ClaimRepo::batch_update_claim_status_with_conn(&mut *tx, &accepted_ids, "accepted").await?;
    }
    result.claims_accepted = accepted_ids;

    // Commit transaction - all or nothing
    tx.commit().await?;

    Ok(result)
}

// --- Character Reducer ---

#[derive(Debug, Clone)]
pub struct CharacterWriteProvenance {
    pub claim_id: String,
    pub evidence_span_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum CharacterWriteCommand {
    IntroduceEntity {
        book_id: String,
        entity_type: String,
        canonical_name: String,
        display_name: String,
        short_summary: Option<String>,
        aliases: Vec<String>,
        chapter_index: i64,
        confidence: f64,
        provenance: CharacterWriteProvenance,
    },
    AddAlias {
        book_id: String,
        entity_id: String,
        alias: String,
        alias_type: String,
        chapter_index: i64,
        confidence: f64,
        provenance: CharacterWriteProvenance,
    },
    UpdateProperty {
        book_id: String,
        entity_id: String,
        dimension_key: String,
        value_text: Option<String>,
        value_json: Option<serde_json::Value>,
        chapter_index: i64,
        confidence: f64,
        provenance: CharacterWriteProvenance,
    },
}

pub async fn apply_character_write(
    command: CharacterWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<ReductionResult> {
    let mut result = ReductionResult::default();
    let mut tx = pool.begin().await?;

    match command {
        CharacterWriteCommand::IntroduceEntity {
            book_id,
            entity_type,
            canonical_name,
            display_name,
            short_summary,
            aliases,
            chapter_index,
            confidence,
            provenance,
        } => {
            apply_character_introduce_entity(
                &book_id,
                &entity_type,
                &canonical_name,
                &display_name,
                short_summary.as_deref(),
                aliases,
                chapter_index,
                confidence,
                &provenance,
                &mut *tx,
                &mut result,
            )
            .await?;
        }
        CharacterWriteCommand::AddAlias {
            book_id,
            entity_id,
            alias,
            alias_type,
            chapter_index,
            confidence,
            provenance,
        } => {
            apply_character_add_alias(
                &book_id,
                &entity_id,
                &alias,
                &alias_type,
                chapter_index,
                confidence,
                &provenance,
                &mut *tx,
                &mut result,
            )
            .await?;
        }
        CharacterWriteCommand::UpdateProperty {
            book_id,
            entity_id,
            dimension_key,
            value_text,
            value_json,
            chapter_index,
            confidence,
            provenance,
        } => {
            apply_character_update_property(
                &book_id,
                &entity_id,
                &dimension_key,
                value_text.as_deref(),
                value_json.as_ref(),
                chapter_index,
                confidence,
                &provenance,
                &mut *tx,
                &mut result,
            )
            .await?;
        }
    }

    tx.commit().await?;
    Ok(result)
}

async fn apply_character_introduce_entity(
    book_id: &str,
    entity_type: &str,
    canonical_name: &str,
    display_name: &str,
    short_summary: Option<&str>,
    aliases: Vec<String>,
    chapter_index: i64,
    confidence: f64,
    provenance: &CharacterWriteProvenance,
    conn: &mut SqliteConnection,
    result: &mut ReductionResult,
) -> anyhow::Result<()> {
    let existing =
        EntityRepo::find_entity_by_alias_with_conn(conn, book_id, canonical_name).await?;
    let entity = if let Some(e) = existing {
        EntityRepo::update_last_seen_with_conn(conn, &e.id, chapter_index).await?;
        result.entities_updated.push(e.id.clone());
        e
    } else {
        match EntityRepo::create_entity_with_conn(
            conn,
            book_id,
            entity_type,
            canonical_name,
            display_name,
            short_summary,
            confidence,
            chapter_index,
        )
        .await
        {
            Ok(entity) => {
                EntityRepo::create_alias_with_conn(
                    conn,
                    book_id,
                    &entity.id,
                    canonical_name,
                    "canonical",
                    chapter_index,
                    confidence,
                    Some(&provenance.claim_id),
                )
                .await?;
                result.entities_created.push(entity.id.clone());
                entity
            }
            Err(_) => {
                let existing =
                    EntityRepo::get_by_canonical_name_with_conn(conn, book_id, canonical_name)
                        .await?
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Entity creation failed and lookup by canonical name returned nothing for '{}'",
                                canonical_name
                            )
                        })?;
                EntityRepo::update_last_seen_with_conn(conn, &existing.id, chapter_index).await?;
                result.entities_updated.push(existing.id.clone());
                existing
            }
        }
    };

    for alias in aliases {
        if alias != canonical_name {
            let alias_result = EntityRepo::create_alias_with_conn(
                conn,
                book_id,
                &entity.id,
                &alias,
                "ai_extracted",
                chapter_index,
                confidence,
                Some(&provenance.claim_id),
            )
            .await;
            if alias_result.is_ok() {
                result.aliases_created.push(alias);
            }
        }
    }

    result.claims_accepted.push(provenance.claim_id.clone());
    Ok(())
}

async fn apply_character_add_alias(
    book_id: &str,
    entity_id: &str,
    alias: &str,
    alias_type: &str,
    chapter_index: i64,
    confidence: f64,
    provenance: &CharacterWriteProvenance,
    conn: &mut SqliteConnection,
    result: &mut ReductionResult,
) -> anyhow::Result<()> {
    let Some(entity) = EntityRepo::get_by_id_with_conn(conn, entity_id).await? else {
        return Ok(());
    };

    EntityRepo::update_last_seen_with_conn(conn, &entity.id, chapter_index).await?;
    let alias_result = EntityRepo::create_alias_with_conn(
        conn,
        book_id,
        &entity.id,
        alias,
        alias_type,
        chapter_index,
        confidence,
        Some(&provenance.claim_id),
    )
    .await;
    if alias_result.is_ok() {
        result.aliases_created.push(alias.to_string());
    }
    result.claims_accepted.push(provenance.claim_id.clone());
    Ok(())
}

async fn apply_character_update_property(
    book_id: &str,
    entity_id: &str,
    dimension_key: &str,
    value_text: Option<&str>,
    value_json: Option<&serde_json::Value>,
    chapter_index: i64,
    confidence: f64,
    provenance: &CharacterWriteProvenance,
    conn: &mut SqliteConnection,
    result: &mut ReductionResult,
) -> anyhow::Result<()> {
    let dimension =
        PropertyRepo::validate_dimension_with_conn(conn, book_id, "character", dimension_key)
            .await?;
    let Some(dimension) = dimension else {
        return Ok(());
    };

    let Some(entity) = EntityRepo::get_by_id_with_conn(conn, entity_id).await? else {
        return Ok(());
    };

    EntityRepo::update_last_seen_with_conn(conn, &entity.id, chapter_index).await?;
    let value_json_string = value_json.map(serde_json::Value::to_string);

    match dimension.merge_strategy.as_str() {
        "append" => {
            PropertyRepo::apply_append_with_conn(
                conn,
                book_id,
                &entity.id,
                dimension_key,
                value_text,
                value_json_string.as_deref(),
                chapter_index,
                &provenance.claim_id,
                confidence,
            )
            .await?;
        }
        _ => {
            PropertyRepo::apply_replace_with_conn(
                conn,
                book_id,
                &entity.id,
                dimension_key,
                value_text,
                value_json_string.as_deref(),
                chapter_index,
                &provenance.claim_id,
                confidence,
            )
            .await?;
        }
    }

    result.properties_inserted.push(provenance.claim_id.clone());
    result.current_properties_updated.push(entity.id);
    result.claims_accepted.push(provenance.claim_id.clone());
    Ok(())
}

// --- Relationship Reducer ---

/// Result of reducing relationship claims into canonical state.
#[derive(Debug, Default)]
pub struct RelationshipReductionResult {
    pub relationships_created: Vec<String>,
    pub relationships_updated: Vec<String>,
    pub events_created: Vec<String>,
    pub claims_accepted: Vec<String>,
    pub claims_skipped: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RelationshipWriteProvenance {
    pub claim_id: String,
    pub evidence_span_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RelationshipWriteCommand {
    pub book_id: String,
    pub subject_character_id: String,
    pub object_character_id: String,
    pub relation_group: String,
    pub relation_label: String,
    pub directionality: String,
    pub current_state: Option<String>,
    pub strength: f64,
    pub polarity: String,
    pub importance_score: f64,
    pub confidence: f64,
    pub chapter_index: i64,
    pub provenance: RelationshipWriteProvenance,
}

#[derive(Debug, Clone)]
pub struct RelationshipStatusWriteCommand {
    pub book_id: String,
    pub relationship_id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct IdentityWriteProvenance {
    pub claim_id: String,
    pub evidence_span_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum IdentityWriteCommand {
    Merge {
        book_id: String,
        entity_a_id: String,
        entity_b_id: String,
        link_type: String,
        survivor_hint: String,
        confidence: f64,
        reason_code: String,
        provenance: IdentityWriteProvenance,
    },
    Link {
        book_id: String,
        entity_a_id: String,
        entity_b_id: String,
        link_type: String,
        confidence: f64,
        provenance: IdentityWriteProvenance,
    },
}

/// Result of reducing identity claims into canonical state.
#[derive(Debug, Default)]
pub struct IdentityReductionResult {
    pub identity_links_created: Vec<String>,
    pub merge_operations_completed: Vec<String>,
    pub redirects_created: Vec<String>,
    pub aliases_transferred: Vec<String>,
    pub claims_accepted: Vec<String>,
    pub claims_skipped: Vec<String>,
}

pub async fn apply_identity_write(
    command: IdentityWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<IdentityReductionResult> {
    let mut result = IdentityReductionResult::default();
    let mut tx = pool.begin().await?;

    match command {
        IdentityWriteCommand::Merge {
            book_id,
            entity_a_id,
            entity_b_id,
            link_type,
            survivor_hint,
            confidence,
            reason_code,
            provenance,
        } => {
            apply_identity_merge_command(
                &book_id,
                &entity_a_id,
                &entity_b_id,
                &link_type,
                &survivor_hint,
                confidence,
                &reason_code,
                &provenance,
                &mut *tx,
                &mut result,
            )
            .await?;
        }
        IdentityWriteCommand::Link {
            book_id,
            entity_a_id,
            entity_b_id,
            link_type,
            confidence,
            provenance,
        } => {
            apply_identity_link_command(
                &book_id,
                &entity_a_id,
                &entity_b_id,
                &link_type,
                confidence,
                &provenance,
                &mut *tx,
                &mut result,
            )
            .await?;
        }
    }

    tx.commit().await?;
    Ok(result)
}

async fn apply_identity_merge_command(
    book_id: &str,
    entity_a_id: &str,
    entity_b_id: &str,
    link_type: &str,
    survivor_hint: &str,
    confidence: f64,
    reason_code: &str,
    provenance: &IdentityWriteProvenance,
    conn: &mut SqliteConnection,
    result: &mut IdentityReductionResult,
) -> anyhow::Result<()> {
    if entity_a_id == entity_b_id {
        result.claims_skipped.push(provenance.claim_id.clone());
        return Ok(());
    }

    let entity_a = EntityRepo::get_by_id_with_conn(conn, entity_a_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_a not found: {}", entity_a_id))?;
    let entity_b = EntityRepo::get_by_id_with_conn(conn, entity_b_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_b not found: {}", entity_b_id))?;
    if entity_a.entity_type != "character" || entity_b.entity_type != "character" {
        result.claims_skipped.push(provenance.claim_id.clone());
        return Ok(());
    }
    if active_not_same_identity_exists(conn, book_id, &entity_a.id, &entity_b.id).await? {
        result.claims_skipped.push(provenance.claim_id.clone());
        return Ok(());
    }

    let (survivor, victim) = select_identity_merge_survivor(&entity_a, &entity_b, survivor_hint);

    let semantic_link = IdentityRepo::find_or_create_identity_link_with_conn(
        conn,
        book_id,
        &entity_a.id,
        &entity_b.id,
        link_type,
        confidence,
        &provenance.claim_id,
        "active",
    )
    .await?;
    result.identity_links_created.push(semantic_link.id.clone());

    let merge_op = IdentityRepo::create_merge_operation_with_conn(
        conn,
        book_id,
        &survivor.id,
        &victim.id,
        &semantic_link.id,
        reason_code,
        confidence,
        "pending",
    )
    .await?;

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE entities SET status = 'active', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&survivor.id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("UPDATE entities SET status = 'merged', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&victim.id)
        .execute(&mut *conn)
        .await?;

    let redirect_link = IdentityRepo::find_or_create_identity_link_with_conn(
        conn,
        book_id,
        &victim.id,
        &survivor.id,
        "redirect",
        confidence,
        &provenance.claim_id,
        "active",
    )
    .await?;
    result.redirects_created.push(redirect_link.id);

    transfer_aliases_to_survivor(conn, book_id, &survivor.id, &victim.id, result).await?;
    let property_conflict_count =
        merge_properties_to_survivor(conn, book_id, &survivor.id, &victim.id, &merge_op.id).await?;
    let relationship_merge_count =
        migrate_relationships_after_identity_merge(conn, book_id, &survivor.id, &victim.id).await?;
    KnowledgeRepo::remap_assertion_entities_after_redirect_with_conn(
        conn,
        book_id,
        &victim.id,
        &survivor.id,
    )
    .await?;

    IdentityRepo::update_merge_operation_status_with_conn(
        conn,
        &merge_op.id,
        "completed",
        Some(
            &serde_json::json!({
                "survivor_entity_id": survivor.id,
                "victim_entity_id": victim.id,
                "source_claim_id": provenance.claim_id,
                "relationship_merge_count": relationship_merge_count,
            })
            .to_string(),
        ),
    )
    .await?;
    sqlx::query("UPDATE entity_merge_operations SET property_conflict_count = ? WHERE id = ?")
        .bind(property_conflict_count)
        .bind(&merge_op.id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("UPDATE entity_merge_operations SET relationship_merge_count = ? WHERE id = ?")
        .bind(relationship_merge_count)
        .bind(&merge_op.id)
        .execute(&mut *conn)
        .await?;
    result.merge_operations_completed.push(merge_op.id);
    result.claims_accepted.push(provenance.claim_id.clone());

    Ok(())
}

async fn apply_identity_link_command(
    book_id: &str,
    entity_a_id: &str,
    entity_b_id: &str,
    link_type: &str,
    confidence: f64,
    provenance: &IdentityWriteProvenance,
    conn: &mut SqliteConnection,
    result: &mut IdentityReductionResult,
) -> anyhow::Result<()> {
    if !matches!(link_type, "possible_same_identity" | "not_same_identity")
        || provenance
            .evidence_span_ids
            .iter()
            .all(|span| span.trim().is_empty())
    {
        result.claims_skipped.push(provenance.claim_id.clone());
        return Ok(());
    }
    if entity_a_id == entity_b_id {
        result.claims_skipped.push(provenance.claim_id.clone());
        return Ok(());
    }

    let entity_a = EntityRepo::get_by_id_with_conn(conn, entity_a_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_a not found: {}", entity_a_id))?;
    let entity_b = EntityRepo::get_by_id_with_conn(conn, entity_b_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_b not found: {}", entity_b_id))?;
    if entity_a.entity_type != "character" || entity_b.entity_type != "character" {
        result.claims_skipped.push(provenance.claim_id.clone());
        return Ok(());
    }

    let link = IdentityRepo::find_or_create_identity_link_with_conn(
        conn,
        book_id,
        &entity_a.id,
        &entity_b.id,
        link_type,
        confidence,
        &provenance.claim_id,
        "active",
    )
    .await?;
    result.identity_links_created.push(link.id);
    result.claims_accepted.push(provenance.claim_id.clone());

    Ok(())
}

fn select_identity_merge_survivor(
    entity_a: &crate::storage::db::v4::entity_repo::EntityRecord,
    entity_b: &crate::storage::db::v4::entity_repo::EntityRecord,
    survivor_hint: &str,
) -> (
    crate::storage::db::v4::entity_repo::EntityRecord,
    crate::storage::db::v4::entity_repo::EntityRecord,
) {
    match survivor_hint {
        "entity_a" => (entity_a.clone(), entity_b.clone()),
        "entity_b" => (entity_b.clone(), entity_a.clone()),
        _ if entity_a.first_seen_chapter <= entity_b.first_seen_chapter => {
            (entity_a.clone(), entity_b.clone())
        }
        _ => (entity_b.clone(), entity_a.clone()),
    }
}

async fn active_not_same_identity_exists(
    conn: &mut SqliteConnection,
    book_id: &str,
    left_id: &str,
    right_id: &str,
) -> anyhow::Result<bool> {
    let (a, b) = if left_id <= right_id {
        (left_id, right_id)
    } else {
        (right_id, left_id)
    };
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entity_identity_links
         WHERE book_id = ? AND entity_a_id = ? AND entity_b_id = ?
           AND link_type = 'not_same_identity' AND status = 'active'",
    )
    .bind(book_id)
    .bind(a)
    .bind(b)
    .fetch_one(&mut *conn)
    .await?;
    Ok(count.0 > 0)
}

async fn transfer_aliases_to_survivor(
    conn: &mut SqliteConnection,
    book_id: &str,
    survivor_id: &str,
    victim_id: &str,
    result: &mut IdentityReductionResult,
) -> anyhow::Result<()> {
    let aliases: Vec<(String, String, i64, f64, Option<String>)> = sqlx::query_as(
        "SELECT alias, alias_type, first_seen_chapter, confidence, source_claim_id
         FROM entity_aliases WHERE book_id = ? AND entity_id = ?",
    )
    .bind(book_id)
    .bind(victim_id)
    .fetch_all(&mut *conn)
    .await?;

    for (alias, alias_type, first_seen_chapter, confidence, source_claim_id) in aliases {
        EntityRepo::create_alias_with_conn(
            conn,
            book_id,
            survivor_id,
            &alias,
            &alias_type,
            first_seen_chapter,
            confidence,
            source_claim_id.as_deref(),
        )
        .await?;
        result.aliases_transferred.push(alias);
    }

    Ok(())
}

async fn merge_properties_to_survivor(
    conn: &mut SqliteConnection,
    book_id: &str,
    survivor_id: &str,
    victim_id: &str,
    merge_operation_id: &str,
) -> anyhow::Result<i64> {
    let victim_current: Vec<(String, String, Option<String>, Option<String>, i64, f64)> =
        sqlx::query_as(
            "SELECT dimension_key, property_id, value_text, value_json, updated_chapter, confidence
             FROM entity_current_properties
             WHERE book_id = ? AND entity_id = ?",
        )
        .bind(book_id)
        .bind(victim_id)
        .fetch_all(&mut *conn)
        .await?;

    let mut conflict_count = 0;
    for (
        dimension_key,
        victim_property_id,
        victim_value_text,
        victim_value_json,
        victim_chapter,
        victim_confidence,
    ) in &victim_current
    {
        let survivor_current: Option<(String, Option<String>, Option<String>, i64, f64)> =
            sqlx::query_as(
                "SELECT property_id, value_text, value_json, updated_chapter, confidence
                 FROM entity_current_properties
                 WHERE book_id = ? AND entity_id = ? AND dimension_key = ?",
            )
            .bind(book_id)
            .bind(survivor_id)
            .bind(dimension_key)
            .fetch_optional(&mut *conn)
            .await?;
        let merge_strategy =
            PropertyRepo::validate_dimension_with_conn(conn, book_id, "character", dimension_key)
                .await?
                .map(|dimension| dimension.merge_strategy)
                .unwrap_or_else(|| "unknown".to_string());

        let mut chosen = (
            victim_property_id.clone(),
            victim_value_text.clone(),
            victim_value_json.clone(),
            *victim_chapter,
            *victim_confidence,
        );

        if let Some((
            survivor_property_id,
            survivor_value_text,
            survivor_value_json,
            survivor_chapter,
            survivor_confidence,
        )) = survivor_current
        {
            let values_differ = survivor_value_text != *victim_value_text
                || survivor_value_json != *victim_value_json;
            if values_differ {
                let resolution = match merge_strategy.as_str() {
                    "replace" if *victim_confidence > survivor_confidence => {
                        "keep_higher_confidence"
                    }
                    "replace"
                        if (*victim_confidence - survivor_confidence).abs() < f64::EPSILON
                            && *victim_chapter >= survivor_chapter =>
                    {
                        "keep_latest"
                    }
                    "append" => "keep_both_history",
                    _ => "keep_survivor",
                };
                IdentityRepo::create_merge_conflict_with_conn(
                    conn,
                    book_id,
                    merge_operation_id,
                    survivor_id,
                    victim_id,
                    "property_conflict",
                    Some(dimension_key),
                    survivor_value_text.as_deref(),
                    victim_value_text.as_deref(),
                    resolution,
                )
                .await?;
                conflict_count += 1;
            }

            let keep_victim = match merge_strategy.as_str() {
                "replace" => {
                    *victim_confidence > survivor_confidence
                        || ((*victim_confidence - survivor_confidence).abs() < f64::EPSILON
                            && *victim_chapter >= survivor_chapter)
                }
                "append" => false,
                _ => false,
            };
            if !keep_victim {
                chosen = (
                    survivor_property_id,
                    survivor_value_text,
                    survivor_value_json,
                    survivor_chapter,
                    survivor_confidence,
                );
            }
        }

        upsert_current_property_with_conn(
            conn,
            book_id,
            survivor_id,
            dimension_key,
            &chosen.0,
            chosen.1.as_deref(),
            chosen.2.as_deref(),
            chosen.3,
            chosen.4,
        )
        .await?;
    }

    sqlx::query("UPDATE entity_properties SET entity_id = ? WHERE book_id = ? AND entity_id = ?")
        .bind(survivor_id)
        .bind(book_id)
        .bind(victim_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM entity_current_properties WHERE book_id = ? AND entity_id = ?")
        .bind(book_id)
        .bind(victim_id)
        .execute(&mut *conn)
        .await?;

    Ok(conflict_count)
}

async fn upsert_current_property_with_conn(
    conn: &mut SqliteConnection,
    book_id: &str,
    entity_id: &str,
    dimension_key: &str,
    property_id: &str,
    value_text: Option<&str>,
    value_json: Option<&str>,
    updated_chapter: i64,
    confidence: f64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO entity_current_properties
         (book_id, entity_id, dimension_key, property_id, value_text, value_json, updated_chapter, confidence)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(book_id, entity_id, dimension_key) DO UPDATE SET
           property_id = excluded.property_id,
           value_text = excluded.value_text,
           value_json = excluded.value_json,
           updated_chapter = excluded.updated_chapter,
           confidence = excluded.confidence",
    )
    .bind(book_id)
    .bind(entity_id)
    .bind(dimension_key)
    .bind(property_id)
    .bind(value_text)
    .bind(value_json)
    .bind(updated_chapter)
    .bind(confidence)
    .execute(conn)
    .await?;
    Ok(())
}

async fn migrate_relationships_after_identity_merge(
    conn: &mut SqliteConnection,
    book_id: &str,
    survivor_id: &str,
    victim_id: &str,
) -> anyhow::Result<i64> {
    let relationships: Vec<RelationshipRecord> = sqlx::query_as(
        "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
         FROM relationships
         WHERE book_id = ? AND status = 'active'
           AND (subject_character_id = ? OR object_character_id = ?)",
    )
    .bind(book_id)
    .bind(victim_id)
    .bind(victim_id)
    .fetch_all(&mut *conn)
    .await?;

    let mut migrated_count = 0;
    for relationship in relationships {
        let (remapped_subject, remapped_object) =
            remap_relationship_pair(&relationship, survivor_id, victim_id);

        if remapped_subject == remapped_object {
            mark_relationship_inactive(conn, &relationship.id).await?;
            migrated_count += 1;
            continue;
        }

        let existing = find_relationship_by_unique_key_excluding_id(
            conn,
            book_id,
            &relationship.id,
            &remapped_subject,
            &remapped_object,
            &relationship.relation_group,
            &relationship.directionality,
        )
        .await?;

        if let Some(target) = existing {
            fold_duplicate_relationship(conn, &target, &relationship).await?;
            sqlx::query(
                "UPDATE relationship_events SET relationship_id = ? WHERE relationship_id = ?",
            )
            .bind(&target.id)
            .bind(&relationship.id)
            .execute(&mut *conn)
            .await?;
            mark_relationship_inactive(conn, &relationship.id).await?;
        } else {
            update_relationship_pair(conn, &relationship.id, &remapped_subject, &remapped_object)
                .await?;
        }
        migrated_count += 1;
    }

    if migrated_count > 0 {
        invalidate_relationship_cache_with_conn(conn, book_id).await?;
    }

    Ok(migrated_count)
}

fn remap_relationship_pair(
    relationship: &RelationshipRecord,
    survivor_id: &str,
    victim_id: &str,
) -> (String, String) {
    let mut subject = if relationship.subject_character_id == victim_id {
        survivor_id.to_string()
    } else {
        relationship.subject_character_id.clone()
    };
    let mut object = if relationship.object_character_id == victim_id {
        survivor_id.to_string()
    } else {
        relationship.object_character_id.clone()
    };

    if relationship.directionality == "undirected" && subject > object {
        std::mem::swap(&mut subject, &mut object);
    }

    (subject, object)
}

async fn find_relationship_by_unique_key_excluding_id(
    conn: &mut SqliteConnection,
    book_id: &str,
    excluded_id: &str,
    subject_id: &str,
    object_id: &str,
    relation_group: &str,
    directionality: &str,
) -> anyhow::Result<Option<RelationshipRecord>> {
    let row = sqlx::query_as::<_, RelationshipRecord>(
        "SELECT id, book_id, subject_character_id, object_character_id, relation_group, relation_label, directionality, current_state, strength, polarity, confidence, importance_score, first_seen_chapter, last_changed_chapter, last_seen_chapter, status, created_at, updated_at
         FROM relationships
         WHERE book_id = ? AND subject_character_id = ? AND object_character_id = ?
           AND relation_group = ? AND directionality = ? AND id != ?
         LIMIT 1",
    )
    .bind(book_id)
    .bind(subject_id)
    .bind(object_id)
    .bind(relation_group)
    .bind(directionality)
    .bind(excluded_id)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(row)
}

async fn fold_duplicate_relationship(
    conn: &mut SqliteConnection,
    target: &RelationshipRecord,
    duplicate: &RelationshipRecord,
) -> anyhow::Result<()> {
    let duplicate_is_latest = duplicate.last_changed_chapter > target.last_changed_chapter;
    let relation_label = if duplicate_is_latest {
        &duplicate.relation_label
    } else {
        &target.relation_label
    };
    let current_state = if duplicate_is_latest {
        duplicate
            .current_state
            .clone()
            .or_else(|| target.current_state.clone())
    } else {
        target
            .current_state
            .clone()
            .or_else(|| duplicate.current_state.clone())
    };
    let polarity = if duplicate_is_latest {
        &duplicate.polarity
    } else {
        &target.polarity
    };
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE relationships
         SET relation_label = ?, current_state = ?, strength = ?, polarity = ?,
             confidence = ?, importance_score = ?, first_seen_chapter = ?,
             last_changed_chapter = ?, last_seen_chapter = ?, status = 'active', updated_at = ?
         WHERE id = ?",
    )
    .bind(relation_label)
    .bind(current_state.as_deref())
    .bind(target.strength.max(duplicate.strength))
    .bind(polarity)
    .bind(target.confidence.max(duplicate.confidence))
    .bind(target.importance_score.max(duplicate.importance_score))
    .bind(target.first_seen_chapter.min(duplicate.first_seen_chapter))
    .bind(
        target
            .last_changed_chapter
            .max(duplicate.last_changed_chapter),
    )
    .bind(target.last_seen_chapter.max(duplicate.last_seen_chapter))
    .bind(&now)
    .bind(&target.id)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

async fn update_relationship_pair(
    conn: &mut SqliteConnection,
    relationship_id: &str,
    subject_id: &str,
    object_id: &str,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE relationships
         SET subject_character_id = ?, object_character_id = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(subject_id)
    .bind(object_id)
    .bind(&now)
    .bind(relationship_id)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

async fn mark_relationship_inactive(
    conn: &mut SqliteConnection,
    relationship_id: &str,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE relationships SET status = 'inactive', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(relationship_id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

async fn invalidate_relationship_cache_with_conn(
    conn: &mut SqliteConnection,
    book_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM view_model_cache
         WHERE book_id = ? AND view_type IN ('relationship_graph', 'relationship_list')",
    )
    .bind(book_id)
    .execute(conn)
    .await?;

    Ok(())
}

pub async fn apply_relationship_write(
    command: RelationshipWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<RelationshipReductionResult> {
    let mut result = RelationshipReductionResult::default();
    let mut tx = pool.begin().await?;
    apply_relationship_write_with_conn(&command, &mut *tx, &mut result).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn apply_relationship_status_write(
    command: RelationshipStatusWriteCommand,
    pool: &SqlitePool,
) -> anyhow::Result<RelationshipReductionResult> {
    let mut result = RelationshipReductionResult::default();
    let mut tx = pool.begin().await?;
    apply_relationship_status_write_with_conn(&command, &mut *tx, &mut result).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn apply_relationship_status_write_with_conn(
    command: &RelationshipStatusWriteCommand,
    conn: &mut SqliteConnection,
    result: &mut RelationshipReductionResult,
) -> anyhow::Result<()> {
    if command.status != "inactive" {
        anyhow::bail!(
            "relationship status write only supports inactive, got {}",
            command.status
        );
    }

    let update_result = sqlx::query(
        "UPDATE relationships SET status = 'inactive', updated_at = ? WHERE book_id = ? AND id = ?",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(&command.book_id)
    .bind(&command.relationship_id)
    .execute(&mut *conn)
    .await?;
    if update_result.rows_affected() == 0 {
        anyhow::bail!("relationship not found for status write");
    }
    invalidate_relationship_cache_with_conn(conn, &command.book_id).await?;
    result
        .relationships_updated
        .push(command.relationship_id.clone());
    Ok(())
}

async fn apply_relationship_write_with_conn(
    command: &RelationshipWriteCommand,
    conn: &mut SqliteConnection,
    result: &mut RelationshipReductionResult,
) -> anyhow::Result<()> {
    let subject = EntityRepo::get_by_id_with_conn(conn, &command.subject_character_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("Subject entity {} not found", command.subject_character_id)
        })?;
    if subject.entity_type != "character" {
        return Err(anyhow::anyhow!(
            "Subject entity {} is not a character (type={})",
            command.subject_character_id,
            subject.entity_type
        ));
    }

    let object = EntityRepo::get_by_id_with_conn(conn, &command.object_character_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("Object entity {} not found", command.object_character_id)
        })?;
    if object.entity_type != "character" {
        return Err(anyhow::anyhow!(
            "Object entity {} is not a character (type={})",
            command.object_character_id,
            object.entity_type
        ));
    }

    // Canonicalize pair for undirected: sort by entity_id (min as subject, max as object)
    let (canonical_subject, canonical_object) = if command.directionality == "undirected" {
        if command.subject_character_id < command.object_character_id {
            (
                command.subject_character_id.clone(),
                command.object_character_id.clone(),
            )
        } else {
            (
                command.object_character_id.clone(),
                command.subject_character_id.clone(),
            )
        }
    } else {
        (
            command.subject_character_id.clone(),
            command.object_character_id.clone(),
        )
    };

    // Check for existing relationship with same pair + same group + same directionality
    let existing = RelationshipRepo::find_existing_with_conn(
        conn,
        &command.book_id,
        &canonical_subject,
        &canonical_object,
        &command.relation_group,
        &command.directionality,
    )
    .await?;

    let (relationship_id, is_new) = if let Some(rel) = existing {
        // Update existing relationship
        let new_confidence = rel.confidence.max(command.confidence);
        let new_importance = rel.importance_score.max(command.importance_score);
        let changed = command.current_state.as_deref() != rel.current_state.as_deref()
            || (command.strength - rel.strength).abs() > f64::EPSILON
            || command.polarity != rel.polarity;
        let last_changed = if changed {
            command.chapter_index
        } else {
            rel.last_changed_chapter
        };

        RelationshipRepo::update_relationship_with_conn(
            conn,
            &rel.id,
            &command.relation_label,
            command.current_state.as_deref(),
            command.strength,
            &command.polarity,
            new_confidence,
            new_importance,
            last_changed,
            command.chapter_index,
            "active",
        )
        .await?;

        result.relationships_updated.push(rel.id.clone());
        (rel.id, false)
    } else {
        // Check if same pair + same group exists with DIFFERENT directionality
        // Query for any relationship with same pair + same group (regardless of directionality)
        let any_existing = sqlx::query_scalar::<_, String>(
            "SELECT id FROM relationships WHERE book_id = ? AND subject_character_id = ? AND object_character_id = ? AND relation_group = ? LIMIT 1"
        )
        .bind(&command.book_id)
        .bind(&canonical_subject)
        .bind(&canonical_object)
        .bind(&command.relation_group)
        .fetch_optional(&mut *conn)
        .await?;

        if any_existing.is_some() {
            // Same pair + same group + different directionality → skip
            result
                .claims_skipped
                .push(command.provenance.claim_id.clone());
            return Ok(());
        }

        // Create new relationship
        let new_rel = RelationshipRepo::create_relationship_with_conn(
            conn,
            &command.book_id,
            &canonical_subject,
            &canonical_object,
            &command.relation_group,
            &command.relation_label,
            &command.directionality,
            command.current_state.as_deref(),
            command.strength,
            &command.polarity,
            command.confidence,
            command.importance_score,
            command.chapter_index,
        )
        .await?;

        result.relationships_created.push(new_rel.id.clone());
        (new_rel.id, true)
    };

    // Create relationship event
    RelationshipEventRepo::create_event_with_conn(
        conn,
        &command.book_id,
        &relationship_id,
        if is_new { "creation" } else { "update" },
        &command.relation_group,
        &command.relation_label,
        command.current_state.as_deref(),
        Some(command.strength),
        Some(&command.polarity),
        command.chapter_index,
        &command.provenance.claim_id,
        command.confidence,
    )
    .await?;

    result
        .events_created
        .push(command.provenance.claim_id.clone());
    result
        .claims_accepted
        .push(command.provenance.claim_id.clone());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    async fn setup() -> (SqlitePool, ClaimRepo, EntityRepo, PropertyRepo) {
        let dir = std::env::temp_dir().join(format!("reader-v4-reducer-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        let pool = db::init_pool(&database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();

        // Insert chapter for FK
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, 'b1', 1, 'text', 'hash', datetime('now'))")
            .bind(&chapter_id).execute(&pool).await.unwrap();

        // Insert source span for FK
        let span_id = uuid::Uuid::new_v4().to_string();
        let seg_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, 'b1', ?, 'hash', 0, datetime('now'))")
            .bind(&seg_id).bind(&chapter_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, 'b1', ?, 'hash', ?, 0, 0, 10, 'test text', datetime('now'))")
            .bind(&span_id).bind(&chapter_id).bind(&seg_id).execute(&pool).await.unwrap();

        // Insert ai_run for FK
        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, 'b1', ?, 'extract', 'gpt-4', 'v1', 1, 'hash', 'running', datetime('now'))")
            .bind(&run_id).bind(&chapter_id).execute(&pool).await.unwrap();

        let claim_repo = ClaimRepo::new(pool.clone());
        let entity_repo = EntityRepo::new(pool.clone());
        let property_repo = PropertyRepo::new(pool.clone());

        (pool, claim_repo, entity_repo, property_repo)
    }

    #[tokio::test]
    async fn reduce_skips_high_risk_claims() {
        let (pool, claim_repo, _entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Create high-risk claim (quarantined)
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "property_update",
                Some("张三"),
                None,
                None,
                None,
                "life_status = 死亡",
                Some("死亡"),
                None,
                &span_id.0,
                &run_id.0,
                0.95,
                "high",
            )
            .await
            .unwrap();

        // Mark as quarantined (simulating claim_writer behavior)
        claim_repo
            .update_claim_status(&claim.id, "quarantined")
            .await
            .unwrap();

        let claims = vec![claim];
        let result = reduce_claims(&claims, "b1", &pool).await.unwrap();

        // Should be skipped
        assert_eq!(result.entities_created.len(), 0);
        assert_eq!(result.claims_accepted.len(), 0);
    }

    #[tokio::test]
    async fn reduce_skips_minor_event_claims() {
        let (pool, claim_repo, _entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Create minor_event claim
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "minor_event",
                Some("张三"),
                None,
                None,
                None,
                "张三和李四切磋",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.8,
                "low",
            )
            .await
            .unwrap();

        let claims = vec![claim];
        let result = reduce_claims(&claims, "b1", &pool).await.unwrap();

        // minor_event is skipped by reducer
        assert_eq!(result.entities_created.len(), 0);
        assert_eq!(result.claims_skipped.len(), 1);
    }

    #[tokio::test]
    async fn character_reducer_accepts_typed_entity_command_without_claim_value_json() {
        let (pool, claim_repo, _entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let ledger_claim = claim_repo
            .create_claim(
                "b1",
                1,
                "entity_introduction",
                Some("张三"),
                None,
                None,
                None,
                "recorded only as ledger provenance",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "low",
            )
            .await
            .unwrap();

        let command = CharacterWriteCommand::IntroduceEntity {
            book_id: "b1".to_string(),
            entity_type: "character".to_string(),
            canonical_name: "张三".to_string(),
            display_name: "张三".to_string(),
            short_summary: Some("主角".to_string()),
            aliases: vec!["小张".to_string()],
            chapter_index: 1,
            confidence: 0.9,
            provenance: CharacterWriteProvenance {
                claim_id: ledger_claim.id,
                evidence_span_ids: vec![],
            },
        };

        let result = apply_character_write(command, &pool).await.unwrap();

        assert_eq!(result.entities_created.len(), 1);
        assert_eq!(result.aliases_created, vec!["小张".to_string()]);

        let entity_repo = EntityRepo::new(pool.clone());
        let entity = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap()
            .expect("typed character command should create canonical alias");
        assert_eq!(entity.canonical_name, "张三");

        let alias_entity = entity_repo
            .find_entity_by_alias("b1", "小张")
            .await
            .unwrap()
            .expect("typed character command should create supplied alias");
        assert_eq!(alias_entity.id, entity.id);
    }

    #[tokio::test]
    async fn character_reducer_accepts_typed_alias_command_without_claim_object_mention() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &entity.id, "张三", "canonical", 1, 0.9, None)
            .await
            .unwrap();
        let ledger_claim = claim_repo
            .create_claim(
                "b1",
                3,
                "alias",
                Some("张三"),
                None,
                Some(&entity.id),
                None,
                "recorded only as ledger provenance",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.88,
                "low",
            )
            .await
            .unwrap();

        let command = CharacterWriteCommand::AddAlias {
            book_id: "b1".to_string(),
            entity_id: entity.id.clone(),
            alias: "张真人".to_string(),
            alias_type: "title".to_string(),
            chapter_index: 3,
            confidence: 0.88,
            provenance: CharacterWriteProvenance {
                claim_id: ledger_claim.id,
                evidence_span_ids: vec![],
            },
        };

        let result = apply_character_write(command, &pool).await.unwrap();

        assert_eq!(result.aliases_created, vec!["张真人".to_string()]);
        let found = entity_repo
            .find_entity_by_alias("b1", "张真人")
            .await
            .unwrap()
            .expect("typed alias command should create supplied alias");
        assert_eq!(found.id, entity.id);
        let updated = entity_repo.get_by_id(&entity.id).await.unwrap().unwrap();
        assert_eq!(updated.last_seen_chapter, 3);
    }

    #[tokio::test]
    async fn character_reducer_accepts_typed_property_command_without_claim_predicate_or_value_json(
    ) {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let ledger_claim = claim_repo
            .create_claim(
                "b1",
                4,
                "property_update",
                Some("张三"),
                None,
                Some(&entity.id),
                None,
                "recorded only as ledger provenance",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.86,
                "low",
            )
            .await
            .unwrap();

        let command = CharacterWriteCommand::UpdateProperty {
            book_id: "b1".to_string(),
            entity_id: entity.id.clone(),
            dimension_key: "realm".to_string(),
            value_text: Some("金丹".to_string()),
            value_json: None,
            chapter_index: 4,
            confidence: 0.86,
            provenance: CharacterWriteProvenance {
                claim_id: ledger_claim.id,
                evidence_span_ids: vec![],
            },
        };

        let result = apply_character_write(command, &pool).await.unwrap();

        assert_eq!(result.properties_inserted.len(), 1);
        let property_repo = PropertyRepo::new(pool.clone());
        let current = property_repo
            .get_current_property("b1", &entity.id, "realm")
            .await
            .unwrap()
            .expect("typed property command should update canonical property");
        assert_eq!(current.value_text.as_deref(), Some("金丹"));
        let updated = entity_repo.get_by_id(&entity.id).await.unwrap().unwrap();
        assert_eq!(updated.last_seen_chapter, 4);
    }

    #[tokio::test]
    async fn generic_reduce_claims_no_longer_applies_character_profile_claims() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "entity_introduction",
                Some("旧桥角色"),
                None,
                None,
                None,
                "is a character",
                None,
                Some(r#"{"entity_type":"character","aliases":["旧桥"]}"#),
                &span_id.0,
                &run_id.0,
                0.9,
                "low",
            )
            .await
            .unwrap();

        let result = reduce_claims(&[claim.clone()], "b1", &pool).await.unwrap();

        assert!(result.claims_accepted.is_empty());
        assert!(
            entity_repo
                .find_entity_by_alias("b1", "旧桥角色")
                .await
                .unwrap()
                .is_none(),
            "generic reducer must not apply character/profile claims; character_processor owns typed commands"
        );
        let stored = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored.status, "proposed");
    }

    // --- Relationship Reducer Tests ---

    /// Helper: build the typed relationship command owned by Step 3.
    fn relationship_write_command(
        claim_id: &str,
        subject_character_id: &str,
        object_character_id: &str,
        chapter_index: i64,
        evidence_span_id: &str,
        group: &str,
        label: &str,
        directionality: &str,
        current_state: Option<&str>,
        strength: f64,
        polarity: &str,
        importance_score: f64,
        judge_confidence: f64,
    ) -> RelationshipWriteCommand {
        RelationshipWriteCommand {
            book_id: "b1".to_string(),
            subject_character_id: subject_character_id.to_string(),
            object_character_id: object_character_id.to_string(),
            relation_group: group.to_string(),
            relation_label: label.to_string(),
            directionality: directionality.to_string(),
            current_state: current_state.map(|state| state.to_string()),
            strength,
            polarity: polarity.to_string(),
            importance_score,
            confidence: judge_confidence,
            chapter_index,
            provenance: RelationshipWriteProvenance {
                claim_id: claim_id.to_string(),
                evidence_span_ids: vec![evidence_span_id.to_string()],
            },
        }
    }

    /// Helper: create two character entities, return their IDs.
    async fn create_two_characters(
        _pool: &SqlitePool,
        entity_repo: &EntityRepo,
    ) -> (String, String) {
        let e1 = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let e2 = entity_repo
            .create_entity("b1", "character", "李四", "李四", None, 0.8, 1)
            .await
            .unwrap();
        (e1.id, e2.id)
    }

    fn identity_merge_command(
        claim_id: &str,
        entity_a_id: &str,
        entity_b_id: &str,
        evidence_span_id: &str,
        survivor_hint: &str,
        confidence: f64,
        reason_code: &str,
    ) -> IdentityWriteCommand {
        IdentityWriteCommand::Merge {
            book_id: "b1".to_string(),
            entity_a_id: entity_a_id.to_string(),
            entity_b_id: entity_b_id.to_string(),
            link_type: "same_identity".to_string(),
            survivor_hint: survivor_hint.to_string(),
            confidence,
            reason_code: reason_code.to_string(),
            provenance: IdentityWriteProvenance {
                claim_id: claim_id.to_string(),
                evidence_span_ids: vec![evidence_span_id.to_string()],
            },
        }
    }

    fn identity_link_command(
        claim_id: &str,
        entity_a_id: &str,
        entity_b_id: &str,
        evidence_span_id: &str,
        link_type: &str,
        confidence: f64,
    ) -> IdentityWriteCommand {
        IdentityWriteCommand::Link {
            book_id: "b1".to_string(),
            entity_a_id: entity_a_id.to_string(),
            entity_b_id: entity_b_id.to_string(),
            link_type: link_type.to_string(),
            confidence,
            provenance: IdentityWriteProvenance {
                claim_id: claim_id.to_string(),
                evidence_span_ids: vec![evidence_span_id.to_string()],
            },
        }
    }

    /// Helper: create a non-character entity.
    async fn create_non_character(
        _pool: &SqlitePool,
        entity_repo: &EntityRepo,
        name: &str,
        entity_type: &str,
    ) -> String {
        let e = entity_repo
            .create_entity("b1", entity_type, name, name, None, 0.3, 1)
            .await
            .unwrap();
        e.id
    }

    #[tokio::test]
    async fn rel_reducer_creates_new_relationship() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let command = relationship_write_command(
            &claim.id,
            &subject_id,
            &object_id,
            1,
            &span_id.0,
            "friendship",
            "friends",
            "undirected",
            None,
            0.7,
            "positive",
            0.8,
            0.9,
        );
        let result = apply_relationship_write(command, &pool).await.unwrap();

        assert_eq!(result.relationships_created.len(), 1);
        assert_eq!(result.events_created.len(), 1);
        assert_eq!(result.claims_accepted.len(), 1);
        assert!(result.claims_skipped.is_empty());

        // Verify the relationship exists and is canonicalized
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo
            .get_by_id(&result.relationships_created[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rel.relation_group, "friendship");
        assert_eq!(rel.directionality, "undirected");
        // Undirected: smaller ID should be subject
        let (expected_sub, expected_obj) = if subject_id < object_id {
            (&subject_id, &object_id)
        } else {
            (&object_id, &subject_id)
        };
        assert_eq!(&rel.subject_character_id, expected_sub);
        assert_eq!(&rel.object_character_id, expected_obj);
    }

    #[tokio::test]
    async fn rel_reducer_accepts_typed_command_without_claim_value_json() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let command = RelationshipWriteCommand {
            book_id: "b1".to_string(),
            subject_character_id: subject_id.clone(),
            object_character_id: object_id.clone(),
            relation_group: "friendship".to_string(),
            relation_label: "friends".to_string(),
            directionality: "undirected".to_string(),
            current_state: None,
            strength: 0.7,
            polarity: "positive".to_string(),
            importance_score: 0.8,
            confidence: 0.9,
            chapter_index: 1,
            provenance: RelationshipWriteProvenance {
                claim_id: claim.id,
                evidence_span_ids: vec![span_id.0],
            },
        };

        let result = apply_relationship_write(command, &pool).await.unwrap();

        assert_eq!(result.relationships_created.len(), 1);
        assert_eq!(result.events_created.len(), 1);
        assert!(result.claims_skipped.is_empty());
    }

    #[tokio::test]
    async fn rel_reducer_updates_existing_relationship() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // First claim: create
        let claim1 = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let command1 = relationship_write_command(
            &claim1.id,
            &subject_id,
            &object_id,
            1,
            &span_id.0,
            "friendship",
            "friends",
            "undirected",
            Some("close"),
            0.7,
            "positive",
            0.8,
            0.9,
        );
        let result1 = apply_relationship_write(command1, &pool).await.unwrap();
        assert_eq!(result1.relationships_created.len(), 1);

        // Second claim: update same pair + same group
        let claim2 = claim_repo
            .create_claim(
                "b1",
                5,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.95,
                "high",
            )
            .await
            .unwrap();

        let command2 = relationship_write_command(
            &claim2.id,
            &subject_id,
            &object_id,
            5,
            &span_id.0,
            "friendship",
            "best friends",
            "undirected",
            Some("very close"),
            0.9,
            "positive",
            0.9,
            0.95,
        );
        let result2 = apply_relationship_write(command2, &pool).await.unwrap();
        assert_eq!(result2.relationships_updated.len(), 1);
        assert_eq!(result2.relationships_created.len(), 0);
        assert_eq!(result2.claims_accepted.len(), 1);

        // Verify updated fields
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo
            .get_by_id(&result2.relationships_updated[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rel.relation_label, "best friends");
        assert_eq!(rel.current_state.as_deref(), Some("very close"));
        assert!((rel.strength - 0.9).abs() < f64::EPSILON);
        assert!((rel.confidence - 0.95).abs() < f64::EPSILON);
        assert_eq!(rel.last_seen_chapter, 5);
    }

    #[tokio::test]
    async fn rel_reducer_different_group_coexists() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // friendship
        let claim1 = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        // rivalry (different group)
        let claim2 = claim_repo
            .create_claim(
                "b1",
                2,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.85,
                "high",
            )
            .await
            .unwrap();

        let command1 = relationship_write_command(
            &claim1.id,
            &subject_id,
            &object_id,
            1,
            &span_id.0,
            "friendship",
            "friends",
            "undirected",
            None,
            0.7,
            "positive",
            0.8,
            0.9,
        );
        let command2 = relationship_write_command(
            &claim2.id,
            &subject_id,
            &object_id,
            2,
            &span_id.0,
            "rivalry",
            "rivals",
            "directed",
            Some("competitive"),
            0.6,
            "negative",
            0.7,
            0.85,
        );
        let result1 = apply_relationship_write(command1, &pool).await.unwrap();
        let result2 = apply_relationship_write(command2, &pool).await.unwrap();
        let relationships_created =
            result1.relationships_created.len() + result2.relationships_created.len();
        let claims_accepted = result1.claims_accepted.len() + result2.claims_accepted.len();
        assert_eq!(relationships_created, 2, "different groups should coexist");
        assert_eq!(claims_accepted, 2);

        // Verify both relationships exist
        let rel_repo = RelationshipRepo::new(pool.clone());
        let all = rel_repo.list_by_book("b1", None, None).await.unwrap();
        assert_eq!(all.len(), 2);
        let groups: Vec<&str> = all.iter().map(|r| r.relation_group.as_str()).collect();
        assert!(groups.contains(&"friendship"));
        assert!(groups.contains(&"rivalry"));
    }

    #[tokio::test]
    async fn rel_reducer_directed_preserves_direction() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Directed: 张三 → 李四 (mentor)
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.95,
                "high",
            )
            .await
            .unwrap();

        let command = relationship_write_command(
            &claim.id,
            &subject_id,
            &object_id,
            1,
            &span_id.0,
            "mentorship",
            "mentor",
            "directed",
            Some("teaching"),
            0.8,
            "positive",
            0.9,
            0.95,
        );
        let result = apply_relationship_write(command, &pool).await.unwrap();
        assert_eq!(result.relationships_created.len(), 1);

        // Verify direction preserved (NOT canonicalized)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo
            .get_by_id(&result.relationships_created[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rel.subject_character_id, subject_id);
        assert_eq!(rel.object_character_id, object_id);
        assert_eq!(rel.directionality, "directed");
    }

    #[tokio::test]
    async fn rel_reducer_undirected_canonicalizes_pair() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (e1, e2) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Pass with reversed order: larger ID as subject
        let (larger, smaller) = if e1 > e2 { (&e1, &e2) } else { (&e2, &e1) };
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("李四"),
                Some("张三"),
                Some(larger),
                Some(smaller),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let command = relationship_write_command(
            &claim.id,
            larger,
            smaller,
            1,
            &span_id.0,
            "friendship",
            "friends",
            "undirected",
            None,
            0.7,
            "positive",
            0.8,
            0.9,
        );
        let result = apply_relationship_write(command, &pool).await.unwrap();
        assert_eq!(result.relationships_created.len(), 1);

        // Verify canonicalized: smaller ID as subject
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo
            .get_by_id(&result.relationships_created[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            &rel.subject_character_id, smaller,
            "undirected: smaller ID should be subject"
        );
        assert_eq!(
            &rel.object_character_id, larger,
            "undirected: larger ID should be object"
        );
    }

    #[tokio::test]
    async fn rel_reducer_subject_not_character_skipped() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let place_id = create_non_character(&pool, &entity_repo, "青云门", "place").await;
        let (char_id, _) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Subject is place, not character
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("青云门"),
                Some("张三"),
                Some(&place_id),
                Some(&char_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.8,
                "high",
            )
            .await
            .unwrap();

        let command = relationship_write_command(
            &claim.id,
            &place_id,
            &char_id,
            1,
            &span_id.0,
            "hierarchy",
            "belongs to",
            "directed",
            None,
            0.5,
            "neutral",
            0.5,
            0.8,
        );
        let result = apply_relationship_write(command, &pool).await;

        assert!(
            result.is_err(),
            "typed relationship command with non-character subject should be rejected"
        );

        let claim_repo2 = ClaimRepo::new(pool.clone());
        let fetched = claim_repo2.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, "proposed");
    }

    #[tokio::test]
    async fn rel_reducer_object_not_character_skipped() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (char_id, _) = create_two_characters(&pool, &entity_repo).await;
        let ability_id = create_non_character(&pool, &entity_repo, "剑法", "ability").await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Object is ability, not character
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("剑法"),
                Some(&char_id),
                Some(&ability_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.7,
                "high",
            )
            .await
            .unwrap();

        let command = relationship_write_command(
            &claim.id,
            &char_id,
            &ability_id,
            1,
            &span_id.0,
            "other_social",
            "practices",
            "undirected",
            None,
            0.3,
            "neutral",
            0.3,
            0.7,
        );
        let result = apply_relationship_write(command, &pool).await;

        assert!(
            result.is_err(),
            "typed relationship command with non-character object should be rejected"
        );
    }

    #[tokio::test]
    async fn rel_reducer_independent_transaction_no_rollback() {
        // Verify that relationship_reducer failure does NOT affect entity/property state
        let (pool, claim_repo, entity_repo, _) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // First: run typed character reducer to create an entity (simulating Phase 1)
        let entity_claim = claim_repo
            .create_claim(
                "b1",
                1,
                "entity_introduction",
                Some("王五"),
                None,
                None,
                None,
                "is a character",
                Some(r#"{"short_summary":"新角色","aliases":[]}"#),
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "low",
            )
            .await
            .unwrap();
        let entity_result = apply_character_write(
            CharacterWriteCommand::IntroduceEntity {
                book_id: "b1".to_string(),
                entity_type: "character".to_string(),
                canonical_name: "王五".to_string(),
                display_name: "王五".to_string(),
                short_summary: Some("新角色".to_string()),
                aliases: vec![],
                chapter_index: 1,
                confidence: 0.9,
                provenance: CharacterWriteProvenance {
                    claim_id: entity_claim.id,
                    evidence_span_ids: vec![span_id.0.clone()],
                },
            },
            &pool,
        )
        .await
        .unwrap();
        assert_eq!(entity_result.entities_created.len(), 1);

        // Now: run relationship_reducer with a claim that will fail (missing entity)
        let bad_rel_claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("王五"),
                Some("不存在"),
                Some("nonexistent_id_1"),
                Some("nonexistent_id_2"),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.8,
                "high",
            )
            .await
            .unwrap();

        let command = relationship_write_command(
            &bad_rel_claim.id,
            "nonexistent_id_1",
            "nonexistent_id_2",
            1,
            &span_id.0,
            "friendship",
            "friends",
            "undirected",
            None,
            0.5,
            "neutral",
            0.5,
            0.8,
        );
        let rel_result = apply_relationship_write(command, &pool).await;

        assert!(
            rel_result.is_err(),
            "bad relationship command should fail its own transaction"
        );

        // But entity from Phase 1 is still there
        let entity = entity_repo
            .get_by_canonical_name("b1", "王五")
            .await
            .unwrap();
        assert!(
            entity.is_some(),
            "Phase 1 entity should NOT be rolled back by relationship_reducer failure"
        );
        assert_eq!(entity.unwrap().canonical_name, "王五");
    }

    #[tokio::test]
    async fn rel_reducer_reports_accepted_claim_without_mutating_ledger_status() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        // Verify claim starts as proposed
        assert_eq!(claim.status, "proposed");

        let command = relationship_write_command(
            &claim.id,
            &subject_id,
            &object_id,
            1,
            &span_id.0,
            "friendship",
            "friends",
            "undirected",
            None,
            0.7,
            "positive",
            0.8,
            0.9,
        );
        let result = apply_relationship_write(command, &pool).await.unwrap();
        assert_eq!(result.claims_accepted.len(), 1);
        assert_eq!(result.claims_accepted[0], claim.id);

        // Step 4 reports the outcome; Step 3/pipeline owns ledger status transitions.
        let claim_repo2 = ClaimRepo::new(pool.clone());
        let fetched = claim_repo2.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, "proposed");
    }

    #[tokio::test]
    async fn rel_reducer_uses_typed_command_fields_not_claim_payload_hints() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        // The ledger payload may preserve extractor hints, but Step 4 must only
        // apply the typed command materialized by Step 3.
        let value_json = serde_json::json!({
            "relation_hint": "对手",
            "relation_group": "rivalry",
            "relation_label": "对手",
            "importance_hint": 0.3,
            "is_long_term_or_significant_hint": false,
        })
        .to_string();

        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&subject_id),
                Some(&object_id),
                "relationship",
                None,
                Some(&value_json),
                &span_id.0,
                &run_id.0,
                0.5,
                "high",
            )
            .await
            .unwrap();

        let command = relationship_write_command(
            &claim.id,
            &subject_id,
            &object_id,
            1,
            &span_id.0,
            "mentorship",
            "master",
            "directed",
            Some("teaching"),
            0.9,
            "positive",
            0.95,
            0.95,
        );
        let result = apply_relationship_write(command, &pool).await.unwrap();
        assert_eq!(result.relationships_created.len(), 1);

        // Verify the relationship uses command fields, not preserved ledger hints.
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo
            .get_by_id(&result.relationships_created[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            rel.relation_group, "mentorship",
            "should use command relation_group, not claim payload relation_group"
        );
        assert_eq!(
            rel.relation_label, "master",
            "should use command relation_label, not claim payload relation_label"
        );
        assert_eq!(rel.directionality, "directed");
        assert!(
            (rel.confidence - 0.95).abs() < f64::EPSILON,
            "should use command confidence"
        );
        assert!(
            (rel.importance_score - 0.95).abs() < f64::EPSILON,
            "should use command importance_score"
        );
    }

    #[tokio::test]
    async fn identity_reducer_accepts_typed_link_command_without_claim_value_json() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (entity_a_id, entity_b_id) = create_two_characters(&pool, &entity_repo).await;
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let ledger_claim = claim_repo
            .create_claim(
                "b1",
                8,
                "not_same_identity",
                Some("张三"),
                Some("李四"),
                Some(&entity_a_id),
                Some(&entity_b_id),
                "not same identity",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let command = IdentityWriteCommand::Link {
            book_id: "b1".to_string(),
            entity_a_id: entity_a_id.clone(),
            entity_b_id: entity_b_id.clone(),
            link_type: "not_same_identity".to_string(),
            confidence: 0.92,
            provenance: IdentityWriteProvenance {
                claim_id: ledger_claim.id,
                evidence_span_ids: vec![span_id.0],
            },
        };

        let result = apply_identity_write(command, &pool).await.unwrap();

        assert_eq!(result.identity_links_created.len(), 1);
        assert_eq!(result.claims_accepted.len(), 1);
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_identity_links WHERE book_id = 'b1' AND link_type = 'not_same_identity' AND status = 'active'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn identity_reducer_accepts_typed_merge_command_without_claim_value_json() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (victim_id, survivor_id) = create_two_characters(&pool, &entity_repo).await;
        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let ledger_claim = claim_repo
            .create_claim(
                "b1",
                8,
                "identity_reveal",
                Some("张三"),
                Some("李四"),
                Some(&victim_id),
                Some(&survivor_id),
                "identity reveal",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();
        let command = IdentityWriteCommand::Merge {
            book_id: "b1".to_string(),
            entity_a_id: victim_id.clone(),
            entity_b_id: survivor_id.clone(),
            link_type: "same_identity".to_string(),
            survivor_hint: "entity_b".to_string(),
            confidence: 0.94,
            reason_code: "explicit_reveal".to_string(),
            provenance: IdentityWriteProvenance {
                claim_id: ledger_claim.id,
                evidence_span_ids: vec![span_id.0],
            },
        };

        let result = apply_identity_write(command, &pool).await.unwrap();

        assert_eq!(result.identity_links_created.len(), 1);
        assert_eq!(result.redirects_created.len(), 1);
        assert_eq!(result.merge_operations_completed.len(), 1);
        assert_eq!(result.claims_accepted.len(), 1);
        let victim = entity_repo.get_by_id(&victim_id).await.unwrap().unwrap();
        let survivor = entity_repo.get_by_id(&survivor_id).await.unwrap().unwrap();
        assert_eq!(victim.status, "merged");
        assert_eq!(survivor.status, "active");
    }

    #[tokio::test]
    async fn identity_reducer_merge_creates_links_operation_redirect_and_accepts_claim() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let victim = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &victim.id, "黑衣人", "canonical", 2, 0.95, None)
            .await
            .unwrap();

        let claim = claim_repo
            .create_claim(
                "b1",
                2,
                "identity_reveal",
                Some("黑衣人"),
                Some("张三"),
                Some(&victim.id),
                Some(&survivor.id),
                "黑衣人 is revealed as 张三",
                Some("摘下面具"),
                None,
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        let command = identity_merge_command(
            &claim.id,
            &victim.id,
            &survivor.id,
            &span_id.0,
            "entity_b",
            0.96,
            "explicit_reveal",
        );
        apply_identity_write(command, &pool).await.unwrap();

        let victim_after = entity_repo.get_by_id(&victim.id).await.unwrap().unwrap();
        let survivor_after = entity_repo.get_by_id(&survivor.id).await.unwrap().unwrap();
        assert_eq!(victim_after.status, "merged");
        assert_eq!(survivor_after.status, "active");

        let identity_links: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT entity_a_id, entity_b_id, link_type, status FROM entity_identity_links WHERE book_id = 'b1' ORDER BY link_type",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(
            identity_links.iter().any(|(a, b, link_type, status)| {
                link_type == "same_identity"
                    && status == "active"
                    && [a.as_str(), b.as_str()].contains(&victim.id.as_str())
                    && [a.as_str(), b.as_str()].contains(&survivor.id.as_str())
            }),
            "semantic same_identity link should exist"
        );
        assert!(
            identity_links.iter().any(|(a, b, link_type, status)| {
                a == &victim.id
                    && b == &survivor.id
                    && link_type == "redirect"
                    && status == "active"
            }),
            "redirect should point victim -> survivor"
        );

        let merge_ops: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_merge_operations WHERE book_id = 'b1' AND survivor_entity_id = ? AND victim_entity_id = ? AND status = 'completed'",
        )
        .bind(&survivor.id)
        .bind(&victim.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(merge_ops.0, 1);

        let survivor_aliases = entity_repo
            .list_aliases_by_entity(&survivor.id)
            .await
            .unwrap();
        assert!(
            survivor_aliases.iter().any(|alias| alias.alias == "黑衣人"),
            "victim alias should transfer to survivor"
        );

        assert!(claim_repo.get_claim(&claim.id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn identity_reducer_merge_transfers_replace_properties_and_records_conflict() {
        let (pool, claim_repo, entity_repo, property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let victim = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        let survivor_prop_claim = claim_repo
            .create_claim(
                "b1",
                1,
                "property_update",
                Some("张三"),
                None,
                Some(&survivor.id),
                None,
                "realm = 筑基",
                Some("筑基"),
                None,
                &span_id.0,
                &run_id.0,
                0.7,
                "medium",
            )
            .await
            .unwrap();
        let victim_prop_claim = claim_repo
            .create_claim(
                "b1",
                2,
                "property_update",
                Some("黑衣人"),
                None,
                Some(&victim.id),
                None,
                "realm = 金丹",
                Some("金丹"),
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "medium",
            )
            .await
            .unwrap();
        property_repo
            .apply_replace(
                "b1",
                &survivor.id,
                "realm",
                Some("筑基"),
                None,
                1,
                &survivor_prop_claim.id,
                0.7,
            )
            .await
            .unwrap();
        property_repo
            .apply_replace(
                "b1",
                &victim.id,
                "realm",
                Some("金丹"),
                None,
                2,
                &victim_prop_claim.id,
                0.9,
            )
            .await
            .unwrap();

        let merge_claim = claim_repo
            .create_claim(
                "b1",
                2,
                "identity_reveal",
                Some("黑衣人"),
                Some("张三"),
                Some(&victim.id),
                Some(&survivor.id),
                "黑衣人 is revealed as 张三",
                Some("摘下面具"),
                None,
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        let command = identity_merge_command(
            &merge_claim.id,
            &victim.id,
            &survivor.id,
            &span_id.0,
            "entity_b",
            0.96,
            "explicit_reveal",
        );
        apply_identity_write(command, &pool).await.unwrap();

        let current = property_repo
            .get_current_property("b1", &survivor.id, "realm")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.value_text.as_deref(), Some("金丹"));

        let survivor_history_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_properties WHERE book_id = 'b1' AND entity_id = ? AND dimension_key = 'realm'",
        )
        .bind(&survivor.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(survivor_history_count.0, 2);

        let conflict_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_merge_conflicts WHERE book_id = 'b1' AND dimension_key = 'realm' AND conflict_type = 'property_conflict'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(conflict_count.0, 1);
    }

    #[tokio::test]
    async fn identity_reducer_records_possible_same_identity_without_merge() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let entity_a = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let entity_b = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let claim = claim_repo
            .create_claim(
                "b1",
                2,
                "entity_merge_candidate",
                Some("黑衣人"),
                Some("张三"),
                Some(&entity_a.id),
                Some(&entity_b.id),
                "黑衣人与张三极为相似",
                Some("只是相似"),
                None,
                &span_id.0,
                &run_id.0,
                0.62,
                "high",
            )
            .await
            .unwrap();

        let command = identity_link_command(
            &claim.id,
            &entity_a.id,
            &entity_b.id,
            &span_id.0,
            "possible_same_identity",
            0.62,
        );
        let result = apply_identity_write(command, &pool).await.unwrap();

        assert_eq!(result.identity_links_created.len(), 1);
        assert!(result.merge_operations_completed.is_empty());
        let link_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_identity_links WHERE book_id = 'b1' AND link_type = 'possible_same_identity' AND status = 'active'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(link_count.0, 1);
        assert_eq!(
            entity_repo
                .get_by_id(&entity_a.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "active"
        );
        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
    }

    #[tokio::test]
    async fn identity_reducer_records_not_same_identity_without_merge() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let entity_a = entity_repo
            .create_entity("b1", "character", "此张三", "此张三", None, 0.6, 2)
            .await
            .unwrap();
        let entity_b = entity_repo
            .create_entity("b1", "character", "彼张三", "彼张三", None, 0.9, 1)
            .await
            .unwrap();
        let claim = claim_repo
            .create_claim(
                "b1",
                2,
                "not_same_identity",
                Some("此张三"),
                Some("彼张三"),
                Some(&entity_a.id),
                Some(&entity_b.id),
                "此张三并非彼张三",
                Some("并非彼张三"),
                None,
                &span_id.0,
                &run_id.0,
                0.98,
                "high",
            )
            .await
            .unwrap();

        let command = identity_link_command(
            &claim.id,
            &entity_a.id,
            &entity_b.id,
            &span_id.0,
            "not_same_identity",
            0.98,
        );
        let result = apply_identity_write(command, &pool).await.unwrap();

        assert_eq!(result.identity_links_created.len(), 1);
        assert!(result.merge_operations_completed.is_empty());
        let link_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_identity_links WHERE book_id = 'b1' AND link_type = 'not_same_identity' AND status = 'active'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(link_count.0, 1);
        assert_eq!(
            entity_repo
                .get_by_id(&entity_a.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "active"
        );
        assert_eq!(result.claims_accepted, vec![claim.id.clone()]);
    }

    #[tokio::test]
    async fn identity_reducer_merge_inactivates_relationship_self_edge_and_keeps_events() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let victim = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        let rel_repo = RelationshipRepo::new(pool.clone());
        let event_repo = RelationshipEventRepo::new(pool.clone());
        let rel = rel_repo
            .create_relationship(
                "b1",
                &victim.id,
                &survivor.id,
                "rivalry",
                "宿敌",
                "directed",
                Some("active hostility"),
                0.8,
                "negative",
                0.9,
                0.9,
                2,
            )
            .await
            .unwrap();
        let rel_claim = claim_repo
            .create_claim(
                "b1",
                2,
                "relationship_update",
                Some("黑衣人"),
                Some("张三"),
                Some(&victim.id),
                Some(&survivor.id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();
        event_repo
            .create_event(
                "b1",
                &rel.id,
                "creation",
                "rivalry",
                "宿敌",
                Some("active hostility"),
                Some(0.8),
                Some("negative"),
                2,
                &rel_claim.id,
                0.9,
            )
            .await
            .unwrap();

        let merge_claim = claim_repo
            .create_claim(
                "b1",
                2,
                "identity_reveal",
                Some("黑衣人"),
                Some("张三"),
                Some(&victim.id),
                Some(&survivor.id),
                "黑衣人 is revealed as 张三",
                Some("摘下面具"),
                None,
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        let command = identity_merge_command(
            &merge_claim.id,
            &victim.id,
            &survivor.id,
            &span_id.0,
            "entity_b",
            0.96,
            "explicit_reveal",
        );
        apply_identity_write(command, &pool).await.unwrap();

        let migrated_rel = rel_repo.get_by_id(&rel.id).await.unwrap().unwrap();
        assert_eq!(migrated_rel.status, "inactive");
        let event_count = event_repo.count_by_relationship(&rel.id).await.unwrap();
        assert_eq!(
            event_count, 1,
            "relationship event evidence should be retained"
        );
    }

    #[tokio::test]
    async fn identity_reducer_merge_dedupes_relationships_and_moves_events() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let victim = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let other = entity_repo
            .create_entity("b1", "character", "李四", "李四", None, 0.7, 1)
            .await
            .unwrap();

        let rel_repo = RelationshipRepo::new(pool.clone());
        let event_repo = RelationshipEventRepo::new(pool.clone());
        let duplicate_rel = rel_repo
            .create_relationship(
                "b1",
                &victim.id,
                &other.id,
                "alliance",
                "旧盟友",
                "directed",
                Some("旧盟约"),
                0.6,
                "positive",
                0.7,
                0.95,
                2,
            )
            .await
            .unwrap();
        let survivor_rel = rel_repo
            .create_relationship(
                "b1",
                &survivor.id,
                &other.id,
                "alliance",
                "盟友",
                "directed",
                Some("共同御敌"),
                0.8,
                "positive",
                0.9,
                0.8,
                3,
            )
            .await
            .unwrap();

        let duplicate_claim = claim_repo
            .create_claim(
                "b1",
                2,
                "relationship_update",
                Some("黑衣人"),
                Some("李四"),
                Some(&victim.id),
                Some(&other.id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.7,
                "high",
            )
            .await
            .unwrap();
        let survivor_claim = claim_repo
            .create_claim(
                "b1",
                3,
                "relationship_update",
                Some("张三"),
                Some("李四"),
                Some(&survivor.id),
                Some(&other.id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();
        event_repo
            .create_event(
                "b1",
                &duplicate_rel.id,
                "creation",
                "alliance",
                "旧盟友",
                Some("旧盟约"),
                Some(0.6),
                Some("positive"),
                2,
                &duplicate_claim.id,
                0.7,
            )
            .await
            .unwrap();
        event_repo
            .create_event(
                "b1",
                &survivor_rel.id,
                "creation",
                "alliance",
                "盟友",
                Some("共同御敌"),
                Some(0.8),
                Some("positive"),
                3,
                &survivor_claim.id,
                0.9,
            )
            .await
            .unwrap();

        let merge_claim = claim_repo
            .create_claim(
                "b1",
                3,
                "identity_reveal",
                Some("黑衣人"),
                Some("张三"),
                Some(&victim.id),
                Some(&survivor.id),
                "黑衣人 is revealed as 张三",
                Some("摘下面具"),
                None,
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        let command = identity_merge_command(
            &merge_claim.id,
            &victim.id,
            &survivor.id,
            &span_id.0,
            "entity_b",
            0.96,
            "explicit_reveal",
        );
        apply_identity_write(command, &pool).await.unwrap();

        let duplicate_after = rel_repo
            .get_by_id(&duplicate_rel.id)
            .await
            .unwrap()
            .unwrap();
        let survivor_after = rel_repo.get_by_id(&survivor_rel.id).await.unwrap().unwrap();
        assert_eq!(duplicate_after.status, "inactive");
        assert_eq!(survivor_after.status, "active");
        assert_eq!(survivor_after.importance_score, 0.95);
        assert_eq!(
            event_repo
                .count_by_relationship(&survivor_rel.id)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            event_repo
                .count_by_relationship(&duplicate_rel.id)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn identity_reducer_merge_remaps_relationship_pair_and_invalidates_cache() {
        let (pool, claim_repo, entity_repo, _property_repo) = setup().await;

        let span_id: (String,) =
            sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let run_id: (String,) =
            sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let victim = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.6, 2)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        let other = entity_repo
            .create_entity("b1", "character", "李四", "李四", None, 0.7, 1)
            .await
            .unwrap();

        let rel_repo = RelationshipRepo::new(pool.clone());
        let event_repo = RelationshipEventRepo::new(pool.clone());
        let rel = rel_repo
            .create_relationship(
                "b1",
                &victim.id,
                &other.id,
                "alliance",
                "盟友",
                "directed",
                Some("共同御敌"),
                0.8,
                "positive",
                0.9,
                0.9,
                2,
            )
            .await
            .unwrap();
        let rel_claim = claim_repo
            .create_claim(
                "b1",
                2,
                "relationship_update",
                Some("黑衣人"),
                Some("李四"),
                Some(&victim.id),
                Some(&other.id),
                "relationship",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();
        event_repo
            .create_event(
                "b1",
                &rel.id,
                "creation",
                "alliance",
                "盟友",
                Some("共同御敌"),
                Some(0.8),
                Some("positive"),
                2,
                &rel_claim.id,
                0.9,
            )
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO view_model_cache (book_id, view_type, scope_id, max_chapter, payload_json, updated_at)
             VALUES ('b1', 'relationship_graph', '__book__', 2, '{}', datetime('now')),
                    ('b1', 'relationship_list', '__book__', 2, '{}', datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        let merge_claim = claim_repo
            .create_claim(
                "b1",
                2,
                "identity_reveal",
                Some("黑衣人"),
                Some("张三"),
                Some(&victim.id),
                Some(&survivor.id),
                "黑衣人 is revealed as 张三",
                Some("摘下面具"),
                None,
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        let command = identity_merge_command(
            &merge_claim.id,
            &victim.id,
            &survivor.id,
            &span_id.0,
            "entity_b",
            0.96,
            "explicit_reveal",
        );
        apply_identity_write(command, &pool).await.unwrap();

        let migrated_rel = rel_repo.get_by_id(&rel.id).await.unwrap().unwrap();
        assert_eq!(migrated_rel.subject_character_id, survivor.id);
        assert_eq!(migrated_rel.object_character_id, other.id);
        assert_eq!(migrated_rel.status, "active");
        assert_eq!(event_repo.count_by_relationship(&rel.id).await.unwrap(), 1);

        let cache_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM view_model_cache WHERE book_id = 'b1' AND view_type IN ('relationship_graph', 'relationship_list')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(cache_count.0, 0);
    }
}
