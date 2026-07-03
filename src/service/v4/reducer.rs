use crate::storage::db::v4::claim_repo::{ClaimRecord, ClaimRepo};
use crate::storage::db::v4::entity_repo::EntityRepo;
use crate::storage::db::v4::property_repo::PropertyRepo;
use crate::storage::db::v4::relationship_repo::{
    RelationshipEventRepo, RelationshipRepo,
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
                tracing::warn!(
                    "Relationship claim {} failed: {}. Skipping.",
                    claim.id,
                    e
                );
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
    let subject_id = claim.subject_entity_id.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Missing subject_entity_id for claim {}", claim.id)
    })?;
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
    let object_id = claim.object_entity_id.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Missing object_entity_id for claim {}", claim.id)
    })?;
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
    async fn create_two_characters(_pool: &SqlitePool, entity_repo: &EntityRepo) -> (String, String) {
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
    async fn create_non_character(_pool: &SqlitePool, entity_repo: &EntityRepo, name: &str, entity_type: &str) -> String {
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

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        let claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship",
            None,
            Some(&judge_value_json("friendship", "friends", "undirected", None, 0.7, "positive", 0.8, 0.9)),
            &span_id.0, &run_id.0, 0.9, "high",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool).await.unwrap();

        assert_eq!(result.relationships_created.len(), 1);
        assert_eq!(result.events_created.len(), 1);
        assert_eq!(result.claims_accepted.len(), 1);
        assert!(result.claims_skipped.is_empty());

        // Verify the relationship exists and is canonicalized
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo.get_by_id(&result.relationships_created[0]).await.unwrap().unwrap();
        assert_eq!(rel.relation_group, "friendship");
        assert_eq!(rel.directionality, "undirected");
        // Undirected: smaller ID should be subject
        let (expected_sub, expected_obj) = if subject_id < object_id { (&subject_id, &object_id) } else { (&object_id, &subject_id) };
        assert_eq!(&rel.subject_character_id, expected_sub);
        assert_eq!(&rel.object_character_id, expected_obj);
    }

    #[tokio::test]
    async fn rel_reducer_updates_existing_relationship() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // First claim: create
        let claim1 = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship", None,
            Some(&judge_value_json("friendship", "friends", "undirected", Some("close"), 0.7, "positive", 0.8, 0.9)),
            &span_id.0, &run_id.0, 0.9, "high",
        ).await.unwrap();

        let result1 = reduce_relationship_claims(&[claim1], "b1", &pool).await.unwrap();
        assert_eq!(result1.relationships_created.len(), 1);

        // Second claim: update same pair + same group
        let claim2 = claim_repo.create_claim(
            "b1", 5, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship", None,
            Some(&judge_value_json("friendship", "best friends", "undirected", Some("very close"), 0.9, "positive", 0.9, 0.95)),
            &span_id.0, &run_id.0, 0.95, "high",
        ).await.unwrap();

        let result2 = reduce_relationship_claims(&[claim2], "b1", &pool).await.unwrap();
        assert_eq!(result2.relationships_updated.len(), 1);
        assert_eq!(result2.relationships_created.len(), 0);
        assert_eq!(result2.claims_accepted.len(), 1);

        // Verify updated fields
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo.get_by_id(&result2.relationships_updated[0]).await.unwrap().unwrap();
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

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // friendship
        let claim1 = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship", None,
            Some(&judge_value_json("friendship", "friends", "undirected", None, 0.7, "positive", 0.8, 0.9)),
            &span_id.0, &run_id.0, 0.9, "high",
        ).await.unwrap();

        // rivalry (different group)
        let claim2 = claim_repo.create_claim(
            "b1", 2, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship", None,
            Some(&judge_value_json("rivalry", "rivals", "directed", Some("competitive"), 0.6, "negative", 0.7, 0.85)),
            &span_id.0, &run_id.0, 0.85, "high",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim1, claim2], "b1", &pool).await.unwrap();
        assert_eq!(result.relationships_created.len(), 2, "different groups should coexist");
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

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // Directed: 张三 → 李四 (mentor)
        let claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship", None,
            Some(&judge_value_json("mentorship", "mentor", "directed", Some("teaching"), 0.8, "positive", 0.9, 0.95)),
            &span_id.0, &run_id.0, 0.95, "high",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool).await.unwrap();
        assert_eq!(result.relationships_created.len(), 1);

        // Verify direction preserved (NOT canonicalized)
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo.get_by_id(&result.relationships_created[0]).await.unwrap().unwrap();
        assert_eq!(rel.subject_character_id, subject_id);
        assert_eq!(rel.object_character_id, object_id);
        assert_eq!(rel.directionality, "directed");
    }

    #[tokio::test]
    async fn rel_reducer_undirected_canonicalizes_pair() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (e1, e2) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // Pass with reversed order: larger ID as subject
        let (larger, smaller) = if e1 > e2 { (&e1, &e2) } else { (&e2, &e1) };
        let claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("李四"), Some("张三"),
            Some(larger), Some(smaller),
            "relationship", None,
            Some(&judge_value_json("friendship", "friends", "undirected", None, 0.7, "positive", 0.8, 0.9)),
            &span_id.0, &run_id.0, 0.9, "high",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool).await.unwrap();
        assert_eq!(result.relationships_created.len(), 1);

        // Verify canonicalized: smaller ID as subject
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo.get_by_id(&result.relationships_created[0]).await.unwrap().unwrap();
        assert_eq!(&rel.subject_character_id, smaller, "undirected: smaller ID should be subject");
        assert_eq!(&rel.object_character_id, larger, "undirected: larger ID should be object");
    }

    #[tokio::test]
    async fn rel_reducer_subject_not_character_skipped() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let place_id = create_non_character(&pool, &entity_repo, "青云门", "place").await;
        let (char_id, _) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // Subject is place, not character
        let claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("青云门"), Some("张三"),
            Some(&place_id), Some(&char_id),
            "relationship", None,
            Some(&judge_value_json("hierarchy", "belongs to", "directed", None, 0.5, "neutral", 0.5, 0.8)),
            &span_id.0, &run_id.0, 0.8, "high",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool).await.unwrap();

        // Should be skipped, not create a relationship
        assert!(result.relationships_created.is_empty());
        assert!(result.claims_accepted.is_empty());
        assert_eq!(result.claims_skipped.len(), 1);

        // Verify claim still proposed (not accepted)
        let claim_repo2 = ClaimRepo::new(pool.clone());
        let fetched = claim_repo2.get_claim(&result.claims_skipped[0]).await.unwrap().unwrap();
        assert_eq!(fetched.status, "proposed");
    }

    #[tokio::test]
    async fn rel_reducer_object_not_character_skipped() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (char_id, _) = create_two_characters(&pool, &entity_repo).await;
        let ability_id = create_non_character(&pool, &entity_repo, "剑法", "ability").await;

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // Object is ability, not character
        let claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("张三"), Some("剑法"),
            Some(&char_id), Some(&ability_id),
            "relationship", None,
            Some(&judge_value_json("other_social", "practices", "undirected", None, 0.3, "neutral", 0.3, 0.7)),
            &span_id.0, &run_id.0, 0.7, "high",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool).await.unwrap();

        assert!(result.relationships_created.is_empty());
        assert!(result.claims_accepted.is_empty());
        assert_eq!(result.claims_skipped.len(), 1);
    }

    #[tokio::test]
    async fn rel_reducer_independent_transaction_no_rollback() {
        // Verify that relationship_reducer failure does NOT affect entity/property state
        let (pool, claim_repo, entity_repo, _) = setup().await;

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // First: run entity_reducer to create an entity (simulating Phase 1)
        let entity_claim = claim_repo.create_claim(
            "b1", 1, "entity_introduction",
            Some("王五"), None, None, None,
            "is a character",
            Some(r#"{"short_summary":"新角色","aliases":[]}"#), None,
            &span_id.0, &run_id.0, 0.9, "low",
        ).await.unwrap();
        let entity_result = reduce_claims(&[entity_claim], "b1", &pool).await.unwrap();
        assert_eq!(entity_result.entities_created.len(), 1);

        // Now: run relationship_reducer with a claim that will fail (missing entity)
        let bad_rel_claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("王五"), Some("不存在"),
            Some("nonexistent_id_1"), Some("nonexistent_id_2"),
            "relationship", None,
            Some(&judge_value_json("friendship", "friends", "undirected", None, 0.5, "neutral", 0.5, 0.8)),
            &span_id.0, &run_id.0, 0.8, "high",
        ).await.unwrap();

        let rel_result = reduce_relationship_claims(&[bad_rel_claim], "b1", &pool).await.unwrap();

        // Relationship failed (skipped)
        assert!(rel_result.relationships_created.is_empty());
        assert_eq!(rel_result.claims_skipped.len(), 1);

        // But entity from Phase 1 is still there
        let entity = entity_repo.get_by_canonical_name("b1", "王五").await.unwrap();
        assert!(entity.is_some(), "Phase 1 entity should NOT be rolled back by relationship_reducer failure");
        assert_eq!(entity.unwrap().canonical_name, "王五");
    }

    #[tokio::test]
    async fn rel_reducer_claims_marked_accepted_after_success() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, object_id) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        let claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship", None,
            Some(&judge_value_json("friendship", "friends", "undirected", None, 0.7, "positive", 0.8, 0.9)),
            &span_id.0, &run_id.0, 0.9, "high",
        ).await.unwrap();

        // Verify claim starts as proposed
        assert_eq!(claim.status, "proposed");

        let result = reduce_relationship_claims(&[claim.clone()], "b1", &pool).await.unwrap();
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

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

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
        }).to_string();

        let claim = claim_repo.create_claim(
            "b1", 1, "relationship_update",
            Some("张三"), Some("李四"),
            Some(&subject_id), Some(&object_id),
            "relationship", None,
            Some(&value_json),
            &span_id.0, &run_id.0, 0.5, "high",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool).await.unwrap();
        assert_eq!(result.relationships_created.len(), 1);

        // Verify the relationship uses JUDGE fields, not extractor hints
        let rel_repo = RelationshipRepo::new(pool.clone());
        let rel = rel_repo.get_by_id(&result.relationships_created[0]).await.unwrap().unwrap();
        assert_eq!(rel.relation_group, "mentorship", "should use judge normalized_relation_group, not extractor relation_group");
        assert_eq!(rel.relation_label, "master", "should use judge normalized_relation_label, not extractor relation_label");
        assert_eq!(rel.directionality, "directed");
        assert!((rel.confidence - 0.95).abs() < f64::EPSILON, "should use judge_confidence");
        assert!((rel.importance_score - 0.95).abs() < f64::EPSILON, "should use judge importance_score");
    }

    #[tokio::test]
    async fn rel_reducer_skips_non_relationship_claims() {
        let (pool, claim_repo, entity_repo, _) = setup().await;
        let (subject_id, _) = create_two_characters(&pool, &entity_repo).await;

        let span_id: (String,) = sqlx::query_as("SELECT id FROM source_spans WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();
        let run_id: (String,) = sqlx::query_as("SELECT id FROM ai_runs WHERE book_id = 'b1' LIMIT 1")
            .fetch_one(&pool).await.unwrap();

        // entity_introduction claim - should be ignored by relationship_reducer
        let claim = claim_repo.create_claim(
            "b1", 1, "entity_introduction",
            Some("张三"), None,
            Some(&subject_id), None,
            "is a character",
            None, None,
            &span_id.0, &run_id.0, 0.9, "low",
        ).await.unwrap();

        let result = reduce_relationship_claims(&[claim], "b1", &pool).await.unwrap();
        assert!(result.relationships_created.is_empty());
        assert!(result.claims_accepted.is_empty());
        // No claims should be processed (not even skipped - they're filtered out)
        assert!(result.claims_skipped.is_empty());
    }
}
