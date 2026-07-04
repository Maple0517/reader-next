use crate::storage::db::v4::entity_repo::{normalize_name, EntityRecord, EntityRepo};
use crate::storage::db::v4::knowledge_repo::{
    KnowledgeAssertionRecord, KnowledgeCardRecord, KnowledgeRepo,
};
use crate::storage::db::v4::place_repo::{
    MapLayoutSnapshotRecord, PlaceDetailRecord, PlaceEdgeConflictRecord, PlaceEdgeRecord, PlaceRepo,
};
use crate::storage::db::v4::quality_repo::{
    NewAuditFinding, NewAuditRun, NewUserCorrection, QualityAuditFindingRecord, QualityRepo,
    UserCorrectionRecord,
};
use crate::storage::db::v4::relationship_repo::{
    RelationshipEventRepo, RelationshipRecord, RelationshipRepo,
};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct AuditFindingDraft {
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
}

#[derive(Debug, Clone)]
pub struct AuditRunResult {
    pub run_id: String,
    pub status: String,
    pub finding_count: usize,
    pub error: Option<String>,
}

pub trait QualityAuditTask {
    fn audit_type(&self) -> &'static str;
    fn collect_findings(&self) -> anyhow::Result<Vec<AuditFindingDraft>>;
}

pub struct QualityAuditService {
    pool: SqlitePool,
}

