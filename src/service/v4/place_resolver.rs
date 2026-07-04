use crate::storage::db::v4::entity_repo::{normalize_name, EntityRecord, EntityRepo};
use crate::storage::db::v4::identity_repo::IdentityRepo;

const LOW_CONFIDENCE_THRESHOLD: f64 = 0.35;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceResolutionAction {
    UseExistingPlace,
    CreateNewPlace,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceResolution {
    pub action: PlaceResolutionAction,
    pub place_entity_id: Option<String>,
    pub canonical_name: String,
    pub display_name: String,
    pub place_type: String,
    pub parent_place_id: Option<String>,
    pub organization_link_candidate_id: Option<String>,
    pub confidence: f64,
    pub reason: String,
}

pub struct PlaceResolver {
    entity_repo: EntityRepo,
    identity_repo: IdentityRepo,
}

impl PlaceResolver {
    pub fn new(entity_repo: EntityRepo, identity_repo: IdentityRepo) -> Self {
        Self {
            entity_repo,
            identity_repo,
        }
    }

    pub async fn resolve_place(
        &self,
        book_id: &str,
        place_mention: &str,
        place_type: &str,
        parent_place_mention: Option<&str>,
        aliases: &[String],
        confidence: f64,
    ) -> anyhow::Result<PlaceResolution> {
        let display_name = place_mention.trim().to_string();
        let canonical_name = normalize_name(place_mention);

        if confidence < LOW_CONFIDENCE_THRESHOLD || display_name.is_empty() {
            return Ok(PlaceResolution {
                action: PlaceResolutionAction::Uncertain,
                place_entity_id: None,
                canonical_name,
                display_name,
                place_type: place_type.to_string(),
                parent_place_id: None,
                organization_link_candidate_id: None,
                confidence,
                reason: "low confidence or empty place mention".to_string(),
            });
        }

        let parent_place_id = self
            .resolve_existing_place(book_id, parent_place_mention)
            .await?
            .map(|entity| entity.id);

        if let Some(place) = self
            .resolve_existing_place(book_id, Some(place_mention))
            .await?
        {
            return Ok(PlaceResolution {
                action: PlaceResolutionAction::UseExistingPlace,
                place_entity_id: Some(place.id),
                canonical_name: place.canonical_name,
                display_name: place.display_name,
                place_type: place_type.to_string(),
                parent_place_id,
                organization_link_candidate_id: None,
                confidence,
                reason: "matched existing place by name or alias".to_string(),
            });
        }

        for alias in aliases {
            if let Some(place) = self.resolve_existing_place(book_id, Some(alias)).await? {
                return Ok(PlaceResolution {
                    action: PlaceResolutionAction::UseExistingPlace,
                    place_entity_id: Some(place.id),
                    canonical_name: place.canonical_name,
                    display_name: place.display_name,
                    place_type: place_type.to_string(),
                    parent_place_id,
                    organization_link_candidate_id: None,
                    confidence,
                    reason: "matched existing place by supplied alias".to_string(),
                });
            }
        }

        let organization_link_candidate_id = self
            .entity_repo
            .get_by_canonical_name_and_type(book_id, "organization", place_mention)
            .await?
            .map(|entity| entity.id);

        Ok(PlaceResolution {
            action: PlaceResolutionAction::CreateNewPlace,
            place_entity_id: None,
            canonical_name,
            display_name,
            place_type: place_type.to_string(),
            parent_place_id,
            organization_link_candidate_id,
            confidence,
            reason: "no existing place matched".to_string(),
        })
    }

    async fn resolve_existing_place(
        &self,
        book_id: &str,
        mention: Option<&str>,
    ) -> anyhow::Result<Option<EntityRecord>> {
        let Some(mention) = mention.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(None);
        };

        let place = if let Some(place) = self
            .entity_repo
            .get_by_canonical_name_and_type(book_id, "place", mention)
            .await?
        {
            Some(place)
        } else {
            self.entity_repo
                .find_entity_by_alias_and_type(book_id, "place", mention)
                .await?
        };

        let Some(place) = place else {
            return Ok(None);
        };

        if let Some(target_id) = self
            .identity_repo
            .resolve_redirect_target(book_id, &place.id)
            .await?
        {
            let target = self.entity_repo.get_by_id(&target_id).await?;
            return Ok(target.filter(|entity| {
                entity.book_id == book_id
                    && entity.entity_type == "place"
                    && entity.status == "active"
            }));
        }

        Ok(Some(place))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::claim_repo::ClaimRepo;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let database_url = "sqlite::memory:";
        let pool = db::init_pool(database_url).await.unwrap();
        crate::storage::db::v4::init_v4(&pool).await.unwrap();
        pool
    }

    fn resolver(pool: &SqlitePool) -> PlaceResolver {
        PlaceResolver::new(EntityRepo::new(pool.clone()), IdentityRepo::new(pool.clone()))
    }

    async fn setup_source_infra(pool: &SqlitePool, book_id: &str) -> (String, String) {
        let chapter_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES (?, ?, 1, 'test text', 'hash1', datetime('now'))")
            .bind(&chapter_id)
            .bind(book_id)
            .execute(pool)
            .await
            .unwrap();

        let segment_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES (?, ?, ?, 'hash1', 0, datetime('now'))")
            .bind(&segment_id)
            .bind(book_id)
            .bind(&chapter_id)
            .execute(pool)
            .await
            .unwrap();

        let span_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES (?, ?, ?, 'hash1', ?, 0, 0, 10, '青云城位于东域', datetime('now'))")
            .bind(&span_id)
            .bind(book_id)
            .bind(&chapter_id)
            .bind(&segment_id)
            .execute(pool)
            .await
            .unwrap();

        let run_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES (?, ?, ?, 'extract', 'test-model', 'v1', 1, 'inputhash', 'success', datetime('now'))")
            .bind(&run_id)
            .bind(book_id)
            .bind(&chapter_id)
            .execute(pool)
            .await
            .unwrap();

        (span_id, run_id)
    }

    async fn create_identity_claim(pool: &SqlitePool, book_id: &str) -> String {
        let (span_id, run_id) = setup_source_infra(pool, book_id).await;
        ClaimRepo::new(pool.clone())
            .create_claim(
                book_id,
                1,
                "identity_reveal",
                Some("旧青云城"),
                Some("青云城"),
                None,
                None,
                "same identity",
                None,
                None,
                &span_id,
                &run_id,
                0.95,
                "high",
            )
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn place_resolver_exact_match() {
        let pool = setup_test_db().await;
        let entity_repo = EntityRepo::new(pool.clone());
        let place = entity_repo
            .create_entity("book1", "place", "青云城", "青云城", None, 0.7, 1)
            .await
            .unwrap();

        let resolved = resolver(&pool)
            .resolve_place("book1", "青云城", "city", None, &[], 0.9)
            .await
            .unwrap();

        assert_eq!(resolved.action, PlaceResolutionAction::UseExistingPlace);
        assert_eq!(resolved.place_entity_id.as_deref(), Some(place.id.as_str()));
    }

    #[tokio::test]
    async fn place_resolver_alias_match() {
        let pool = setup_test_db().await;
        let entity_repo = EntityRepo::new(pool.clone());
        let place = entity_repo
            .create_entity("book1", "place", "青云城", "青云城", None, 0.7, 1)
            .await
            .unwrap();
        entity_repo
            .create_alias("book1", &place.id, "云城", "name", 1, 0.9, None)
            .await
            .unwrap();

        let resolved = resolver(&pool)
            .resolve_place("book1", "云城", "city", None, &[], 0.9)
            .await
            .unwrap();

        assert_eq!(resolved.action, PlaceResolutionAction::UseExistingPlace);
        assert_eq!(resolved.place_entity_id.as_deref(), Some(place.id.as_str()));
    }

    #[tokio::test]
    async fn organization_not_merged_with_place() {
        let pool = setup_test_db().await;
        let entity_repo = EntityRepo::new(pool.clone());
        let organization = entity_repo
            .create_entity(
                "book1",
                "organization",
                "青云门",
                "青云门",
                None,
                0.8,
                1,
            )
            .await
            .unwrap();

        let resolved = resolver(&pool)
            .resolve_place("book1", "青云门", "sect_site", None, &[], 0.85)
            .await
            .unwrap();

        assert_eq!(resolved.action, PlaceResolutionAction::CreateNewPlace);
        assert_ne!(
            resolved.place_entity_id.as_deref(),
            Some(organization.id.as_str())
        );
        assert_eq!(
            resolved.organization_link_candidate_id.as_deref(),
            Some(organization.id.as_str())
        );
    }

    #[tokio::test]
    async fn phase3_merged_victim_resolves_survivor_for_place_context() {
        let pool = setup_test_db().await;
        let entity_repo = EntityRepo::new(pool.clone());
        let victim = entity_repo
            .create_entity("book1", "place", "旧青云城", "旧青云城", None, 0.4, 1)
            .await
            .unwrap();
        let survivor = entity_repo
            .create_entity("book1", "place", "青云城", "青云城", None, 0.8, 2)
            .await
            .unwrap();
        let source_claim_id = create_identity_claim(&pool, "book1").await;
        let mut conn = pool.acquire().await.unwrap();
        IdentityRepo::find_or_create_identity_link_with_conn(
            &mut conn,
            "book1",
            &victim.id,
            &survivor.id,
            "redirect",
            0.99,
            &source_claim_id,
            "active",
        )
        .await
        .unwrap();

        let resolved = resolver(&pool)
            .resolve_place("book1", "旧青云城", "city", None, &[], 0.9)
            .await
            .unwrap();

        assert_eq!(resolved.action, PlaceResolutionAction::UseExistingPlace);
        assert_eq!(
            resolved.place_entity_id.as_deref(),
            Some(survivor.id.as_str())
        );
    }

    #[tokio::test]
    async fn place_resolver_low_confidence_uncertain() {
        let pool = setup_test_db().await;

        let resolved = resolver(&pool)
            .resolve_place("book1", "疑似古城", "unknown", None, &[], 0.2)
            .await
            .unwrap();

        assert_eq!(resolved.action, PlaceResolutionAction::Uncertain);
        assert!(resolved.place_entity_id.is_none());
    }

    #[tokio::test]
    async fn place_resolver_unresolved_parent_defers() {
        let pool = setup_test_db().await;

        let resolved = resolver(&pool)
            .resolve_place("book1", "青云城", "city", Some("东域"), &[], 0.8)
            .await
            .unwrap();

        assert_eq!(resolved.action, PlaceResolutionAction::CreateNewPlace);
        assert!(resolved.parent_place_id.is_none());
    }
}
