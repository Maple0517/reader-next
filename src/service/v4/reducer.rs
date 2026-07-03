use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::property_repo::PropertyRepo;
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
    let (short_summary, aliases) = extract_entity_info(claim);

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
        match EntityRepo::create_entity_with_conn(
            conn,
            book_id,
            "character", // Default to character for now
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
                let existing = EntityRepo::get_by_canonical_name_with_conn(
                    conn,
                    book_id,
                    subject_mention,
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!(
                    "Entity creation failed and lookup by canonical name returned nothing for '{}'",
                    subject_mention
                ))?;
                EntityRepo::update_last_seen_with_conn(conn, &existing.id, claim.chapter_index).await?;
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

/// Extract entity info (short_summary, aliases) from claim value_json.
fn extract_entity_info(claim: &ClaimRecord) -> (Option<String>, Vec<String>) {
    let value_json = match &claim.value_json {
        Some(j) => j,
        None => return (None, Vec::new()),
    };

    let parsed: serde_json::Value = match serde_json::from_str(value_json) {
        Ok(v) => v,
        Err(_) => return (None, Vec::new()),
    };

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

    (short_summary, aliases)
}

/// Extract dimension key from predicate string like "realm = 筑基期".
fn extract_dimension_from_predicate(predicate: &str) -> Option<String> {
    predicate.split('=').next().map(|s| s.trim().to_string())
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
        assert_eq!(updated.last_seen_chapter, 5, "last_seen_chapter should be updated by alias claim");
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
}