impl QualityAuditService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn run_task<T: QualityAuditTask>(
        &self,
        book_id: &str,
        scope_json: &str,
        task: T,
    ) -> anyhow::Result<AuditRunResult> {
        match task.collect_findings() {
            Ok(findings) => {
                self.run_with_findings(book_id, task.audit_type(), scope_json, findings)
                    .await
            }
            Err(err) => {
                self.record_failed_run(book_id, task.audit_type(), scope_json, &err.to_string())
                    .await
            }
        }
    }

    pub async fn run_with_findings(
        &self,
        book_id: &str,
        audit_type: &str,
        scope_json: &str,
        findings: Vec<AuditFindingDraft>,
    ) -> anyhow::Result<AuditRunResult> {
        let mut tx = self.pool.begin().await?;
        let run = QualityRepo::create_audit_run_with_conn(
            &mut *tx,
            NewAuditRun {
                id: None,
                book_id,
                audit_type,
                scope_json,
                status: "running",
            },
        )
        .await?;

        for finding in &findings {
            QualityRepo::create_audit_finding_with_conn(
                &mut *tx,
                NewAuditFinding {
                    id: None,
                    book_id,
                    audit_run_id: &run.id,
                    finding_type: &finding.finding_type,
                    severity: &finding.severity,
                    target_type: &finding.target_type,
                    target_id: &finding.target_id,
                    related_target_type: finding.related_target_type.as_deref(),
                    related_target_id: finding.related_target_id.as_deref(),
                    reason_code: &finding.reason_code,
                    reason_text: finding.reason_text.as_deref(),
                    evidence_json: finding.evidence_json.as_deref(),
                    suggested_action: &finding.suggested_action,
                },
            )
            .await?;
        }

        let summary_json = format!("{{\"findingCount\":{}}}", findings.len());
        QualityRepo::update_audit_run_status_with_conn(
            &mut *tx,
            &run.id,
            "completed",
            Some(&summary_json),
            None,
        )
        .await?;
        tx.commit().await?;

        Ok(AuditRunResult {
            run_id: run.id,
            status: "completed".to_string(),
            finding_count: findings.len(),
            error: None,
        })
    }

    pub async fn record_failed_run(
        &self,
        book_id: &str,
        audit_type: &str,
        scope_json: &str,
        error: &str,
    ) -> anyhow::Result<AuditRunResult> {
        let mut tx = self.pool.begin().await?;
        let run = QualityRepo::create_audit_run_with_conn(
            &mut *tx,
            NewAuditRun {
                id: None,
                book_id,
                audit_type,
                scope_json,
                status: "running",
            },
        )
        .await?;
        QualityRepo::update_audit_run_status_with_conn(
            &mut *tx,
            &run.id,
            "failed",
            None,
            Some(error),
        )
        .await?;
        tx.commit().await?;
        Ok(AuditRunResult {
            run_id: run.id,
            status: "failed".to_string(),
            finding_count: 0,
            error: Some(error.to_string()),
        })
    }

    pub async fn audit_duplicate_entities(&self, book_id: &str) -> anyhow::Result<AuditRunResult> {
        let findings = self.collect_duplicate_entity_findings(book_id).await?;
        self.run_with_findings(book_id, "duplicate_entities", "{}", findings)
            .await
    }

    async fn collect_duplicate_entity_findings(
        &self,
        book_id: &str,
    ) -> anyhow::Result<Vec<AuditFindingDraft>> {
        let entity_repo = EntityRepo::new(self.pool.clone());
        let entities = entity_repo.list_by_book(book_id).await?;
        let mut findings = Vec::new();

        for left_index in 0..entities.len() {
            for right_index in (left_index + 1)..entities.len() {
                let left = &entities[left_index];
                let right = &entities[right_index];
                if let Some(finding) = self
                    .duplicate_entity_finding(book_id, &entity_repo, left, right)
                    .await?
                {
                    findings.push(finding);
                }
            }
        }

        Ok(findings)
    }

    async fn duplicate_entity_finding(
        &self,
        book_id: &str,
        entity_repo: &EntityRepo,
        left: &EntityRecord,
        right: &EntityRecord,
    ) -> anyhow::Result<Option<AuditFindingDraft>> {
        if is_org_place_same_name(left, right) {
            return Ok(Some(build_duplicate_finding(
                left,
                right,
                "org_place_same_name",
                "organization/place share a normalized name",
                "create_entity_link",
            )));
        }

        if left.entity_type != right.entity_type {
            return Ok(None);
        }

        let reason_code = if left.canonical_name == right.canonical_name {
            Some("same_normalized_name")
        } else if self
            .entities_have_overlapping_names(entity_repo, left, right)
            .await?
        {
            Some("alias_overlap")
        } else {
            None
        };

        let Some(reason_code) = reason_code else {
            return Ok(None);
        };

        if self
            .active_not_same_identity_exists(book_id, &left.id, &right.id)
            .await?
        {
            return Ok(None);
        }

        Ok(Some(build_duplicate_finding(
            left,
            right,
            reason_code,
            "same-type entities have overlapping names or aliases",
            "merge_entities",
        )))
    }

    async fn entities_have_overlapping_names(
        &self,
        entity_repo: &EntityRepo,
        left: &EntityRecord,
        right: &EntityRecord,
    ) -> anyhow::Result<bool> {
        let mut left_names = vec![
            left.canonical_name.clone(),
            normalize_name(&left.display_name),
        ];
        for alias in entity_repo.list_aliases_by_entity(&left.id).await? {
            left_names.push(normalize_name(&alias.alias));
        }

        let mut right_names = vec![
            right.canonical_name.clone(),
            normalize_name(&right.display_name),
        ];
        for alias in entity_repo.list_aliases_by_entity(&right.id).await? {
            right_names.push(normalize_name(&alias.alias));
        }

        Ok(left_names
            .iter()
            .any(|left_name| right_names.iter().any(|right_name| right_name == left_name)))
    }

    async fn active_not_same_identity_exists(
        &self,
        book_id: &str,
        left_id: &str,
        right_id: &str,
    ) -> anyhow::Result<bool> {
        let (entity_a_id, entity_b_id) = canonical_entity_pair(left_id, right_id);
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entity_identity_links
             WHERE book_id = ?
               AND ((entity_a_id = ? AND entity_b_id = ?) OR (entity_a_id = ? AND entity_b_id = ?))
               AND link_type = 'not_same_identity' AND status = 'active'",
        )
        .bind(book_id)
        .bind(&entity_a_id)
        .bind(&entity_b_id)
        .bind(&entity_b_id)
        .bind(&entity_a_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0 > 0)
    }

    pub async fn audit_relationship_pollution(
        &self,
        book_id: &str,
    ) -> anyhow::Result<AuditRunResult> {
        let findings = self
            .collect_relationship_pollution_findings(book_id)
            .await?;
        self.run_with_findings(book_id, "relationship_pollution", "{}", findings)
            .await
    }

    async fn collect_relationship_pollution_findings(
        &self,
        book_id: &str,
    ) -> anyhow::Result<Vec<AuditFindingDraft>> {
        let relationship_repo = RelationshipRepo::new(self.pool.clone());
        let entity_repo = EntityRepo::new(self.pool.clone());
        let event_repo = RelationshipEventRepo::new(self.pool.clone());
        let relationships = relationship_repo.list_active_by_book(book_id).await?;
        let mut findings = Vec::new();

        for relationship in relationships {
            if let Some(finding) = self
                .relationship_pollution_finding(&entity_repo, &event_repo, &relationship)
                .await?
            {
                findings.push(finding);
            }
        }

        Ok(findings)
    }

    async fn relationship_pollution_finding(
        &self,
        entity_repo: &EntityRepo,
        event_repo: &RelationshipEventRepo,
        relationship: &RelationshipRecord,
    ) -> anyhow::Result<Option<AuditFindingDraft>> {
        let subject = entity_repo
            .get_by_id(&relationship.subject_character_id)
            .await?;
        let object = entity_repo
            .get_by_id(&relationship.object_character_id)
            .await?;
        if subject.as_ref().map(|entity| entity.entity_type.as_str()) != Some("character")
            || object.as_ref().map(|entity| entity.entity_type.as_str()) != Some("character")
        {
            return Ok(Some(build_relationship_pollution_finding(
                relationship,
                "endpoint_not_character",
                "relationship endpoint is missing or not a character",
                "deactivate_relationship",
                "high",
            )));
        }

        if is_property_like_relationship_label(&relationship.relation_label) {
            return Ok(Some(build_relationship_pollution_finding(
                relationship,
                "property_like_relationship_label",
                "relationship label resembles location, ability, equipment, or affiliation property",
                "deactivate_relationship",
                "medium",
            )));
        }

        let event_count = event_repo.count_by_relationship(&relationship.id).await?;
        if relationship.confidence < 0.45 && event_count <= 1 {
            return Ok(Some(build_relationship_pollution_finding(
                relationship,
                "low_confidence_one_off_relationship",
                "low confidence relationship has at most one supporting event",
                "needs_manual_review",
                "low",
            )));
        }

        Ok(None)
    }

    pub async fn audit_knowledge_quality(&self, book_id: &str) -> anyhow::Result<AuditRunResult> {
        let findings = self.collect_knowledge_quality_findings(book_id).await?;
        self.run_with_findings(book_id, "knowledge_topic_drift", "{}", findings)
            .await
    }

    async fn collect_knowledge_quality_findings(
        &self,
        book_id: &str,
    ) -> anyhow::Result<Vec<AuditFindingDraft>> {
        let knowledge_repo = KnowledgeRepo::new(self.pool.clone());
        let cards = knowledge_repo
            .list_cards(book_id, None, Some("active"))
            .await?;
        let mut findings = Vec::new();
        let mut contradiction_finding_keys = std::collections::HashSet::new();

        for left_index in 0..cards.len() {
            for right_index in (left_index + 1)..cards.len() {
                let left = &cards[left_index];
                let right = &cards[right_index];
                if left.category == right.category
                    && normalize_name(&left.topic_display) == normalize_name(&right.topic_display)
                {
                    findings.push(build_knowledge_card_finding(
                        "knowledge_topic_drift_candidate",
                        "medium",
                        left,
                        Some(("knowledge_card", &right.id)),
                        "duplicate_topic_display",
                        "active knowledge cards share category and normalized display topic",
                        "needs_manual_review",
                    ));
                }
            }
        }

        for card in &cards {
            let assertions = knowledge_repo.list_assertions_for_card(&card.id).await?;
            for assertion in &assertions {
                if matches!(assertion.status.as_str(), "rumor" | "uncertain")
                    && summary_contains_assertion(card, assertion)
                {
                    findings.push(build_knowledge_assertion_finding(
                        "low_confidence_accepted_fact",
                        "medium",
                        card,
                        assertion,
                        "rumor_leaked_into_summary",
                        "rumor or uncertain assertion text is present in current summary",
                        "revise_knowledge_assertion",
                    ));
                }

                if matches!(assertion.status.as_str(), "contradicted" | "false_in_world")
                    && summary_contains_assertion(card, assertion)
                {
                    if contradiction_finding_keys.insert(format!("{}:{}", card.id, assertion.id)) {
                        findings.push(build_knowledge_assertion_finding(
                            "knowledge_contradiction_candidate",
                            "high",
                            card,
                            assertion,
                            "contradicted_assertion_in_summary",
                            "contradicted or false assertion text is still present in current summary",
                            "revise_knowledge_assertion",
                        ));
                    }
                    continue;
                }

                let contradiction_links = knowledge_repo
                    .list_links_from(book_id, &assertion.id, Some("contradicts"))
                    .await?;
                for link in contradiction_links {
                    let Some(contradicted) = knowledge_repo
                        .get_assertion_by_id(&link.to_assertion_id)
                        .await?
                    else {
                        continue;
                    };
                    if contradicted.card_id == card.id
                        && summary_contains_assertion(card, &contradicted)
                        && contradiction_finding_keys
                            .insert(format!("{}:{}", card.id, contradicted.id))
                    {
                        findings.push(build_knowledge_assertion_finding(
                            "knowledge_contradiction_candidate",
                            "high",
                            card,
                            &contradicted,
                            "contradicted_assertion_in_summary",
                            "summary contains an assertion contradicted by a newer linked assertion",
                            "revise_knowledge_assertion",
                        ));
                    }
                }
            }
        }

        Ok(findings)
    }

    pub async fn audit_map_quality(&self, book_id: &str) -> anyhow::Result<AuditRunResult> {
        let findings = self.collect_map_quality_findings(book_id).await?;
        self.run_with_findings(book_id, "map_conflicts", "{}", findings)
            .await
    }

    async fn collect_map_quality_findings(
        &self,
        book_id: &str,
    ) -> anyhow::Result<Vec<AuditFindingDraft>> {
        let place_repo = PlaceRepo::new(self.pool.clone());
        let active_edges: Vec<PlaceEdgeRecord> =
            sqlx::query_as("SELECT * FROM place_edges WHERE book_id = ? AND status = 'active'")
                .bind(book_id)
                .fetch_all(&self.pool)
                .await?;
        let active_details: Vec<PlaceDetailRecord> =
            sqlx::query_as("SELECT * FROM place_details WHERE book_id = ? AND status = 'active'")
                .bind(book_id)
                .fetch_all(&self.pool)
                .await?;
        let detail_by_id = active_details
            .iter()
            .map(|detail| (detail.entity_id.as_str(), detail))
            .collect::<std::collections::HashMap<_, _>>();
        let mut findings = Vec::new();

        for conflict in place_repo.list_conflicts(book_id, Some("open")).await? {
            findings.push(build_map_conflict_finding(&conflict));
        }

        for edge in &active_edges {
            let from_detail = detail_by_id.get(edge.from_place_id.as_str());
            let to_detail = detail_by_id.get(edge.to_place_id.as_str());
            if from_detail.is_none() || to_detail.is_none() {
                findings.push(build_map_edge_finding(
                    "broken_reference",
                    "high",
                    edge,
                    None,
                    "missing_place_detail",
                    "active place edge references a missing or inactive place detail",
                    "needs_manual_review",
                ));
                continue;
            }

            if edge.edge_type == "contains" {
                let child_detail = to_detail.unwrap();
                if child_detail.parent_place_id.as_deref() != Some(edge.from_place_id.as_str()) {
                    findings.push(build_map_parent_finding(edge, child_detail));
                }
            }
        }

        if let Some(snapshot) = place_repo
            .latest_layout_snapshot(book_id, "deterministic-v1")
            .await?
        {
            let current_hash = PlaceRepo::source_edge_hash(&active_edges);
            if snapshot.source_edge_hash != current_hash {
                findings.push(build_map_layout_finding(&snapshot, &current_hash));
            }
        }

        Ok(findings)
    }

    pub async fn convert_finding_to_correction(
        &self,
        finding_id: &str,
        actor: &str,
    ) -> anyhow::Result<UserCorrectionRecord> {
        let mut tx = self.pool.begin().await?;
        let finding = QualityRepo::get_audit_finding_with_conn(&mut *tx, finding_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("audit finding not found"))?;
        let correction = self
            .create_correction_from_finding(&mut tx, &finding, actor)
            .await?;
        QualityRepo::update_audit_finding_status_with_conn(
            &mut *tx,
            finding_id,
            "converted_to_correction",
        )
        .await?;
        tx.commit().await?;
        Ok(correction)
    }

    async fn create_correction_from_finding(
        &self,
        conn: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        finding: &QualityAuditFindingRecord,
        actor: &str,
    ) -> anyhow::Result<UserCorrectionRecord> {
        let correction_id = format!("finding:{}", finding.id);
        let correction_json = finding
            .evidence_json
            .clone()
            .unwrap_or_else(|| "{}".to_string());
        QualityRepo::create_user_correction_with_conn(
            &mut **conn,
            NewUserCorrection {
                id: Some(&correction_id),
                book_id: &finding.book_id,
                target_type: &finding.target_type,
                target_id: &finding.target_id,
                correction_type: &finding.suggested_action,
                correction_json: &correction_json,
                source: "audit",
                source_claim_id: None,
                source_span_id: None,
                created_by: actor,
            },
        )
        .await
    }
}

