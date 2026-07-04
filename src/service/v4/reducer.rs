use crate::service::v4::knowledge_judge::{
    KnowledgeCardAction, KnowledgeJudgeDecision, KnowledgeJudgeOutput,
};
use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::identity_repo::IdentityRepo;
use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
use crate::storage::db::v4::property_repo::PropertyRepo;
use crate::storage::db::v4::relationship_repo::{
    RelationshipEventRepo, RelationshipRecord, RelationshipRepo,
};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashMap;

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

pub async fn reduce_knowledge_claims(
    claims: &[ClaimRecord],
    book_id: &str,
    pool: &SqlitePool,
    judge_outputs: &HashMap<String, KnowledgeJudgeOutput>,
) -> anyhow::Result<KnowledgeReductionResult> {
    let mut result = KnowledgeReductionResult::default();
    let reducible: Vec<&ClaimRecord> = claims
        .iter()
        .filter(|claim| claim.status == "proposed" && claim.claim_type == "knowledge_assertion")
        .collect();

    if reducible.is_empty() {
        return Ok(result);
    }

    let mut tx = pool.begin().await?;
    for claim in reducible {
        let Some(output) = judge_outputs.get(&claim.id) else {
            result.claims_skipped.push(claim.id.clone());
            continue;
        };
        if output.decision == KnowledgeJudgeDecision::Reject
            || output.card_action == KnowledgeCardAction::Uncertain
        {
            result.claims_skipped.push(claim.id.clone());
            continue;
        }

        reduce_single_knowledge_claim(claim, book_id, output, &mut *tx, &mut result).await?;
        result.claims_accepted.push(claim.id.clone());
    }

    if !result.claims_accepted.is_empty() {
        ClaimRepo::batch_update_claim_status_with_conn(
            &mut *tx,
            &result.claims_accepted,
            "accepted",
        )
        .await?;
    }

    tx.commit().await?;
    Ok(result)
}

async fn reduce_single_knowledge_claim(
    claim: &ClaimRecord,
    book_id: &str,
    output: &KnowledgeJudgeOutput,
    conn: &mut SqliteConnection,
    result: &mut KnowledgeReductionResult,
) -> anyhow::Result<()> {
    let fields = extract_knowledge_claim_fields(claim)?;
    let card = match output.card_action {
        KnowledgeCardAction::CreateNewCard => {
            KnowledgeRepo::find_or_create_card_with_conn(
                conn,
                book_id,
                &fields.category,
                &fields.topic_display,
                &fields.topic_display,
                active_summary(output),
                output.confidence,
                fields.importance_score,
                claim.chapter_index,
            )
            .await?
        }
        KnowledgeCardAction::UseExistingCard => {
            let card_id = output
                .target_card_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("use_existing_card missing target_card_id"))?;
            get_knowledge_card_by_id_with_conn(conn, card_id).await?
        }
        KnowledgeCardAction::Uncertain => {
            result.claims_skipped.push(claim.id.clone());
            return Ok(());
        }
    };
    if card.book_id != book_id {
        anyhow::bail!(
            "knowledge card {} belongs to book {}, not {}",
            card.id,
            card.book_id,
            book_id
        );
    }
    result.cards_created_or_reused.push(card.id.clone());

    let assertion = KnowledgeRepo::find_or_create_assertion_with_conn(
        conn,
        book_id,
        &card.id,
        &claim.id,
        &fields.assertion_text,
        &output.assertion_status,
        output.confidence,
        fields.importance_score,
        claim.chapter_index,
    )
    .await?;
    result.assertions_inserted.push(assertion.id.clone());

    for entity_ref in &fields.referenced_entities {
        KnowledgeRepo::insert_assertion_entity_with_conn(
            conn,
            book_id,
            &assertion.id,
            &entity_ref.entity_id,
            &entity_ref.role,
        )
        .await?;
    }

    apply_knowledge_revision_links(conn, book_id, &card.id, &assertion.id, output, result).await?;

    if let Some(summary) = active_summary(output) {
        update_knowledge_card_summary_with_conn(
            conn,
            &card.id,
            summary,
            output.confidence,
            fields.importance_score,
            claim.chapter_index,
        )
        .await?;
    }
    invalidate_knowledge_cache_with_conn(conn, book_id).await?;

    Ok(())
}

struct KnowledgeClaimFields {
    category: String,
    topic_display: String,
    assertion_text: String,
    importance_score: f64,
    referenced_entities: Vec<KnowledgeClaimEntityRef>,
}

struct KnowledgeClaimEntityRef {
    entity_id: String,
    role: String,
}