fn is_property_like_relationship_label(label: &str) -> bool {
    [
        "位于",
        "身处",
        "在青云",
        "在宗",
        "在门",
        "修炼",
        "功法",
        "能力",
        "装备",
        "持有",
        "同处",
        "来自",
        "隶属",
    ]
    .iter()
    .any(|keyword| label.contains(keyword))
}

fn summary_contains_assertion(
    card: &KnowledgeCardRecord,
    assertion: &KnowledgeAssertionRecord,
) -> bool {
    card.current_summary
        .as_ref()
        .map(|summary| summary.contains(&assertion.assertion_text))
        .unwrap_or(false)
}

fn build_knowledge_card_finding(
    finding_type: &str,
    severity: &str,
    card: &KnowledgeCardRecord,
    related: Option<(&str, &str)>,
    reason_code: &str,
    reason_text: &str,
    suggested_action: &str,
) -> AuditFindingDraft {
    AuditFindingDraft {
        finding_type: finding_type.to_string(),
        severity: severity.to_string(),
        target_type: "knowledge_card".to_string(),
        target_id: card.id.clone(),
        related_target_type: related.map(|(target_type, _)| target_type.to_string()),
        related_target_id: related.map(|(_, target_id)| target_id.to_string()),
        reason_code: reason_code.to_string(),
        reason_text: Some(reason_text.to_string()),
        evidence_json: Some(
            serde_json::json!({
                "cardId": card.id,
                "category": card.category,
                "topicKey": card.topic_key,
                "topicDisplay": card.topic_display,
            })
            .to_string(),
        ),
        suggested_action: suggested_action.to_string(),
    }
}

fn build_knowledge_assertion_finding(
    finding_type: &str,
    severity: &str,
    card: &KnowledgeCardRecord,
    assertion: &KnowledgeAssertionRecord,
    reason_code: &str,
    reason_text: &str,
    suggested_action: &str,
) -> AuditFindingDraft {
    AuditFindingDraft {
        finding_type: finding_type.to_string(),
        severity: severity.to_string(),
        target_type: "knowledge_card".to_string(),
        target_id: card.id.clone(),
        related_target_type: Some("knowledge_assertion".to_string()),
        related_target_id: Some(assertion.id.clone()),
        reason_code: reason_code.to_string(),
        reason_text: Some(reason_text.to_string()),
        evidence_json: Some(
            serde_json::json!({
                "cardId": card.id,
                "assertionId": assertion.id,
                "assertionStatus": assertion.status,
                "assertionText": assertion.assertion_text,
                "summary": card.current_summary,
            })
            .to_string(),
        ),
        suggested_action: suggested_action.to_string(),
    }
}

fn build_map_conflict_finding(conflict: &PlaceEdgeConflictRecord) -> AuditFindingDraft {
    let target_id = conflict
        .existing_edge_id
        .clone()
        .unwrap_or_else(|| conflict.new_edge_claim_id.clone());
    let target_type = if conflict.existing_edge_id.is_some() {
        "place_edge"
    } else {
        "claim"
    };
    AuditFindingDraft {
        finding_type: "map_conflict_candidate".to_string(),
        severity: "high".to_string(),
        target_type: target_type.to_string(),
        target_id,
        related_target_type: Some("claim".to_string()),
        related_target_id: Some(conflict.new_edge_claim_id.clone()),
        reason_code: conflict.reason_code.clone(),
        reason_text: Some(format!("open map conflict: {}", conflict.conflict_type)),
        evidence_json: Some(
            serde_json::json!({
                "conflictId": conflict.id,
                "conflictType": conflict.conflict_type,
                "reasonCode": conflict.reason_code,
                "existingEdgeId": conflict.existing_edge_id,
                "newEdgeClaimId": conflict.new_edge_claim_id,
            })
            .to_string(),
        ),
        suggested_action: "mark_place_edge_conflict".to_string(),
    }
}

fn build_map_edge_finding(
    finding_type: &str,
    severity: &str,
    edge: &PlaceEdgeRecord,
    related: Option<(&str, &str)>,
    reason_code: &str,
    reason_text: &str,
    suggested_action: &str,
) -> AuditFindingDraft {
    AuditFindingDraft {
        finding_type: finding_type.to_string(),
        severity: severity.to_string(),
        target_type: "place_edge".to_string(),
        target_id: edge.id.clone(),
        related_target_id: related.map(|(target_id, _)| target_id.to_string()),
        related_target_type: related.map(|(_, target_type)| target_type.to_string()),
        reason_code: reason_code.to_string(),
        reason_text: Some(reason_text.to_string()),
        evidence_json: Some(
            serde_json::json!({
                "edgeId": edge.id,
                "fromPlaceId": edge.from_place_id,
                "toPlaceId": edge.to_place_id,
                "edgeType": edge.edge_type,
            })
            .to_string(),
        ),
        suggested_action: suggested_action.to_string(),
    }
}

fn build_map_parent_finding(
    edge: &PlaceEdgeRecord,
    child_detail: &PlaceDetailRecord,
) -> AuditFindingDraft {
    AuditFindingDraft {
        finding_type: "map_conflict_candidate".to_string(),
        severity: "high".to_string(),
        target_type: "place".to_string(),
        target_id: child_detail.entity_id.clone(),
        related_target_type: Some("place_edge".to_string()),
        related_target_id: Some(edge.id.clone()),
        reason_code: "parent_detail_conflicts_with_contains_edge".to_string(),
        reason_text: Some("place detail parent does not match active contains edge".to_string()),
        evidence_json: Some(
            serde_json::json!({
                "edgeId": edge.id,
                "edgeFromPlaceId": edge.from_place_id,
                "childPlaceId": child_detail.entity_id,
                "detailParentPlaceId": child_detail.parent_place_id,
            })
            .to_string(),
        ),
        suggested_action: "correct_place_parent".to_string(),
    }
}