fn extract_knowledge_claim_fields(claim: &ClaimRecord) -> anyhow::Result<KnowledgeClaimFields> {
    let parsed: serde_json::Value = serde_json::from_str(
        claim
            .value_json
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("knowledge claim missing value_json"))?,
    )?;
    let category = parsed
        .get("category")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("knowledge claim missing category"))?
        .trim()
        .to_string();
    let topic_display = parsed
        .get("topic_display")
        .or_else(|| parsed.get("raw_topic"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("knowledge claim missing topic"))?
        .trim()
        .to_string();
    let assertion_text = parsed
        .get("assertion_text")
        .and_then(|value| value.as_str())
        .or(claim.value_text.as_deref())
        .ok_or_else(|| anyhow::anyhow!("knowledge claim missing assertion_text"))?
        .trim()
        .to_string();
    let importance_score = parsed
        .get("importance_score")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.5);
    let referenced_entities = parsed
        .get("referenced_entity_mentions")
        .and_then(|value| value.as_array())
        .map(|mentions| {
            mentions
                .iter()
                .filter_map(|mention| {
                    let entity_id = mention
                        .get("resolved_entity_id")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())?;
                    let role = mention
                        .get("role")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("related");
                    Some(KnowledgeClaimEntityRef {
                        entity_id: entity_id.to_string(),
                        role: role.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(KnowledgeClaimFields {
        category,
        topic_display,
        assertion_text,
        importance_score,
        referenced_entities,
    })
}

fn active_summary(output: &KnowledgeJudgeOutput) -> Option<&str> {
    if output.assertion_status == "active" {
        output
            .current_summary
            .as_deref()
            .filter(|s| !s.trim().is_empty())
    } else {
        None
    }
}

async fn apply_knowledge_revision_links(
    conn: &mut SqliteConnection,
    book_id: &str,
    card_id: &str,
    new_assertion_id: &str,
    output: &KnowledgeJudgeOutput,
    result: &mut KnowledgeReductionResult,
) -> anyhow::Result<()> {
    let (old_status, link_type, target_vec) = match output.decision {
        KnowledgeJudgeDecision::ReviseExisting => (
            "revised",
            "supersedes",
            Some(&mut result.assertions_revised),
        ),
        KnowledgeJudgeDecision::ContradictExisting => (
            "contradicted",
            "contradicts",
            Some(&mut result.assertions_contradicted),
        ),
        _ => ("", "", None),
    };
    let Some(target_vec) = target_vec else {
        return Ok(());
    };

    for old_assertion_id in &output.affected_assertion_ids {
        validate_knowledge_assertion_same_card(conn, book_id, card_id, old_assertion_id).await?;
        KnowledgeRepo::update_assertion_status_with_conn(conn, old_assertion_id, old_status)
            .await?;
        target_vec.push(old_assertion_id.clone());
        let link = KnowledgeRepo::insert_assertion_link_with_conn(
            conn,
            book_id,
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
    use super::reduce_knowledge_claims;
    use crate::service::v4::knowledge_judge::{
        KnowledgeCardAction, KnowledgeJudgeDecision, KnowledgeJudgeOutput,
    };
    use crate::storage::db;
    use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
    use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
    use sqlx::SqlitePool;
    use std::collections::HashMap;

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
        let mut decisions = HashMap::new();
        decisions.insert(
            claim.id.clone(),
            output(
                KnowledgeJudgeDecision::AddNew,
                KnowledgeCardAction::CreateNewCard,
                None,
                Vec::new(),
                "active",
                Some("Cultivation realm summary"),
            ),
        );

        let result = reduce_knowledge_claims(&[claim.clone()], "b1", &pool, &decisions)
            .await
            .unwrap();

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
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "accepted"
        );
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
        let mut decisions = HashMap::new();
        decisions.insert(
            new_claim.id.clone(),
            output(
                KnowledgeJudgeDecision::Supplement,
                KnowledgeCardAction::UseExistingCard,
                Some(&card.id),
                Vec::new(),
                "active",
                Some("Updated factual summary"),
            ),
        );

        reduce_knowledge_claims(&[new_claim], "b1", &pool, &decisions)
            .await
            .unwrap();

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
            let mut decisions = HashMap::new();
            decisions.insert(
                claim.id.clone(),
                output(
                    decision,
                    KnowledgeCardAction::UseExistingCard,
                    Some(&card.id),
                    vec![old_assertion.id.clone()],
                    "active",
                    Some("Revised summary"),
                ),
            );

            let result = reduce_knowledge_claims(&[claim], "b1", &pool, &decisions)
                .await
                .unwrap();

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
            let mut decisions = HashMap::new();
            decisions.insert(
                claim.id.clone(),
                output(
                    decision,
                    KnowledgeCardAction::UseExistingCard,
                    Some(&card.id),
                    Vec::new(),
                    status,
                    Some("Should not replace"),
                ),
            );

            reduce_knowledge_claims(&[claim], "b1", &pool, &decisions)
                .await
                .unwrap();
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
        let mut decisions = HashMap::new();
        decisions.insert(
            claim.id.clone(),
            output(
                KnowledgeJudgeDecision::Supplement,
                KnowledgeCardAction::UseExistingCard,
                Some("missing-card"),
                Vec::new(),
                "active",
                Some("Summary"),
            ),
        );

        assert!(
            reduce_knowledge_claims(&[claim.clone()], "b1", &pool, &decisions)
                .await
                .is_err()
        );
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
        let mut decisions = HashMap::new();
        decisions.insert(
            claim.id.clone(),
            output(
                KnowledgeJudgeDecision::AddNew,
                KnowledgeCardAction::CreateNewCard,
                None,
                Vec::new(),
                "active",
                Some("Cultivation realm summary"),
            ),
        );

        reduce_knowledge_claims(&[claim.clone()], "b1", &pool, &decisions)
            .await
            .unwrap();
        reduce_knowledge_claims(&[claim], "b1", &pool, &decisions)
            .await
            .unwrap();

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
        let mut decisions = HashMap::new();
        decisions.insert(
            claim.id.clone(),
            output(
                KnowledgeJudgeDecision::AddNew,
                KnowledgeCardAction::CreateNewCard,
                None,
                Vec::new(),
                "active",
                Some("Cultivation realm summary"),
            ),
        );

        reduce_knowledge_claims(&[claim], "b1", &pool, &decisions)
            .await
            .unwrap();

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
/// - entity_introduction: create/update entity + aliases
/// - property_update: apply replace/append strategy
/// - alias: create alias for existing entity
/// - minor_event: skip (reducer doesn't consume)
/// - All claims in the batch commit/rollback together
/// - Successful claims marked as 'accepted'
pub async fn reduce_claims(
    claims: &[ClaimRecord],
    book_id: &str,
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
            "entity_introduction" => {
                reduce_entity_introduction(claim, book_id, &mut *tx, &mut result).await?;
            }
            "alias" => {
                reduce_alias(claim, book_id, &mut *tx, &mut result).await?;
            }
            "property_update" => {
                reduce_property_update(claim, book_id, &mut *tx, &mut result).await?;
            }
            // minor_event: ledger-only, intentionally not reduced.
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

/// Reduce an entity_introduction claim.
async fn reduce_entity_introduction(
    claim: &ClaimRecord,
    book_id: &str,
    conn: &mut SqliteConnection,
    result: &mut ReductionResult,
) -> anyhow::Result<()> {
    let subject_mention = match &claim.subject_mention {
        Some(m) => m,
        None => return Ok(()), // No mention, skip
    };

    // Extract entity info from value_json if available
    let (entity_type, short_summary, aliases) = extract_entity_info(claim);

    // Check if entity already exists by alias or canonical name
    let existing =
        EntityRepo::find_entity_by_alias_with_conn(conn, book_id, subject_mention).await?;
    let entity = if let Some(e) = existing {
        // Update last_seen_chapter
        EntityRepo::update_last_seen_with_conn(conn, &e.id, claim.chapter_index).await?;
        result.entities_updated.push(e.id.clone());
        e
    } else {
        // Create new entity; catch UNIQUE violation and return existing entity
        let entity_type_str = entity_type.as_deref().unwrap_or("character");
        match EntityRepo::create_entity_with_conn(
            conn,
            book_id,
            entity_type_str,
            subject_mention,
            subject_mention,
            short_summary.as_deref(),
            claim.confidence,
            claim.chapter_index,
        )
        .await
        {
            Ok(entity) => {
                // Create alias for the canonical name itself
                EntityRepo::create_alias_with_conn(
                    conn,
                    book_id,
                    &entity.id,
                    subject_mention,
                    "canonical",
                    claim.chapter_index,
                    claim.confidence,
                    Some(&claim.id),
                )
                .await?;

                result.entities_created.push(entity.id.clone());
                entity
            }
            Err(_) => {
                // UNIQUE violation: entity already exists by canonical name.
                // Find and return it.
                let existing =
                    EntityRepo::get_by_canonical_name_with_conn(conn, book_id, subject_mention)
                        .await?
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                    "Entity creation failed and lookup by canonical name returned nothing for '{}'",
                    subject_mention
                )
                        })?;
                EntityRepo::update_last_seen_with_conn(conn, &existing.id, claim.chapter_index)
                    .await?;
                result.entities_updated.push(existing.id.clone());
                existing
            }
        }
    };

    // Create additional aliases from value_json
    for alias in aliases {
        if alias != *subject_mention {
            let alias_result = EntityRepo::create_alias_with_conn(
                conn,
                book_id,
                &entity.id,
                &alias,
                "ai_extracted",
                claim.chapter_index,
                claim.confidence,
                Some(&claim.id),
            )
            .await;
            if alias_result.is_ok() {
                result.aliases_created.push(alias);
            }
        }
    }

    Ok(())
}

/// Reduce an alias claim.
async fn reduce_alias(
    claim: &ClaimRecord,
    book_id: &str,
    conn: &mut SqliteConnection,
    result: &mut ReductionResult,
) -> anyhow::Result<()> {
    let subject_mention = match &claim.subject_mention {
        Some(m) => m,
        None => return Ok(()),
    };

    let alias = match &claim.object_mention {
        Some(a) => a,
        None => return Ok(()),
    };

    // Find the entity
    let entity = if let Some(eid) = &claim.subject_entity_id {
        EntityRepo::get_by_id_with_conn(conn, eid).await?
    } else {
        EntityRepo::find_entity_by_alias_with_conn(conn, book_id, subject_mention).await?
    };

    if let Some(e) = entity {
        // Update last_seen_chapter for any accepted claim mentioning this entity
        EntityRepo::update_last_seen_with_conn(conn, &e.id, claim.chapter_index).await?;

        let alias_result = EntityRepo::create_alias_with_conn(
            conn,
            book_id,
            &e.id,
            alias,
            "ai_extracted",
            claim.chapter_index,
            claim.confidence,
            Some(&claim.id),
        )
        .await;
        if alias_result.is_ok() {
            result.aliases_created.push(alias.clone());
        }
    }

    Ok(())
}

/// Reduce a property_update claim.
async fn reduce_property_update(
    claim: &ClaimRecord,
    book_id: &str,
    conn: &mut SqliteConnection,
    result: &mut ReductionResult,
) -> anyhow::Result<()> {
    let subject_mention = match &claim.subject_mention {
        Some(m) => m,
        None => return Ok(()),
    };

    // Extract dimension from predicate (e.g., "realm = 筑基期" -> "realm")
    let dimension_key = extract_dimension_from_predicate(&claim.predicate)
        .ok_or_else(|| anyhow::anyhow!("No dimension key found"))?;

    // Validate dimension exists
    let dimension =
        PropertyRepo::validate_dimension_with_conn(conn, book_id, "character", &dimension_key)
            .await?;
    if dimension.is_none() {
        return Ok(()); // Dimension not registered, skip
    }

    // Find the entity
    let entity = if let Some(eid) = &claim.subject_entity_id {
        EntityRepo::get_by_id_with_conn(conn, eid).await?
    } else {
        EntityRepo::find_entity_by_alias_with_conn(conn, book_id, subject_mention).await?
    };

    let entity = match entity {
        Some(e) => e,
        None => return Ok(()), // Entity not found, skip
    };

    // Update last_seen_chapter for any accepted claim mentioning this entity
    EntityRepo::update_last_seen_with_conn(conn, &entity.id, claim.chapter_index).await?;

    // Determine merge strategy from validated dimension
    let merge_strategy = dimension
        .as_ref()
        .map(|d| d.merge_strategy.as_str())
        .unwrap_or("replace");

    // Apply the property update
    match merge_strategy {
        "append" => {
            PropertyRepo::apply_append_with_conn(
                conn,
                book_id,
                &entity.id,
                &dimension_key,
                claim.value_text.as_deref(),
                claim.value_json.as_deref(),
                claim.chapter_index,
                &claim.id,
                claim.confidence,
            )
            .await?;
        }
        _ => {
            // Default: replace
            PropertyRepo::apply_replace_with_conn(
                conn,
                book_id,
                &entity.id,
                &dimension_key,
                claim.value_text.as_deref(),
                claim.value_json.as_deref(),
                claim.chapter_index,
                &claim.id,
                claim.confidence,
            )
            .await?;
        }
    }

    result.properties_inserted.push(claim.id.clone());
    result.current_properties_updated.push(entity.id.clone());

    Ok(())
}

/// Extract entity info (entity_type, short_summary, aliases) from claim value_json.
fn extract_entity_info(claim: &ClaimRecord) -> (Option<String>, Option<String>, Vec<String>) {
    let value_json = match &claim.value_json {
        Some(j) => j,
        None => return (None, None, Vec::new()),
    };

    let parsed: serde_json::Value = match serde_json::from_str(value_json) {
        Ok(v) => v,
        Err(_) => return (None, None, Vec::new()),
    };

    let entity_type = parsed
        .get("entity_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let short_summary = parsed
        .get("short_summary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let aliases = parsed
        .get("aliases")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    (entity_type, short_summary, aliases)
}

/// Extract dimension key from predicate string like "realm = 筑基期".
fn extract_dimension_from_predicate(predicate: &str) -> Option<String> {
    predicate.split('=').next().map(|s| s.trim().to_string())
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

struct IdentityJudgeFields {
    decision: String,
    link_type: String,
    survivor_hint: String,
    judge_confidence: f64,
    reason_code: String,
}

fn extract_identity_judge_fields(claim: &ClaimRecord) -> Option<IdentityJudgeFields> {
    let parsed: serde_json::Value = serde_json::from_str(claim.value_json.as_ref()?).ok()?;
    Some(IdentityJudgeFields {
        decision: parsed
            .get("judge_decision")
            .and_then(|v| v.as_str())?
            .to_string(),
        link_type: parsed
            .get("link_type")
            .and_then(|v| v.as_str())
            .unwrap_or("same_identity")
            .to_string(),
        survivor_hint: parsed
            .get("survivor_hint")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        judge_confidence: parsed
            .get("judge_confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(claim.confidence),
        reason_code: parsed
            .get("reason_code")
            .and_then(|v| v.as_str())
            .unwrap_or("identity_judge")
            .to_string(),
    })
}

fn is_identity_claim_type(claim_type: &str) -> bool {
    matches!(
        claim_type,
        "identity_reveal"
            | "entity_merge_candidate"
            | "entity_split_candidate"
            | "not_same_identity"
    )
}

/// Reduce judge-approved identity claims into identity links and merge ledger rows.
pub async fn reduce_identity_claims(
    claims: &[ClaimRecord],
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<IdentityReductionResult> {
    let mut result = IdentityReductionResult::default();
    let reducible: Vec<&ClaimRecord> = claims
        .iter()
        .filter(|claim| claim.status == "proposed" && is_identity_claim_type(&claim.claim_type))
        .collect();

    if reducible.is_empty() {
        return Ok(result);
    }

    let mut tx = pool.begin().await?;
    for claim in reducible {
        let Some(fields) = extract_identity_judge_fields(claim) else {
            result.claims_skipped.push(claim.id.clone());
            continue;
        };

        match fields.decision.as_str() {
            "merge" => {
                reduce_identity_merge_claim(claim, book_id, &fields, &mut *tx, &mut result).await?;
            }
            "possible_same_identity" | "not_same_identity" => {
                reduce_identity_link_claim(claim, book_id, &fields, &mut *tx, &mut result).await?;
            }
            _ => result.claims_skipped.push(claim.id.clone()),
        }
    }

    if !result.claims_accepted.is_empty() {
        ClaimRepo::batch_update_claim_status_with_conn(
            &mut *tx,
            &result.claims_accepted,
            "accepted",
        )
        .await?;
    }

    tx.commit().await?;
    Ok(result)
}

async fn reduce_identity_merge_claim(
    claim: &ClaimRecord,
    book_id: &str,
    fields: &IdentityJudgeFields,
    conn: &mut SqliteConnection,
    result: &mut IdentityReductionResult,
) -> anyhow::Result<()> {
    let Some(entity_a_id) = claim.subject_entity_id.as_deref() else {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    };
    let Some(entity_b_id) = claim.object_entity_id.as_deref() else {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    };
    if entity_a_id == entity_b_id {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    }

    let entity_a = EntityRepo::get_by_id_with_conn(conn, entity_a_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_a not found: {}", entity_a_id))?;
    let entity_b = EntityRepo::get_by_id_with_conn(conn, entity_b_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_b not found: {}", entity_b_id))?;
    if entity_a.entity_type != "character" || entity_b.entity_type != "character" {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    }
    if active_not_same_identity_exists(conn, book_id, &entity_a.id, &entity_b.id).await? {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    }

    let (survivor, victim) = select_identity_merge_survivor(&entity_a, &entity_b, fields);

    let semantic_link = IdentityRepo::find_or_create_identity_link_with_conn(
        conn,
        book_id,
        &entity_a.id,
        &entity_b.id,
        &fields.link_type,
        fields.judge_confidence,
        &claim.id,
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
        &fields.reason_code,
        fields.judge_confidence,
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
        fields.judge_confidence,
        &claim.id,
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
                "source_claim_id": claim.id,
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
    result.claims_accepted.push(claim.id.clone());

    Ok(())
}

async fn reduce_identity_link_claim(
    claim: &ClaimRecord,
    book_id: &str,
    fields: &IdentityJudgeFields,
    conn: &mut SqliteConnection,
    result: &mut IdentityReductionResult,
) -> anyhow::Result<()> {
    let expected_link_type = match fields.decision.as_str() {
        "possible_same_identity" => "possible_same_identity",
        "not_same_identity" => "not_same_identity",
        _ => {
            result.claims_skipped.push(claim.id.clone());
            return Ok(());
        }
    };
    if fields.link_type != expected_link_type || claim.primary_source_span_id.trim().is_empty() {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    }

    let Some(entity_a_id) = claim.subject_entity_id.as_deref() else {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    };
    let Some(entity_b_id) = claim.object_entity_id.as_deref() else {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    };
    if entity_a_id == entity_b_id {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    }

    let entity_a = EntityRepo::get_by_id_with_conn(conn, entity_a_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_a not found: {}", entity_a_id))?;
    let entity_b = EntityRepo::get_by_id_with_conn(conn, entity_b_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("entity_b not found: {}", entity_b_id))?;
    if entity_a.entity_type != "character" || entity_b.entity_type != "character" {
        result.claims_skipped.push(claim.id.clone());
        return Ok(());
    }

    let link = IdentityRepo::find_or_create_identity_link_with_conn(
        conn,
        book_id,
        &entity_a.id,
        &entity_b.id,
        expected_link_type,
        fields.judge_confidence,
        &claim.id,
        "active",
    )
    .await?;
    result.identity_links_created.push(link.id);
    result.claims_accepted.push(claim.id.clone());

    Ok(())
}

fn select_identity_merge_survivor(
    entity_a: &crate::storage::db::v4::entity_repo::EntityRecord,
    entity_b: &crate::storage::db::v4::entity_repo::EntityRecord,
    fields: &IdentityJudgeFields,
) -> (
    crate::storage::db::v4::entity_repo::EntityRecord,
    crate::storage::db::v4::entity_repo::EntityRecord,
) {
    match fields.survivor_hint.as_str() {
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

/// Judge-normalized fields extracted from claim.value_json.
struct JudgeNormalizedFields {
    normalized_relation_group: String,
    normalized_relation_label: String,
    directionality: String,
    current_state: Option<String>,
    strength: f64,
    polarity: String,
    importance_score: f64,
    judge_confidence: f64,
}

fn extract_judge_normalized_fields(claim: &ClaimRecord) -> Option<JudgeNormalizedFields> {
    let value_json = claim.value_json.as_ref()?;
    let parsed: serde_json::Value = serde_json::from_str(value_json).ok()?;

    let group = parsed
        .get("normalized_relation_group")
        .and_then(|v| v.as_str())?
        .to_string();
    let label = parsed
        .get("normalized_relation_label")
        .and_then(|v| v.as_str())?
        .to_string();
    let directionality = parsed
        .get("directionality")
        .and_then(|v| v.as_str())?
        .to_string();
    let current_state = parsed
        .get("current_state")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let strength = parsed
        .get("strength")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let polarity = parsed
        .get("polarity")
        .and_then(|v| v.as_str())
        .unwrap_or("neutral")
        .to_string();
    let importance_score = parsed
        .get("importance_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let judge_confidence = parsed
        .get("judge_confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(claim.confidence);

    Some(JudgeNormalizedFields {
        normalized_relation_group: group,
        normalized_relation_label: label,
        directionality,
        current_state,
        strength,
        polarity,
        importance_score,
        judge_confidence,
    })
}

/// Reduce relationship_update claims into canonical relationships.
///
/// Uses its OWN transaction, independent from entity/property reducer.
/// Relationship failure does NOT roll back Phase 1 canonical state.
///
/// Input: only claims with status='proposed' AND claim_type='relationship_update'.
pub async fn reduce_relationship_claims(
    claims: &[ClaimRecord],
    book_id: &str,
    pool: &SqlitePool,
) -> anyhow::Result<RelationshipReductionResult> {
    let mut result = RelationshipReductionResult::default();

    // Filter to proposed relationship_update claims
    let reducible: Vec<&ClaimRecord> = claims
        .iter()
        .filter(|c| c.status == "proposed" && c.claim_type == "relationship_update")
        .collect();

    if reducible.is_empty() {
        return Ok(result);
    }

    // Start independent transaction
    let mut tx = pool.begin().await?;

    for claim in &reducible {
        match reduce_single_relationship_claim(claim, book_id, &mut *tx, &mut result).await {
            Ok(()) => {}
            Err(e) => {
                // Log the error, skip this claim, continue with others
                tracing::warn!("Relationship claim {} failed: {}. Skipping.", claim.id, e);
                result.claims_skipped.push(claim.id.clone());
            }
        }
    }

    // Mark accepted claims
    if !result.claims_accepted.is_empty() {
        ClaimRepo::batch_update_claim_status_with_conn(
            &mut *tx,
            &result.claims_accepted,
            "accepted",
        )
        .await?;
    }

    // Commit independent transaction
    tx.commit().await?;

    Ok(result)
}

async fn reduce_single_relationship_claim(
    claim: &ClaimRecord,
    book_id: &str,
    conn: &mut SqliteConnection,
    result: &mut RelationshipReductionResult,
) -> anyhow::Result<()> {
    // Extract Judge normalized fields from claim.value_json
    let fields = extract_judge_normalized_fields(claim).ok_or_else(|| {
        anyhow::anyhow!(
            "Missing or invalid Judge normalized fields in claim {}",
            claim.id
        )
    })?;

    // Validate subject_entity_id exists and entity_type='character'
    let subject_id = claim
        .subject_entity_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing subject_entity_id for claim {}", claim.id))?;
    let subject = EntityRepo::get_by_id_with_conn(conn, subject_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Subject entity {} not found", subject_id))?;
    if subject.entity_type != "character" {
        return Err(anyhow::anyhow!(
            "Subject entity {} is not a character (type={})",
            subject_id,
            subject.entity_type
        ));
    }

    // Validate object_entity_id exists and entity_type='character'
    let object_id = claim
        .object_entity_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing object_entity_id for claim {}", claim.id))?;
    let object = EntityRepo::get_by_id_with_conn(conn, object_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Object entity {} not found", object_id))?;
    if object.entity_type != "character" {
        return Err(anyhow::anyhow!(
            "Object entity {} is not a character (type={})",
            object_id,
            object.entity_type
        ));
    }

    // Canonicalize pair for undirected: sort by entity_id (min as subject, max as object)
    let (canonical_subject, canonical_object) = if fields.directionality == "undirected" {
        if subject_id < object_id {
            (subject_id.clone(), object_id.clone())
        } else {
            (object_id.clone(), subject_id.clone())
        }
    } else {
        (subject_id.clone(), object_id.clone())
    };

    // Check for existing relationship with same pair + same group + same directionality
    let existing = RelationshipRepo::find_existing_with_conn(
        conn,
        book_id,
        &canonical_subject,
        &canonical_object,
        &fields.normalized_relation_group,
        &fields.directionality,
    )
    .await?;

    let (relationship_id, is_new) = if let Some(rel) = existing {
        // Update existing relationship
        let new_confidence = rel.confidence.max(fields.judge_confidence);
        let new_importance = rel.importance_score.max(fields.importance_score);
        let changed = fields.current_state.as_deref() != rel.current_state.as_deref()
            || (fields.strength - rel.strength).abs() > f64::EPSILON
            || fields.polarity != rel.polarity;
        let last_changed = if changed {
            claim.chapter_index
        } else {
            rel.last_changed_chapter
        };

        RelationshipRepo::update_relationship_with_conn(
            conn,
            &rel.id,
            &fields.normalized_relation_label,
            fields.current_state.as_deref(),
            fields.strength,
            &fields.polarity,
            new_confidence,
            new_importance,
            last_changed,
            claim.chapter_index,
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
        .bind(book_id)
        .bind(&canonical_subject)
        .bind(&canonical_object)
        .bind(&fields.normalized_relation_group)
        .fetch_optional(&mut *conn)
        .await?;

        if any_existing.is_some() {
            // Same pair + same group + different directionality → skip
            result.claims_skipped.push(claim.id.clone());
            return Ok(());
        }

        // Create new relationship
        let new_rel = RelationshipRepo::create_relationship_with_conn(
            conn,
            book_id,
            &canonical_subject,
            &canonical_object,
            &fields.normalized_relation_group,
            &fields.normalized_relation_label,
            &fields.directionality,
            fields.current_state.as_deref(),
            fields.strength,
            &fields.polarity,
            fields.judge_confidence,
            fields.importance_score,
            claim.chapter_index,
        )
        .await?;

        result.relationships_created.push(new_rel.id.clone());
        (new_rel.id, true)
    };

    // Create relationship event
    RelationshipEventRepo::create_event_with_conn(
        conn,
        book_id,
        &relationship_id,
        if is_new { "creation" } else { "update" },
        &fields.normalized_relation_group,
        &fields.normalized_relation_label,
        fields.current_state.as_deref(),
        Some(fields.strength),
        Some(&fields.polarity),
        claim.chapter_index,
        &claim.id,
        fields.judge_confidence,
    )
    .await?;

    result.events_created.push(claim.id.clone());
    result.claims_accepted.push(claim.id.clone());

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
    async fn reduce_entity_introduction_creates_entity() {
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

        // Create a claim
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "entity_introduction",
                Some("张三"),
                None,
                None,
                None,
                "is a character",
                Some(r#"{"short_summary":"主角","aliases":["小张"]}"#),
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "low",
            )
            .await
            .unwrap();

        let claims = vec![claim];
        let result = reduce_claims(&claims, "b1", &pool).await.unwrap();

        assert_eq!(result.entities_created.len(), 1);
        assert_eq!(result.claims_accepted.len(), 1);

        // Verify entity exists (using pool directly for read)
        let entity_repo = EntityRepo::new(pool.clone());
        let entity = entity_repo
            .find_entity_by_alias("b1", "张三")
            .await
            .unwrap();
        assert!(entity.is_some());
        assert_eq!(entity.unwrap().canonical_name, "张三");
    }

    #[tokio::test]
    async fn reduce_property_update_replace() {
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

        // Create entity first
        let entity_repo = EntityRepo::new(pool.clone());
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // Create property_update claim
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "property_update",
                Some("张三"),
                None,
                Some(&entity.id),
                None,
                "realm = 筑基期",
                Some("筑基期"),
                None,
                &span_id.0,
                &run_id.0,
                0.85,
                "low",
            )
            .await
            .unwrap();

        let claims = vec![claim];
        let result = reduce_claims(&claims, "b1", &pool).await.unwrap();

        assert_eq!(result.properties_inserted.len(), 1);
        assert_eq!(result.claims_accepted.len(), 1);

        // Verify property exists
        let property_repo = PropertyRepo::new(pool.clone());
        let current = property_repo
            .get_current_property("b1", &entity.id, "realm")
            .await
            .unwrap();
        assert!(current.is_some());
        assert_eq!(current.unwrap().value_text.as_deref(), Some("筑基期"));
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
    async fn reduce_alias_creates_alias() {
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

        // Create entity first
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // Create alias claim
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "alias",
                Some("张三"),
                Some("小张"),
                Some(&entity.id),
                None,
                "also known as 小张",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.85,
                "low",
            )
            .await
            .unwrap();

        let claims = vec![claim];
        let result = reduce_claims(&claims, "b1", &pool).await.unwrap();

        assert_eq!(result.aliases_created.len(), 1);
        assert_eq!(result.claims_accepted.len(), 1);

        // Verify alias exists
        let found = entity_repo
            .find_entity_by_alias("b1", "小张")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, entity.id);

        // Verify last_seen_chapter was updated
        let updated_entity = entity_repo.get_by_id(&entity.id).await.unwrap().unwrap();
        assert_eq!(updated_entity.last_seen_chapter, 1);
    }

    #[tokio::test]
    async fn reduce_alias_updates_last_seen_chapter() {
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

        // Create entity at chapter 1
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();
        assert_eq!(entity.last_seen_chapter, 1);

        // Alias claim at chapter 5
        let claim = claim_repo
            .create_claim(
                "b1",
                5,
                "alias",
                Some("张三"),
                Some("张真人"),
                Some(&entity.id),
                None,
                "also known as 张真人",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "low",
            )
            .await
            .unwrap();

        let claims = vec![claim];
        let result = reduce_claims(&claims, "b1", &pool).await.unwrap();
        assert_eq!(result.claims_accepted.len(), 1);

        // Verify last_seen_chapter updated to 5
        let updated = entity_repo.get_by_id(&entity.id).await.unwrap().unwrap();
        assert_eq!(
            updated.last_seen_chapter, 5,
            "last_seen_chapter should be updated by alias claim"
        );
    }

    #[tokio::test]
    async fn reduce_property_update_append() {
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

        // Create entity
        let entity = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.9, 1)
            .await
            .unwrap();

        // First ability claim
        let claim1 = claim_repo
            .create_claim(
                "b1",
                1,
                "property_update",
                Some("张三"),
                None,
                Some(&entity.id),
                None,
                "ability = 剑法",
                Some("剑法"),
                None,
                &span_id.0,
                &run_id.0,
                0.85,
                "low",
            )
            .await
            .unwrap();

        // Second ability claim
        let claim2 = claim_repo
            .create_claim(
                "b1",
                2,
                "property_update",
                Some("张三"),
                None,
                Some(&entity.id),
                None,
                "ability = 拳法",
                Some("拳法"),
                None,
                &span_id.0,
                &run_id.0,
                0.8,
                "low",
            )
            .await
            .unwrap();

        let claims = vec![claim1, claim2];
        let result = reduce_claims(&claims, "b1", &pool).await.unwrap();

        assert_eq!(result.properties_inserted.len(), 2);
        assert_eq!(result.claims_accepted.len(), 2);

        // Verify append aggregation
        let property_repo = PropertyRepo::new(pool.clone());
        let current = property_repo
            .get_current_property("b1", &entity.id, "ability")
            .await
            .unwrap();
        assert!(current.is_some());
        let current = current.unwrap();
        // value_json should contain aggregated list
        assert!(current.value_json.is_some());
        let json: serde_json::Value = serde_json::from_str(&current.value_json.unwrap()).unwrap();
        assert!(json.as_array().unwrap().len() >= 1);
    }

    // --- Relationship Reducer Tests ---

    /// Helper: build a relationship_update claim value_json with Judge normalized fields.
    fn judge_value_json(
        group: &str,
        label: &str,
        directionality: &str,
        current_state: Option<&str>,
        strength: f64,
        polarity: &str,
        importance_score: f64,
        judge_confidence: f64,
    ) -> String {
        serde_json::json!({
            "normalized_relation_group": group,
            "normalized_relation_label": label,
            "directionality": directionality,
            "current_state": current_state,
            "strength": strength,
            "polarity": polarity,
            "importance_score": importance_score,
            "judge_confidence": judge_confidence,
            // Extractor hints should be ignored by reducer
            "relation_hint": "SHOULD_BE_IGNORED",
            "relation_group": "SHOULD_BE_IGNORED",
        })
        .to_string()
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
                Some(&judge_value_json(
                    "friendship",
                    "friends",
                    "undirected",
                    None,
                    0.7,
                    "positive",
                    0.8,
                    0.9,
                )),
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool)
            .await
            .unwrap();

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
                Some(&judge_value_json(
                    "friendship",
                    "friends",
                    "undirected",
                    Some("close"),
                    0.7,
                    "positive",
                    0.8,
                    0.9,
                )),
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let result1 = reduce_relationship_claims(&[claim1], "b1", &pool)
            .await
            .unwrap();
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
                Some(&judge_value_json(
                    "friendship",
                    "best friends",
                    "undirected",
                    Some("very close"),
                    0.9,
                    "positive",
                    0.9,
                    0.95,
                )),
                &span_id.0,
                &run_id.0,
                0.95,
                "high",
            )
            .await
            .unwrap();

        let result2 = reduce_relationship_claims(&[claim2], "b1", &pool)
            .await
            .unwrap();
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
                Some(&judge_value_json(
                    "friendship",
                    "friends",
                    "undirected",
                    None,
                    0.7,
                    "positive",
                    0.8,
                    0.9,
                )),
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
                Some(&judge_value_json(
                    "rivalry",
                    "rivals",
                    "directed",
                    Some("competitive"),
                    0.6,
                    "negative",
                    0.7,
                    0.85,
                )),
                &span_id.0,
                &run_id.0,
                0.85,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_relationship_claims(&[claim1, claim2], "b1", &pool)
            .await
            .unwrap();
        assert_eq!(
            result.relationships_created.len(),
            2,
            "different groups should coexist"
        );
        assert_eq!(result.claims_accepted.len(), 2);

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
                Some(&judge_value_json(
                    "mentorship",
                    "mentor",
                    "directed",
                    Some("teaching"),
                    0.8,
                    "positive",
                    0.9,
                    0.95,
                )),
                &span_id.0,
                &run_id.0,
                0.95,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool)
            .await
            .unwrap();
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
                Some(&judge_value_json(
                    "friendship",
                    "friends",
                    "undirected",
                    None,
                    0.7,
                    "positive",
                    0.8,
                    0.9,
                )),
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool)
            .await
            .unwrap();
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
                Some(&judge_value_json(
                    "hierarchy",
                    "belongs to",
                    "directed",
                    None,
                    0.5,
                    "neutral",
                    0.5,
                    0.8,
                )),
                &span_id.0,
                &run_id.0,
                0.8,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool)
            .await
            .unwrap();

        // Should be skipped, not create a relationship
        assert!(result.relationships_created.is_empty());
        assert!(result.claims_accepted.is_empty());
        assert_eq!(result.claims_skipped.len(), 1);

        // Verify claim still proposed (not accepted)
        let claim_repo2 = ClaimRepo::new(pool.clone());
        let fetched = claim_repo2
            .get_claim(&result.claims_skipped[0])
            .await
            .unwrap()
            .unwrap();
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
                Some(&judge_value_json(
                    "other_social",
                    "practices",
                    "undirected",
                    None,
                    0.3,
                    "neutral",
                    0.3,
                    0.7,
                )),
                &span_id.0,
                &run_id.0,
                0.7,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool)
            .await
            .unwrap();

        assert!(result.relationships_created.is_empty());
        assert!(result.claims_accepted.is_empty());
        assert_eq!(result.claims_skipped.len(), 1);
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

        // First: run entity_reducer to create an entity (simulating Phase 1)
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
        let entity_result = reduce_claims(&[entity_claim], "b1", &pool).await.unwrap();
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
                Some(&judge_value_json(
                    "friendship",
                    "friends",
                    "undirected",
                    None,
                    0.5,
                    "neutral",
                    0.5,
                    0.8,
                )),
                &span_id.0,
                &run_id.0,
                0.8,
                "high",
            )
            .await
            .unwrap();

        let rel_result = reduce_relationship_claims(&[bad_rel_claim], "b1", &pool)
            .await
            .unwrap();

        // Relationship failed (skipped)
        assert!(rel_result.relationships_created.is_empty());
        assert_eq!(rel_result.claims_skipped.len(), 1);

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
    async fn rel_reducer_claims_marked_accepted_after_success() {
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
                Some(&judge_value_json(
                    "friendship",
                    "friends",
                    "undirected",
                    None,
                    0.7,
                    "positive",
                    0.8,
                    0.9,
                )),
                &span_id.0,
                &run_id.0,
                0.9,
                "high",
            )
            .await
            .unwrap();

        // Verify claim starts as proposed
        assert_eq!(claim.status, "proposed");

        let result = reduce_relationship_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();
        assert_eq!(result.claims_accepted.len(), 1);
        assert_eq!(result.claims_accepted[0], claim.id);

        // Verify claim status is now accepted in DB
        let claim_repo2 = ClaimRepo::new(pool.clone());
        let fetched = claim_repo2.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, "accepted");
    }

    #[tokio::test]
    async fn rel_reducer_reads_judge_fields_not_extractor_hints() {
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

        // value_json has DIFFERENT extractor hints vs judge normalized fields
        let value_json = serde_json::json!({
            // Judge normalized fields (these should be used)
            "normalized_relation_group": "mentorship",
            "normalized_relation_label": "master",
            "directionality": "directed",
            "current_state": "teaching",
            "strength": 0.9,
            "polarity": "positive",
            "importance_score": 0.95,
            "judge_confidence": 0.95,
            // Extractor hints (these should be IGNORED)
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

        let result = reduce_relationship_claims(&[claim], "b1", &pool)
            .await
            .unwrap();
        assert_eq!(result.relationships_created.len(), 1);

        // Verify the relationship uses JUDGE fields, not extractor hints
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo
            .get_by_id(&result.relationships_created[0])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            rel.relation_group, "mentorship",
            "should use judge normalized_relation_group, not extractor relation_group"
        );
        assert_eq!(
            rel.relation_label, "master",
            "should use judge normalized_relation_label, not extractor relation_label"
        );
        assert_eq!(rel.directionality, "directed");
        assert!(
            (rel.confidence - 0.95).abs() < f64::EPSILON,
            "should use judge_confidence"
        );
        assert!(
            (rel.importance_score - 0.95).abs() < f64::EPSILON,
            "should use judge importance_score"
        );
    }

    #[tokio::test]
    async fn rel_reducer_skips_non_relationship_claims() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, _) = create_two_characters(&pool, &entity_repo).await;

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

        // entity_introduction claim - should be ignored by relationship_reducer
        let claim = claim_repo
            .create_claim(
                "b1",
                1,
                "entity_introduction",
                Some("张三"),
                None,
                Some(&subject_id),
                None,
                "is a character",
                None,
                None,
                &span_id.0,
                &run_id.0,
                0.9,
                "low",
            )
            .await
            .unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool)
            .await
            .unwrap();
        assert!(result.relationships_created.is_empty());
        assert!(result.claims_accepted.is_empty());
        // No claims should be processed (not even skipped - they're filtered out)
        assert!(result.claims_skipped.is_empty());
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
                Some(
                    r#"{"judge_decision":"merge","link_type":"same_identity","survivor_hint":"entity_b","judge_confidence":0.96,"reason_code":"explicit_reveal"}"#,
                ),
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        reduce_identity_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();

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

        let stored_claim = claim_repo.get_claim(&claim.id).await.unwrap().unwrap();
        assert_eq!(stored_claim.status, "accepted");
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
                Some(
                    r#"{"judge_decision":"merge","link_type":"same_identity","survivor_hint":"entity_b","judge_confidence":0.96,"reason_code":"explicit_reveal"}"#,
                ),
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        reduce_identity_claims(&[merge_claim], "b1", &pool)
            .await
            .unwrap();

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
                Some(
                    r#"{"judge_decision":"possible_same_identity","link_type":"possible_same_identity","survivor_hint":"unknown","judge_confidence":0.62,"reason_code":"weak_similarity"}"#,
                ),
                &span_id.0,
                &run_id.0,
                0.62,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_identity_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();

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
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "accepted"
        );
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
                Some(
                    r#"{"judge_decision":"not_same_identity","link_type":"not_same_identity","survivor_hint":"unknown","judge_confidence":0.98,"reason_code":"explicit_not_same"}"#,
                ),
                &span_id.0,
                &run_id.0,
                0.98,
                "high",
            )
            .await
            .unwrap();

        let result = reduce_identity_claims(&[claim.clone()], "b1", &pool)
            .await
            .unwrap();

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
        assert_eq!(
            claim_repo
                .get_claim(&claim.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "accepted"
        );
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
                Some(
                    r#"{"judge_decision":"merge","link_type":"same_identity","survivor_hint":"entity_b","judge_confidence":0.96,"reason_code":"explicit_reveal"}"#,
                ),
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        reduce_identity_claims(&[merge_claim], "b1", &pool)
            .await
            .unwrap();

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
                Some(
                    r#"{"judge_decision":"merge","link_type":"same_identity","survivor_hint":"entity_b","judge_confidence":0.96,"reason_code":"explicit_reveal"}"#,
                ),
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        reduce_identity_claims(&[merge_claim], "b1", &pool)
            .await
            .unwrap();

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
                Some(
                    r#"{"judge_decision":"merge","link_type":"same_identity","survivor_hint":"entity_b","judge_confidence":0.96,"reason_code":"explicit_reveal"}"#,
                ),
                &span_id.0,
                &run_id.0,
                0.96,
                "high",
            )
            .await
            .unwrap();

        reduce_identity_claims(&[merge_claim], "b1", &pool)
            .await
            .unwrap();

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