fn build_map_layout_finding(
    snapshot: &MapLayoutSnapshotRecord,
    current_hash: &str,
) -> AuditFindingDraft {
    AuditFindingDraft {
        finding_type: "stale_projection".to_string(),
        severity: "medium".to_string(),
        target_type: "projection_cache".to_string(),
        target_id: format!("map_layout:{}", snapshot.layout_version),
        related_target_type: None,
        related_target_id: None,
        reason_code: "stale_layout_source_edge_hash".to_string(),
        reason_text: Some("latest map layout hash does not match active place edges".to_string()),
        evidence_json: Some(
            serde_json::json!({
                "snapshotId": snapshot.id,
                "layoutVersion": snapshot.layout_version,
                "snapshotHash": snapshot.source_edge_hash,
                "currentHash": current_hash,
            })
            .to_string(),
        ),
        suggested_action: "rebuild_projection".to_string(),
    }
}

fn build_relationship_pollution_finding(
    relationship: &RelationshipRecord,
    reason_code: &str,
    reason_text: &str,
    suggested_action: &str,
    severity: &str,
) -> AuditFindingDraft {
    AuditFindingDraft {
        finding_type: "relationship_pollution_candidate".to_string(),
        severity: severity.to_string(),
        target_type: "relationship".to_string(),
        target_id: relationship.id.clone(),
        related_target_type: Some("entity".to_string()),
        related_target_id: Some(relationship.object_character_id.clone()),
        reason_code: reason_code.to_string(),
        reason_text: Some(reason_text.to_string()),
        evidence_json: Some(
            serde_json::json!({
                "relationshipId": relationship.id,
                "subjectId": relationship.subject_character_id,
                "objectId": relationship.object_character_id,
                "label": relationship.relation_label,
                "confidence": relationship.confidence,
            })
            .to_string(),
        ),
        suggested_action: suggested_action.to_string(),
    }
}

fn is_org_place_same_name(left: &EntityRecord, right: &EntityRecord) -> bool {
    let type_pair_matches = (left.entity_type == "organization" && right.entity_type == "place")
        || (left.entity_type == "place" && right.entity_type == "organization");
    type_pair_matches && left.canonical_name == right.canonical_name
}

fn canonical_entity_pair(left_id: &str, right_id: &str) -> (String, String) {
    if left_id <= right_id {
        (left_id.to_string(), right_id.to_string())
    } else {
        (right_id.to_string(), left_id.to_string())
    }
}

fn build_duplicate_finding(
    left: &EntityRecord,
    right: &EntityRecord,
    reason_code: &str,
    reason_text: &str,
    suggested_action: &str,
) -> AuditFindingDraft {
    AuditFindingDraft {
        finding_type: "duplicate_entity_candidate".to_string(),
        severity: "medium".to_string(),
        target_type: "entity".to_string(),
        target_id: left.id.clone(),
        related_target_type: Some("entity".to_string()),
        related_target_id: Some(right.id.clone()),
        reason_code: reason_code.to_string(),
        reason_text: Some(reason_text.to_string()),
        evidence_json: Some(format!(
            "{{\"leftEntityId\":\"{}\",\"rightEntityId\":\"{}\",\"leftType\":\"{}\",\"rightType\":\"{}\"}}",
            left.id, right.id, left.entity_type, right.entity_type
        )),
        suggested_action: suggested_action.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;
    use crate::storage::db::v4::knowledge_repo::KnowledgeRepo;
    use crate::storage::db::v4::quality_repo::{NewAuditFinding, QualityRepo};
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let dir =
            std::env::temp_dir().join(format!("reader-quality-audit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database_url = format!("sqlite:{}?mode=rwc", dir.join("reader.db").display());
        db::init_pool(&database_url).await.unwrap()
    }

    #[tokio::test]
    async fn quality_audit_service_lifecycle_records_completed_findings() {
        let pool = setup_test_db().await;
        let service = QualityAuditService::new(pool.clone());

        let result = service
            .run_with_findings(
                "b1",
                "duplicate_entities",
                "{}",
                vec![AuditFindingDraft {
                    finding_type: "duplicate_entity_candidate".to_string(),
                    severity: "medium".to_string(),
                    target_type: "entity".to_string(),
                    target_id: "e1".to_string(),
                    related_target_type: Some("entity".to_string()),
                    related_target_id: Some("e2".to_string()),
                    reason_code: "same_name".to_string(),
                    reason_text: Some("same canonical name".to_string()),
                    evidence_json: Some("{}".to_string()),
                    suggested_action: "merge_entities".to_string(),
                }],
            )
            .await
            .unwrap();

        assert_eq!(result.status, "completed");
        assert_eq!(result.finding_count, 1);
        let run = QualityRepo::new(pool.clone())
            .get_audit_run(&result.run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.status, "completed");
        assert_eq!(run.summary_json.as_deref(), Some("{\"findingCount\":1}"));
        let findings = QualityRepo::new(pool)
            .list_audit_findings("b1", Some("open"), None, None, None, 10)
            .await
            .unwrap();
        assert_eq!(findings.len(), 1);
    }

    #[tokio::test]
    async fn quality_audit_service_records_failure_without_findings() {
        let pool = setup_test_db().await;
        let service = QualityAuditService::new(pool.clone());

        let result = service
            .record_failed_run("b1", "relationship_pollution", "{}", "boom")
            .await
            .unwrap();

        assert_eq!(result.status, "failed");
        let run = QualityRepo::new(pool.clone())
            .get_audit_run(&result.run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.status, "failed");
        assert_eq!(run.error.as_deref(), Some("boom"));
        let findings = QualityRepo::new(pool)
            .list_audit_findings("b1", None, None, None, None, 10)
            .await
            .unwrap();
        assert!(findings.is_empty());
    }

    async fn seed_source_claim(pool: &SqlitePool, claim_id: &str) {
        sqlx::query("INSERT INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch_identity', 'b1', 1, 'text', 'hash_identity', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg_identity', 'b1', 'ch_identity', 'hash_identity', 0, datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span_identity', 'b1', 'ch_identity', 'hash_identity', 'seg_identity', 0, 0, 10, 'text', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run_identity', 'b1', 'ch_identity', 'identity_judge', 'test', 'v1', 1, 'input', 'completed', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', 1, 'not_same_identity', 'test', 'span_identity', 'run_identity', 0.9, 'high', 'accepted', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn duplicate_entity_audit_flags_alias_overlap_without_mutating_entities() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let first = entity_repo
            .create_entity("b1", "character", "张三", "张三", None, 0.8, 1)
            .await
            .unwrap();
        let second = entity_repo
            .create_entity("b1", "character", "黑衣人", "黑衣人", None, 0.7, 2)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &second.id, "张三", "identity", 2, 0.9, None)
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_duplicate_entities("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool.clone())
            .list_audit_findings("b1", Some("open"), None, None, None, 10)
            .await
            .unwrap();
        assert_eq!(findings[0].suggested_action, "merge_entities");
        assert_eq!(findings[0].target_id, first.id);
        assert_eq!(
            findings[0].related_target_id.as_deref(),
            Some(second.id.as_str())
        );
        assert_eq!(
            entity_repo
                .get_by_id(&second.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "active"
        );
    }

    #[tokio::test]
    async fn duplicate_entity_audit_respects_active_not_same_guard() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let first = entity_repo
            .create_entity("b1", "character", "此张三", "此张三", None, 0.8, 1)
            .await
            .unwrap();
        let second = entity_repo
            .create_entity("b1", "character", "彼张三", "彼张三", None, 0.7, 2)
            .await
            .unwrap();
        entity_repo
            .create_alias("b1", &second.id, "此张三", "name", 2, 0.9, None)
            .await
            .unwrap();
        seed_source_claim(&pool, "identity_claim").await;
        sqlx::query("INSERT INTO entity_identity_links (id, book_id, entity_a_id, entity_b_id, link_type, confidence, source_claim_id, status, created_at, updated_at) VALUES ('not_same_1', 'b1', ?, ?, 'not_same_identity', 0.95, 'identity_claim', 'active', datetime('now'), datetime('now'))")
            .bind(&first.id)
            .bind(&second.id)
            .execute(&pool)
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_duplicate_entities("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 0);
    }

    #[tokio::test]
    async fn duplicate_entity_audit_suggests_entity_link_for_org_place_same_name() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let org = entity_repo
            .create_entity("b1", "organization", "青云门", "青云门", None, 0.8, 1)
            .await
            .unwrap();
        let place = entity_repo
            .create_entity("b1", "place", "青云门", "青云门", None, 0.7, 1)
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_duplicate_entities("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool)
            .list_audit_findings("b1", Some("open"), None, None, None, 10)
            .await
            .unwrap();
        assert_eq!(findings[0].suggested_action, "create_entity_link");
        assert_eq!(findings[0].target_id, org.id);
        assert_eq!(
            findings[0].related_target_id.as_deref(),
            Some(place.id.as_str())
        );
    }

    async fn create_test_entity(
        entity_repo: &crate::storage::db::v4::entity_repo::EntityRepo,
        entity_type: &str,
        name: &str,
    ) -> String {
        entity_repo
            .create_entity("b1", entity_type, name, name, None, 0.7, 1)
            .await
            .unwrap()
            .id
    }

    async fn create_test_relationship(
        pool: &SqlitePool,
        subject_id: &str,
        object_id: &str,
        label: &str,
        confidence: f64,
    ) -> String {
        crate::storage::db::v4::relationship_repo::RelationshipRepo::new(pool.clone())
            .create_relationship(
                "b1",
                subject_id,
                object_id,
                "other_social",
                label,
                "undirected",
                None,
                0.6,
                "neutral",
                confidence,
                0.6,
                1,
            )
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn relationship_pollution_audit_flags_non_character_endpoint() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let character = create_test_entity(&entity_repo, "character", "张三").await;
        let organization = create_test_entity(&entity_repo, "organization", "青云门").await;
        let relationship_id =
            create_test_relationship(&pool, &character, &organization, "同门", 0.8).await;

        let result = QualityAuditService::new(pool.clone())
            .audit_relationship_pollution("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool.clone())
            .list_audit_findings("b1", Some("open"), None, None, None, 10)
            .await
            .unwrap();
        assert_eq!(findings[0].reason_code, "endpoint_not_character");
        assert_eq!(findings[0].target_id, relationship_id);
        let status: (String,) = sqlx::query_as("SELECT status FROM relationships WHERE id = ?")
            .bind(&relationship_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status.0, "active");
    }

    #[tokio::test]
    async fn relationship_pollution_audit_flags_property_like_label() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let first = create_test_entity(&entity_repo, "character", "张三").await;
        let second = create_test_entity(&entity_repo, "character", "李四").await;
        create_test_relationship(&pool, &first, &second, "张三在青云门", 0.8).await;

        let result = QualityAuditService::new(pool.clone())
            .audit_relationship_pollution("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool)
            .list_audit_findings("b1", Some("open"), None, None, None, 10)
            .await
            .unwrap();
        assert_eq!(findings[0].reason_code, "property_like_relationship_label");
    }

    #[tokio::test]
    async fn relationship_pollution_audit_does_not_flag_valid_relationship() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let first = create_test_entity(&entity_repo, "character", "师父").await;
        let second = create_test_entity(&entity_repo, "character", "徒弟").await;
        create_test_relationship(&pool, &first, &second, "师徒", 0.9).await;

        let result = QualityAuditService::new(pool.clone())
            .audit_relationship_pollution("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 0);
    }

    async fn seed_knowledge_source_claim(pool: &SqlitePool, claim_id: &str, chapter_index: i64) {
        sqlx::query("INSERT OR IGNORE INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch_knowledge', 'b1', 1, 'knowledge text', 'hash_knowledge', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg_knowledge', 'b1', 'ch_knowledge', 'hash_knowledge', 0, datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span_knowledge', 'b1', 'ch_knowledge', 'hash_knowledge', 'seg_knowledge', 0, 0, 20, 'knowledge text', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run_knowledge', 'b1', 'ch_knowledge', 'knowledge_extract', 'test', 'v1', 1, 'knowledge_input', 'completed', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', ?, 'knowledge_assertion', 'test', 'span_knowledge', 'run_knowledge', 0.8, 'medium', 'accepted', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .bind(chapter_index)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn knowledge_quality_audit_flags_duplicate_topic_display() {
        let pool = setup_test_db().await;
        let knowledge_repo = KnowledgeRepo::new(pool.clone());
        let first = knowledge_repo
            .find_or_create_card(
                "b1",
                "world_rule",
                "heavenly-dao",
                "天道",
                Some("天道会约束修士誓言"),
                0.8,
                0.7,
                1,
            )
            .await
            .unwrap();
        let second = knowledge_repo
            .find_or_create_card(
                "b1",
                "world_rule",
                "dao-of-heaven",
                "天道",
                Some("天道誓言会反噬违誓者"),
                0.7,
                0.6,
                2,
            )
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_knowledge_quality("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool)
            .list_audit_findings(
                "b1",
                Some("open"),
                Some("knowledge_topic_drift_candidate"),
                None,
                None,
                10,
            )
            .await
            .unwrap();
        assert_eq!(findings[0].reason_code, "duplicate_topic_display");
        assert_eq!(findings[0].target_id, first.id);
        assert_eq!(
            findings[0].related_target_id.as_deref(),
            Some(second.id.as_str())
        );
    }

    #[tokio::test]
    async fn knowledge_quality_audit_flags_rumor_leakage_without_mutating_summary() {
        let pool = setup_test_db().await;
        let knowledge_repo = KnowledgeRepo::new(pool.clone());
        let card = knowledge_repo
            .find_or_create_card(
                "b1",
                "secret",
                "blood-moon",
                "血月传闻",
                Some("传说血月会吞噬修士灵力"),
                0.5,
                0.8,
                3,
            )
            .await
            .unwrap();
        seed_knowledge_source_claim(&pool, "knowledge_rumor_claim", 3).await;
        knowledge_repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "knowledge_rumor_claim",
                "传说血月会吞噬修士灵力",
                "rumor",
                0.35,
                0.7,
                3,
            )
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_knowledge_quality("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool.clone())
            .list_audit_findings(
                "b1",
                Some("open"),
                Some("low_confidence_accepted_fact"),
                None,
                None,
                10,
            )
            .await
            .unwrap();
        assert_eq!(findings[0].reason_code, "rumor_leaked_into_summary");
        let summary: (String,) =
            sqlx::query_as("SELECT current_summary FROM knowledge_cards WHERE id = ?")
                .bind(&card.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(summary.0, "传说血月会吞噬修士灵力");
    }

    #[tokio::test]
    async fn knowledge_quality_audit_flags_contradicted_summary() {
        let pool = setup_test_db().await;
        let knowledge_repo = KnowledgeRepo::new(pool.clone());
        let card = knowledge_repo
            .find_or_create_card(
                "b1",
                "world_rule",
                "spirit-vein",
                "灵脉规则",
                Some("灵脉只会滋养修士"),
                0.8,
                0.8,
                4,
            )
            .await
            .unwrap();
        seed_knowledge_source_claim(&pool, "knowledge_old_claim", 4).await;
        seed_knowledge_source_claim(&pool, "knowledge_new_claim", 5).await;
        let old_assertion = knowledge_repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "knowledge_old_claim",
                "灵脉只会滋养修士",
                "contradicted",
                0.7,
                0.6,
                4,
            )
            .await
            .unwrap();
        let new_assertion = knowledge_repo
            .find_or_create_assertion(
                "b1",
                &card.id,
                "knowledge_new_claim",
                "灵脉会吞噬修士灵力",
                "active",
                0.9,
                0.8,
                5,
            )
            .await
            .unwrap();
        knowledge_repo
            .insert_assertion_link("b1", &new_assertion.id, &old_assertion.id, "contradicts")
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_knowledge_quality("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool)
            .list_audit_findings(
                "b1",
                Some("open"),
                Some("knowledge_contradiction_candidate"),
                None,
                None,
                10,
            )
            .await
            .unwrap();
        assert_eq!(findings[0].reason_code, "contradicted_assertion_in_summary");
        assert_eq!(findings[0].target_id, card.id);
        assert_eq!(
            findings[0].related_target_id.as_deref(),
            Some(old_assertion.id.as_str())
        );
    }

    async fn seed_map_source_claim(pool: &SqlitePool, claim_id: &str, chapter_index: i64) {
        sqlx::query("INSERT OR IGNORE INTO chapters (id, book_id, chapter_index, raw_text, text_hash, created_at) VALUES ('ch_map', 'b1', 1, 'map text', 'hash_map', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO chapter_segments (id, book_id, chapter_id, chapter_hash, segment_index, created_at) VALUES ('seg_map', 'b1', 'ch_map', 'hash_map', 0, datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO source_spans (id, book_id, chapter_id, chapter_hash, segment_id, span_index, start_offset, end_offset, text_excerpt, created_at) VALUES ('span_map', 'b1', 'ch_map', 'hash_map', 'seg_map', 0, 0, 20, 'map text', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO ai_runs (id, book_id, chapter_id, run_type, model, prompt_version, schema_version, input_hash, status, started_at) VALUES ('run_map', 'b1', 'ch_map', 'map_extract', 'test', 'v1', 1, 'map_input', 'completed', datetime('now'))")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO claims (id, book_id, chapter_index, claim_type, predicate, primary_source_span_id, ai_run_id, confidence, risk_level, status, created_at, updated_at) VALUES (?, 'b1', ?, 'location_edge', 'test', 'span_map', 'run_map', 0.8, 'medium', 'accepted', datetime('now'), datetime('now'))")
            .bind(claim_id)
            .bind(chapter_index)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn create_test_place(
        _pool: &SqlitePool,
        place_repo: &PlaceRepo,
        entity_repo: &crate::storage::db::v4::entity_repo::EntityRepo,
        name: &str,
    ) -> String {
        let entity_id = entity_repo
            .create_entity("b1", "place", name, name, None, 0.8, 1)
            .await
            .unwrap()
            .id;
        place_repo
            .upsert_place_detail(
                "b1", &entity_id, "region", None, 1, 0.6, true, 1, 1, "active",
            )
            .await
            .unwrap();
        entity_id
    }

    #[tokio::test]
    async fn map_quality_audit_flags_open_conflict_without_mutating_edge() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let place_repo = PlaceRepo::new(pool.clone());
        let north = create_test_place(&pool, &place_repo, &entity_repo, "北山").await;
        let south = create_test_place(&pool, &place_repo, &entity_repo, "南谷").await;
        seed_map_source_claim(&pool, "map_claim_edge", 1).await;
        seed_map_source_claim(&pool, "map_claim_conflict", 2).await;
        let edge = place_repo
            .find_or_create_edge(
                "b1",
                &north,
                &south,
                "north_of",
                None,
                None,
                0.8,
                "map_claim_edge",
                1,
            )
            .await
            .unwrap();
        place_repo
            .insert_conflict(
                "b1",
                "map_claim_conflict",
                Some(&edge.id),
                "opposite_direction",
                "inverse_direction_conflict",
                Some("{}"),
            )
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_map_quality("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool.clone())
            .list_audit_findings(
                "b1",
                Some("open"),
                Some("map_conflict_candidate"),
                None,
                None,
                10,
            )
            .await
            .unwrap();
        assert_eq!(findings[0].reason_code, "inverse_direction_conflict");
        assert_eq!(findings[0].target_id, edge.id);
        let status: (String,) = sqlx::query_as("SELECT status FROM place_edges WHERE id = ?")
            .bind(&edge.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status.0, "active");
    }

    #[tokio::test]
    async fn map_quality_audit_flags_parent_detail_conflict() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let place_repo = PlaceRepo::new(pool.clone());
        let child = create_test_place(&pool, &place_repo, &entity_repo, "内院").await;
        let old_parent = create_test_place(&pool, &place_repo, &entity_repo, "旧城").await;
        let new_parent = create_test_place(&pool, &place_repo, &entity_repo, "新城").await;
        place_repo
            .upsert_place_detail(
                "b1",
                &child,
                "building",
                Some(&old_parent),
                2,
                0.7,
                true,
                1,
                2,
                "active",
            )
            .await
            .unwrap();
        seed_map_source_claim(&pool, "map_claim_parent", 2).await;
        let edge = place_repo
            .find_or_create_edge(
                "b1",
                &new_parent,
                &child,
                "contains",
                None,
                None,
                0.9,
                "map_claim_parent",
                2,
            )
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_map_quality("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool)
            .list_audit_findings(
                "b1",
                Some("open"),
                Some("map_conflict_candidate"),
                None,
                None,
                10,
            )
            .await
            .unwrap();
        assert_eq!(
            findings[0].reason_code,
            "parent_detail_conflicts_with_contains_edge"
        );
        assert_eq!(findings[0].target_id, child);
        assert_eq!(
            findings[0].related_target_id.as_deref(),
            Some(edge.id.as_str())
        );
    }

    #[tokio::test]
    async fn map_quality_audit_flags_stale_layout_hash() {
        let pool = setup_test_db().await;
        let entity_repo = crate::storage::db::v4::entity_repo::EntityRepo::new(pool.clone());
        let place_repo = PlaceRepo::new(pool.clone());
        let first = create_test_place(&pool, &place_repo, &entity_repo, "东门").await;
        let second = create_test_place(&pool, &place_repo, &entity_repo, "西门").await;
        seed_map_source_claim(&pool, "map_claim_layout", 3).await;
        place_repo
            .find_or_create_edge(
                "b1",
                &first,
                &second,
                "connects_to",
                None,
                None,
                0.8,
                "map_claim_layout",
                3,
            )
            .await
            .unwrap();
        place_repo
            .create_layout_snapshot("b1", 3, "deterministic-v1", "{}", "stale-hash")
            .await
            .unwrap();

        let result = QualityAuditService::new(pool.clone())
            .audit_map_quality("b1")
            .await
            .unwrap();

        assert_eq!(result.finding_count, 1);
        let findings = QualityRepo::new(pool)
            .list_audit_findings("b1", Some("open"), Some("stale_projection"), None, None, 10)
            .await
            .unwrap();
        assert_eq!(findings[0].reason_code, "stale_layout_source_edge_hash");
        assert_eq!(findings[0].target_type, "projection_cache");
    }

    #[tokio::test]
    async fn quality_audit_finding_converts_to_correction_once() {
        let pool = setup_test_db().await;
        let repo = QualityRepo::new(pool.clone());
        let run = repo
            .create_audit_run(crate::storage::db::v4::quality_repo::NewAuditRun {
                id: None,
                book_id: "b1",
                audit_type: "relationship_pollution",
                scope_json: "{}",
                status: "completed",
            })
            .await
            .unwrap();
        let finding = repo
            .create_audit_finding(NewAuditFinding {
                id: None,
                book_id: "b1",
                audit_run_id: &run.id,
                finding_type: "relationship_pollution_candidate",
                severity: "high",
                target_type: "relationship",
                target_id: "rel1",
                related_target_type: None,
                related_target_id: None,
                reason_code: "endpoint_not_character",
                reason_text: Some("bad endpoint"),
                evidence_json: Some("{}"),
                suggested_action: "deactivate_relationship",
            })
            .await
            .unwrap();

        let service = QualityAuditService::new(pool.clone());
        let first = service
            .convert_finding_to_correction(&finding.id, "tester")
            .await
            .unwrap();
        let second = service
            .convert_finding_to_correction(&finding.id, "tester")
            .await
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.correction_type, "deactivate_relationship");
        let updated = repo.get_audit_finding(&finding.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "converted_to_correction");
    }
}
