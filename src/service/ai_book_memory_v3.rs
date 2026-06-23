use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::error::AppError;
use crate::model::ai_book::{
    AiBookChapterDigestV3, AiBookChapterMemoryViewModel, AiBookCharacterRelationV3,
    AiBookCharacterStateV3, AiBookCharacterView, AiBookCharacterV3, AiBookDroppedFactV3,
    AiBookDroppedFactsSummary, AiBookEvidenceV3, AiBookFactCategory, AiBookFactConfidence,
    AiBookFactImportance, AiBookKnowledgeFactView, AiBookKnowledgeFactV3, AiBookLocationEdgeKind,
    AiBookLocationEdgeV3, AiBookLocationV3, AiBookLocationView, AiBookMapView, AiBookMemoryV3,
    AiBookMemoryViewModel, AiBookRelationChangeView, AiBookRelationFacetView,
    AiBookRelationKind, AiBookRelationPolarity, AiBookRelationStatus, AiBookRelationStrength,
    AiBookRelationView,
};
use crate::model::ai_book_generation::{
    AiBookCharacterPatchV3, AiBookCharacterRelationPatchV3, AiBookCharacterStatePatchV3,
    AiBookKnowledgeFactPatchV3, AiBookLocationEdgePatchV3, AiBookLocationPatchV3,
    KnowledgePatchV3,
};
use crate::util::{hash::md5_hex, time::now_ts};

const MAX_RECENT_CHAPTER_DIGESTS: usize = 8;
const MAX_RELEVANT_CHARACTERS: usize = 20;
const MAX_RELEVANT_RELATIONS: usize = 12;
const MAX_RELEVANT_FACTS: usize = 12;
const MAX_RELEVANT_LOCATIONS: usize = 15;
const MAX_DROPPED_FACTS: usize = 200;
const V3_RELATION_META_PREFIX: &str = "__ai_book_v3_relation__:";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiBookWorkingContextV3 {
    pub book_name: Option<String>,
    pub author: Option<String>,
    pub summary_current: String,
    pub recent_chapter_digests: Vec<WorkingContextDigestV3>,
    pub relevant_characters: Vec<WorkingContextCharacterV3>,
    pub relevant_relations: Vec<WorkingContextRelationV3>,
    pub relevant_knowledge_facts: Vec<WorkingContextFactV3>,
    pub relevant_locations: Vec<WorkingContextLocationV3>,
    pub schema_hint: String,
    pub current_chapter_index: Option<i32>,
    pub current_chapter_title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingContextDigestV3 {
    pub chapter_index: i32,
    pub chapter_title: String,
    pub digest: String,
    pub key_events: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingContextCharacterV3 {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub status: Option<String>,
    pub affiliations: Vec<String>,
    pub abilities: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingContextRelationV3 {
    pub source_character_id: String,
    pub source_name: String,
    pub target_character_id: String,
    pub target_name: String,
    pub kind: String,
    pub subtype: Option<String>,
    pub label: String,
    pub polarity: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingContextFactV3 {
    pub id: String,
    pub title: String,
    pub category: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingContextLocationV3 {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub parent_name: Option<String>,
    pub kind: String,
    pub scale: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedKnowledgePatchV3 {
    pub chapter_index: i32,
    pub summary: Option<String>,
    pub characters: Vec<NormalizedCharacterV3>,
    pub character_states: Vec<NormalizedCharacterStateV3>,
    pub character_relations: Vec<NormalizedCharacterRelationV3>,
    pub knowledge_facts: Vec<NormalizedKnowledgeFactV3>,
    pub locations: Vec<NormalizedLocationV3>,
    pub location_edges: Vec<NormalizedLocationEdgeV3>,
    pub dropped_facts: Vec<AiBookDroppedFactV3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedCharacterV3 {
    pub id: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub status: Option<String>,
    pub faction: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedCharacterStateV3 {
    pub character_id: String,
    pub canonical_name: String,
    pub current_status: Option<String>,
    pub affiliations: Vec<String>,
    pub abilities: Vec<String>,
    pub resources: Vec<String>,
    pub current_location_id: Option<String>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedCharacterRelationV3 {
    pub id: String,
    pub source_character_id: String,
    pub source_name: String,
    pub target_character_id: String,
    pub target_name: String,
    pub kind: AiBookRelationKind,
    pub subtype: Option<String>,
    pub label: String,
    pub polarity: AiBookRelationPolarity,
    pub strength: AiBookRelationStrength,
    pub status: AiBookRelationStatus,
    pub direction: RelationDirectionV3,
    pub summary: String,
    pub current_dynamics: Vec<String>,
    pub evidence: Vec<AiBookEvidenceV3>,
    pub history: Vec<AiBookRelationChangeView>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedKnowledgeFactV3 {
    pub id: String,
    pub title: String,
    pub content: String,
    pub category: AiBookFactCategory,
    pub confidence: AiBookFactConfidence,
    pub importance: AiBookFactImportance,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedLocationV3 {
    pub id: String,
    pub canonical_name: String,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedLocationEdgeV3 {
    pub id: String,
    pub source_location_id: String,
    pub source_name: String,
    pub target_location_id: String,
    pub target_name: String,
    pub kind: AiBookLocationEdgeKind,
    pub label: Option<String>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationDirectionV3 {
    Directed,
    Undirected,
}

impl Default for RelationDirectionV3 {
    fn default() -> Self {
        Self::Undirected
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelationCandidateV3 {
    pub source_name: String,
    pub target_name: String,
    pub kind_raw: String,
    pub polarity_raw: String,
    pub strength_raw: String,
    pub status_raw: String,
    pub description: Option<String>,
    pub evidence: Vec<AiBookEvidenceV3>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationRedirectV3 {
    CharacterState(NormalizedCharacterStateV3),
    KnowledgeFact(NormalizedKnowledgeFactV3),
    LocationEdge(NormalizedLocationEdgeV3),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationClassificationV3 {
    Keep(NormalizedCharacterRelationV3),
    Redirect(RelationRedirectV3),
    Drop(AiBookDroppedFactV3),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RelationStorageMetaV3 {
    id: String,
    source_character_id: String,
    target_character_id: String,
    source_name: String,
    target_name: String,
    subtype: Option<String>,
    label: String,
    direction: String,
    summary: String,
    current_dynamics: Vec<String>,
    evidence: Vec<AiBookEvidenceV3>,
    history: Vec<AiBookRelationChangeView>,
}

pub fn create_empty_ai_book_memory_v3(
    book_url: impl Into<String>,
    book_name: Option<String>,
    author: Option<String>,
) -> AiBookMemoryV3 {
    let mut memory = AiBookMemoryV3::new(book_url);
    memory.book_name = book_name.filter(|value| !value.trim().is_empty());
    memory.author = author.filter(|value| !value.trim().is_empty());
    memory
}

pub fn validate_ai_book_memory_v3(memory: &AiBookMemoryV3) -> Result<(), AppError> {
    if memory.schema_version != 3 {
        return Err(AppError::BadRequest(format!(
            "unsupported ai book schema version: {}",
            memory.schema_version
        )));
    }
    if memory.book_url.trim().is_empty() {
        return Err(AppError::BadRequest("bookUrl is required".to_string()));
    }
    for character in &memory.characters {
        if character.name.trim().is_empty() {
            return Err(AppError::BadRequest(
                "character name cannot be empty".to_string(),
            ));
        }
    }
    for relation in &memory.character_relations {
        if relation.source.trim().is_empty() || relation.target.trim().is_empty() {
            return Err(AppError::BadRequest(
                "relation source and target are required".to_string(),
            ));
        }
    }
    for edge in &memory.location_edges {
        if edge.source.trim().is_empty() || edge.target.trim().is_empty() {
            return Err(AppError::BadRequest(
                "location edge source and target are required".to_string(),
            ));
        }
    }
    Ok(())
}

pub fn normalize_knowledge_patch_v3(
    patch: KnowledgePatchV3,
    context: &AiBookWorkingContextV3,
) -> NormalizedKnowledgePatchV3 {
    let evidence = patch_evidence(context, patch.chapter_index);
    let mut normalized = NormalizedKnowledgePatchV3 {
        chapter_index: patch.chapter_index,
        summary: clean_optional(patch.summary),
        ..NormalizedKnowledgePatchV3::default()
    };

    let normalized_characters: Vec<_> = patch
        .characters
        .iter()
        .filter_map(|character| normalize_character_patch(character, context, &evidence))
        .collect();
    let normalized_locations: Vec<_> = patch
        .locations
        .iter()
        .filter_map(|location| normalize_location_patch(location, context, &evidence))
        .collect();

    let extended_context = extend_context(context, &normalized_characters, &normalized_locations);

    normalized.characters = dedupe_by_id(normalized_characters, |item| item.id.clone(), merge_character_patch);
    normalized.locations = dedupe_by_id(normalized_locations, |item| item.id.clone(), merge_location_patch);
    normalized.character_states = patch
        .character_states
        .iter()
        .filter_map(|state| normalize_character_state_patch(state, &extended_context, &evidence))
        .collect();
    normalized.knowledge_facts = patch
        .knowledge_facts
        .iter()
        .filter_map(|fact| normalize_fact_patch(fact, &evidence))
        .collect();
    normalized.location_edges = patch
        .location_edges
        .iter()
        .filter_map(|edge| normalize_location_edge_patch(edge, &extended_context, &evidence))
        .collect();

    for relation in &patch.character_relations {
        let candidate = relation_candidate_from_patch(relation, &extended_context, &evidence);
        match classify_relation_candidate_v3(candidate, &extended_context) {
            RelationClassificationV3::Keep(relation) => {
                normalized.character_relations.push(relation);
            }
            RelationClassificationV3::Redirect(RelationRedirectV3::CharacterState(state)) => {
                normalized.character_states.push(state);
            }
            RelationClassificationV3::Redirect(RelationRedirectV3::KnowledgeFact(fact)) => {
                normalized.knowledge_facts.push(fact);
            }
            RelationClassificationV3::Redirect(RelationRedirectV3::LocationEdge(edge)) => {
                normalized.location_edges.push(edge);
            }
            RelationClassificationV3::Drop(dropped) => {
                normalized.dropped_facts.push(dropped);
            }
        }
    }

    normalized.character_states = dedupe_by_id(
        normalized.character_states,
        |item| item.character_id.clone(),
        merge_character_state_patch,
    );
    normalized.character_relations = dedupe_by_id(
        normalized.character_relations,
        |item| item.id.clone(),
        merge_relation_patch,
    );
    normalized.knowledge_facts = dedupe_by_id(
        normalized.knowledge_facts,
        |item| item.id.clone(),
        merge_fact_patch,
    );
    normalized.location_edges = dedupe_by_id(
        normalized.location_edges,
        |item| item.id.clone(),
        merge_location_edge_patch,
    );
    normalized.dropped_facts.truncate(MAX_DROPPED_FACTS);
    normalized
}

pub fn classify_relation_candidate_v3(
    candidate: RelationCandidateV3,
    context: &AiBookWorkingContextV3,
) -> RelationClassificationV3 {
    let preview = format!(
        "{} -> {} ({})",
        candidate.source_name, candidate.target_name, candidate.kind_raw
    );
    if candidate.evidence.is_empty() {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "missing_evidence",
            preview,
            None,
            context.current_chapter_index,
        ));
    }

    let source_character = find_character_ref(context, &candidate.source_name);
    let target_character = find_character_ref(context, &candidate.target_name);
    let source_location = find_location_ref(context, &candidate.source_name);
    let target_location = find_location_ref(context, &candidate.target_name);
    let kind_key = canonical_key(&candidate.kind_raw);

    if (source_character.is_some() && target_location.is_some())
        || (source_location.is_some() && target_character.is_some())
        || matches_location_relation(&kind_key)
    {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "person_location_relation",
            preview,
            None,
            context.current_chapter_index,
        ));
    }

    if source_location.is_some() && target_location.is_some() {
        if let (Some(source_location), Some(target_location)) = (source_location, target_location) {
            let edge_kind = normalize_location_edge_kind(&candidate.kind_raw);
            if edge_kind == AiBookLocationEdgeKind::Unknown {
                return RelationClassificationV3::Drop(make_dropped_fact(
                    "location_edge_relation",
                    preview,
                    None,
                    context.current_chapter_index,
                ));
            }
            let edge = NormalizedLocationEdgeV3 {
                id: stable_location_edge_id(
                    &source_location.id,
                    &target_location.id,
                    &edge_kind,
                ),
                source_location_id: source_location.id,
                source_name: source_location.name,
                target_location_id: target_location.id,
                target_name: target_location.name,
                kind: edge_kind,
                label: clean_optional(candidate.description.clone()),
                evidence: dedupe_evidence(candidate.evidence),
            };
            return RelationClassificationV3::Redirect(RelationRedirectV3::LocationEdge(edge));
        }
    }

    if is_person_ability_relation(&kind_key, &candidate.target_name, source_character.is_some()) {
        if let Some(source_character) = source_character {
            let ability = clean_required(&candidate.target_name);
            let current_status = clean_optional(candidate.description.clone());
            let state = NormalizedCharacterStateV3 {
                character_id: source_character.id,
                canonical_name: source_character.name,
                current_status,
                abilities: if ability.is_empty() { Vec::new() } else { vec![ability] },
                evidence: dedupe_evidence(candidate.evidence),
                ..NormalizedCharacterStateV3::default()
            };
            return RelationClassificationV3::Redirect(RelationRedirectV3::CharacterState(state));
        }
    }

    if is_person_item_relation(&kind_key) {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "person_item_relation",
            preview,
            None,
            context.current_chapter_index,
        ));
    }

    if is_transient_relation(&kind_key, candidate.description.as_deref()) {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "transient_action",
            preview,
            None,
            context.current_chapter_index,
        ));
    }

    if is_low_value_relation(&kind_key, candidate.description.as_deref()) {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "low_value_relation",
            preview,
            None,
            context.current_chapter_index,
        ));
    }

    let Some(source_character) = source_character else {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "invalid_entity",
            preview,
            None,
            context.current_chapter_index,
        ));
    };
    let Some(target_character) = target_character else {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "invalid_entity",
            preview,
            None,
            context.current_chapter_index,
        ));
    };

    let Some((kind, direction)) = normalize_relation_kind(&kind_key) else {
        return RelationClassificationV3::Drop(make_dropped_fact(
            "invalid_relation_kind",
            preview,
            None,
            context.current_chapter_index,
        ));
    };

    let polarity = normalize_relation_polarity(&candidate.polarity_raw);
    let strength = normalize_relation_strength(&candidate.strength_raw);
    let status = normalize_relation_status(&candidate.status_raw);
    let summary = clean_optional(candidate.description.clone()).unwrap_or_else(|| kind_label(&kind).to_string());
    let label = kind_label(&kind).to_string();
    let id = stable_relation_id(
        &source_character.id,
        &target_character.id,
        &kind,
        None,
        &direction,
    );
    let history = vec![AiBookRelationChangeView {
        chapter_index: context.current_chapter_index.unwrap_or_default(),
        chapter_title: context.current_chapter_title.clone().unwrap_or_default(),
        previous_kind: None,
        next_kind: kind.clone(),
        previous_polarity: None,
        next_polarity: polarity.clone(),
        previous_status: None,
        next_status: status.clone(),
        note: summary.clone(),
        evidence: dedupe_evidence(candidate.evidence.clone()),
    }];

    RelationClassificationV3::Keep(NormalizedCharacterRelationV3 {
        id,
        source_character_id: source_character.id,
        source_name: source_character.name,
        target_character_id: target_character.id,
        target_name: target_character.name,
        kind,
        subtype: None,
        label,
        polarity,
        strength,
        status,
        direction,
        summary: summary.clone(),
        current_dynamics: vec![summary],
        evidence: dedupe_evidence(candidate.evidence),
        history,
    })
}

pub fn merge_ai_book_memory_v3(
    previous: AiBookMemoryV3,
    patch: NormalizedKnowledgePatchV3,
) -> AiBookMemoryV3 {
    let mut memory = previous;
    if let Some(summary) = patch.summary {
        memory.summary.current = summary;
    }

    let mut character_index: HashMap<String, usize> = memory
        .characters
        .iter()
        .enumerate()
        .map(|(index, item)| (stable_character_id(&item.name, &item.aliases), index))
        .collect();

    for character in patch.characters {
        if let Some(index) = character_index.get(&character.id).copied() {
            let existing = &mut memory.characters[index];
            merge_character_into_memory(existing, &character);
        } else {
            let character_value = AiBookCharacterV3 {
                name: character.canonical_name.clone(),
                aliases: dedupe_strings(character.aliases),
                status: character.status.unwrap_or_default(),
                faction: character.faction,
                location: character.location,
                description: character.description,
                last_seen_chapter: None,
            };
            memory.characters.push(character_value);
            character_index.insert(character.id, memory.characters.len() - 1);
        }
    }

    let mut state_index: HashMap<String, usize> = memory
        .character_states
        .iter()
        .enumerate()
        .map(|(index, item)| (stable_character_id(&item.name, &[]), index))
        .collect();
    for state in patch.character_states {
        if let Some(index) = state_index.get(&state.character_id).copied() {
            let existing = &mut memory.character_states[index];
            merge_state_into_memory(existing, &state);
        } else {
            let description = clean_optional(Some(render_state_description(&state)));
            memory.character_states.push(AiBookCharacterStateV3 {
                name: state.canonical_name.clone(),
                status: state.current_status.clone().unwrap_or_default(),
                description,
                last_seen_chapter_index: patch.chapter_index.into(),
                last_seen_chapter_title: None,
                updated_at: Some(now_ts()),
            });
            state_index.insert(state.character_id, memory.character_states.len() - 1);
        }
    }

    let mut relation_index: HashMap<String, usize> = memory
        .character_relations
        .iter()
        .enumerate()
        .map(|(index, item)| (relation_storage_id(item), index))
        .collect();
    for relation in patch.character_relations {
        let description = Some(encode_relation_meta(&relation));
        if let Some(index) = relation_index.get(&relation.id).copied() {
            let existing = &mut memory.character_relations[index];
            existing.kind = relation.kind.clone();
            existing.polarity = relation.polarity.clone();
            existing.strength = relation.strength.clone();
            existing.status = relation.status.clone();
            existing.description = description;
        } else {
            memory.character_relations.push(AiBookCharacterRelationV3 {
                source: relation.source_name.clone(),
                target: relation.target_name.clone(),
                kind: relation.kind.clone(),
                polarity: relation.polarity.clone(),
                strength: relation.strength.clone(),
                status: relation.status.clone(),
                description,
            });
            relation_index.insert(relation.id, memory.character_relations.len() - 1);
        }
    }

    let mut fact_index: HashMap<String, usize> = memory
        .knowledge_facts
        .iter()
        .enumerate()
        .map(|(index, item)| (stable_fact_id(&item.category, &item.title), index))
        .collect();
    for fact in patch.knowledge_facts {
        if let Some(index) = fact_index.get(&fact.id).copied() {
            let existing = &mut memory.knowledge_facts[index];
            existing.content = fact.content.clone();
            existing.category = fact.category.clone();
            existing.confidence = fact.confidence.clone();
            existing.importance = fact.importance.clone();
        } else {
            memory.knowledge_facts.push(AiBookKnowledgeFactV3 {
                title: fact.title.clone(),
                content: fact.content.clone(),
                category: fact.category.clone(),
                confidence: fact.confidence.clone(),
                importance: fact.importance.clone(),
            });
            fact_index.insert(fact.id, memory.knowledge_facts.len() - 1);
        }
    }

    let mut location_index: HashMap<String, usize> = memory
        .locations
        .iter()
        .enumerate()
        .map(|(index, item)| (stable_location_id(&item.name), index))
        .collect();
    for location in patch.locations {
        if let Some(index) = location_index.get(&location.id).copied() {
            let existing = &mut memory.locations[index];
            existing.kind = location.kind.clone().or_else(|| existing.kind.clone());
            existing.description = location
                .description
                .clone()
                .unwrap_or_else(|| existing.description.clone());
            existing.status = location.status.clone().or_else(|| existing.status.clone());
        } else {
            memory.locations.push(AiBookLocationV3 {
                name: location.canonical_name.clone(),
                kind: location.kind.clone(),
                description: location.description.clone().unwrap_or_default(),
                status: location.status.clone(),
                related_characters: Vec::new(),
                first_seen_chapter: None,
            });
            location_index.insert(location.id, memory.locations.len() - 1);
        }
    }

    let mut edge_index: HashMap<String, usize> = memory
        .location_edges
        .iter()
        .enumerate()
        .map(|(index, item)| {
            (
                stable_location_edge_id(
                    &stable_location_id(&item.source),
                    &stable_location_id(&item.target),
                    &item.kind,
                ),
                index,
            )
        })
        .collect();
    for edge in patch.location_edges {
        if let Some(index) = edge_index.get(&edge.id).copied() {
            let existing = &mut memory.location_edges[index];
            existing.kind = edge.kind.clone();
            existing.description = edge.label.clone();
        } else {
            memory.location_edges.push(AiBookLocationEdgeV3 {
                source: edge.source_name.clone(),
                target: edge.target_name.clone(),
                kind: edge.kind.clone(),
                description: edge.label.clone(),
            });
            edge_index.insert(edge.id, memory.location_edges.len() - 1);
        }
    }

    for dropped in patch.dropped_facts {
        memory.dropped_facts.push(dropped);
    }
    if memory.dropped_facts.len() > MAX_DROPPED_FACTS {
        let keep_from = memory.dropped_facts.len() - MAX_DROPPED_FACTS;
        memory.dropped_facts = memory.dropped_facts.split_off(keep_from);
    }
    memory.updated_at = now_ts();
    memory.processed_chapter_index = Some(patch.chapter_index);
    let _ = validate_ai_book_memory_v3(&memory);
    memory
}

pub fn select_ai_book_display_memory_v3(memory: &AiBookMemoryV3) -> AiBookMemoryViewModel {
    let characters = select_character_views(&memory.characters);
    let locations = select_location_views(&memory.locations, &memory.location_edges);
    let relationships = select_relationship_views(&memory.characters, &memory.character_relations);
    let knowledge_facts = memory
        .knowledge_facts
        .iter()
        .map(|fact| AiBookKnowledgeFactView {
            id: stable_fact_id(&fact.category, &fact.title),
            category: fact.category.clone(),
            title: fact.title.clone(),
            content: fact.content.clone(),
            confidence: fact.confidence.clone(),
            importance: fact.importance.clone(),
            first_seen_chapter_index: None,
            last_confirmed_chapter_index: None,
            evidence: Vec::new(),
        })
        .collect();
    let cleanup = summarize_dropped_facts(&memory.dropped_facts);
    let map = if memory.map_state.is_some()
        || memory.render_artifacts.is_some()
        || !memory.locations.is_empty()
        || !memory.location_edges.is_empty()
    {
        Some(AiBookMapView {
            state: memory.map_state.clone(),
            render_artifacts: memory.render_artifacts.clone(),
            locations: locations.clone(),
            location_edges: memory.location_edges.clone(),
        })
    } else {
        None
    };

    AiBookMemoryViewModel {
        book_url: memory.book_url.clone(),
        book_name: memory.book_name.clone(),
        author: memory.author.clone(),
        enabled: memory.enabled,
        processed_chapter_index: memory.processed_chapter_index,
        processed_chapter_title: memory.processed_chapter_title.clone(),
        updated_at: memory.updated_at,
        summary: memory.summary.clone(),
        characters,
        relationships,
        knowledge_facts,
        locations,
        map,
        cleanup,
        catchup_stats: memory.catchup_stats.clone(),
        last_error: memory.last_error.clone(),
        last_error_chapter_index: memory.last_error_chapter_index,
        last_error_chapter_title: memory.last_error_chapter_title.clone(),
    }
}

pub fn select_ai_book_chapter_view_v3(
    memory: &AiBookMemoryV3,
    chapter_index: i32,
) -> AiBookChapterMemoryViewModel {
    let digest = memory
        .chapter_digests
        .iter()
        .find(|item| item.chapter_index == chapter_index)
        .cloned();

    if let Some(digest) = digest.clone() {
        let temp_memory = AiBookMemoryV3 {
            book_url: memory.book_url.clone(),
            book_name: memory.book_name.clone(),
            author: memory.author.clone(),
            characters: digest.characters.clone(),
            character_relations: digest.character_relations.clone(),
            knowledge_facts: digest.knowledge_facts.clone(),
            locations: digest.locations.clone(),
            location_edges: digest.location_edges.clone(),
            ..AiBookMemoryV3::default()
        };
        let display = select_ai_book_display_memory_v3(&temp_memory);
        AiBookChapterMemoryViewModel {
            book_url: memory.book_url.clone(),
            chapter_index,
            chapter_title: Some(digest.chapter_title.clone()),
            digest: Some(digest),
            characters: display.characters,
            relationships: display.relationships,
            knowledge_facts: display.knowledge_facts,
            locations: display.locations,
            generation_status: "cached".to_string(),
            last_error: None,
        }
    } else {
        AiBookChapterMemoryViewModel {
            book_url: memory.book_url.clone(),
            chapter_index,
            chapter_title: None,
            digest: None,
            characters: Vec::new(),
            relationships: Vec::new(),
            knowledge_facts: Vec::new(),
            locations: Vec::new(),
            generation_status: "missing".to_string(),
            last_error: None,
        }
    }
}

pub fn select_working_context_v3(
    memory: &AiBookMemoryV3,
    chapter_digest: Option<&AiBookChapterDigestV3>,
    chapter_text: &str,
) -> AiBookWorkingContextV3 {
    let mentioned_names = collect_mentions(memory, chapter_digest, chapter_text);
    let character_states = state_map(&memory.character_states);

    let recent_chapter_digests = memory
        .chapter_digests
        .iter()
        .rev()
        .take(MAX_RECENT_CHAPTER_DIGESTS)
        .map(|digest| WorkingContextDigestV3 {
            chapter_index: digest.chapter_index,
            chapter_title: digest.chapter_title.clone(),
            digest: digest.summary.clone(),
            key_events: digest.key_points.clone(),
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let relevant_characters = memory
        .characters
        .iter()
        .filter(|character| mentioned_names.characters.contains(&stable_character_id(&character.name, &character.aliases)))
        .take(MAX_RELEVANT_CHARACTERS)
        .map(|character| {
            let id = stable_character_id(&character.name, &character.aliases);
            let state = character_states.get(&id);
            WorkingContextCharacterV3 {
                id,
                name: character.name.clone(),
                aliases: dedupe_strings(character.aliases.clone()),
                status: clean_optional(Some(character.status.clone())).or_else(|| {
                    state.and_then(|item| clean_optional(Some(item.status.clone())))
                }),
                affiliations: Vec::new(),
                abilities: state
                    .and_then(|item| item.description.as_deref())
                    .map(parse_state_abilities)
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    let allowed_character_ids: HashSet<_> = relevant_characters.iter().map(|item| item.id.clone()).collect();
    let relevant_character_name_map: HashMap<_, _> = relevant_characters
        .iter()
        .map(|item| (item.id.clone(), item.name.clone()))
        .collect();
    let relevant_relations = select_relationship_views(&memory.characters, &memory.character_relations)
        .into_iter()
        .filter(|relation| {
            allowed_character_ids.contains(&relation.source_character_id)
                && allowed_character_ids.contains(&relation.target_character_id)
        })
        .take(MAX_RELEVANT_RELATIONS)
        .map(|relation| WorkingContextRelationV3 {
            source_name: relevant_character_name_map
                .get(&relation.source_character_id)
                .cloned()
                .unwrap_or_else(|| relation.source_character_id.clone()),
            target_name: relevant_character_name_map
                .get(&relation.target_character_id)
                .cloned()
                .unwrap_or_else(|| relation.target_character_id.clone()),
            source_character_id: relation.source_character_id,
            target_character_id: relation.target_character_id,
            kind: format!("{:?}", relation.kind),
            subtype: relation.subtype,
            label: relation.label,
            polarity: format!("{:?}", relation.polarity),
            status: format!("{:?}", relation.status),
        })
        .collect();

    let relevant_knowledge_facts = memory
        .knowledge_facts
        .iter()
        .filter(|fact| chapter_text.contains(&fact.title) || mentioned_names.fact_titles.contains(&fact.title))
        .chain(
            memory
                .knowledge_facts
                .iter()
                .filter(|fact| fact.importance == AiBookFactImportance::High),
        )
        .take(MAX_RELEVANT_FACTS)
        .map(|fact| WorkingContextFactV3 {
            id: stable_fact_id(&fact.category, &fact.title),
            title: fact.title.clone(),
            category: format!("{:?}", fact.category),
            content: fact.content.clone(),
        })
        .collect::<Vec<_>>();

    let relevant_locations = memory
        .locations
        .iter()
        .filter(|location| mentioned_names.locations.contains(&stable_location_id(&location.name)))
        .take(MAX_RELEVANT_LOCATIONS)
        .map(|location| WorkingContextLocationV3 {
            id: stable_location_id(&location.name),
            name: location.name.clone(),
            aliases: Vec::new(),
            parent_name: parent_location_name(location, &memory.location_edges),
            kind: location.kind.clone().unwrap_or_else(|| "unknown".to_string()),
            scale: "unknown".to_string(),
        })
        .collect();

    AiBookWorkingContextV3 {
        book_name: memory.book_name.clone(),
        author: memory.author.clone(),
        summary_current: memory.summary.current.clone(),
        recent_chapter_digests,
        relevant_characters,
        relevant_relations,
        relevant_knowledge_facts: dedupe_context_facts(relevant_knowledge_facts),
        relevant_locations,
        schema_hint: "KnowledgePatchV3".to_string(),
        current_chapter_index: chapter_digest.map(|item| item.chapter_index),
        current_chapter_title: chapter_digest.map(|item| item.chapter_title.clone()),
    }
}

fn select_character_views(characters: &[AiBookCharacterV3]) -> Vec<AiBookCharacterView> {
    characters
        .iter()
        .map(|character| AiBookCharacterView {
            id: stable_character_id(&character.name, &character.aliases),
            name: character.name.clone(),
            aliases: dedupe_strings(character.aliases.clone()),
            importance: "moderate".to_string(),
            description: character.description.clone(),
            first_seen_chapter_index: None,
            last_seen_chapter_index: None,
            evidence: Vec::new(),
        })
        .collect()
}

fn select_location_views(
    locations: &[AiBookLocationV3],
    location_edges: &[AiBookLocationEdgeV3],
) -> Vec<AiBookLocationView> {
    locations
        .iter()
        .map(|location| AiBookLocationView {
            id: stable_location_id(&location.name),
            name: location.name.clone(),
            aliases: Vec::new(),
            kind: location.kind.clone().unwrap_or_else(|| "unknown".to_string()),
            scale: "unknown".to_string(),
            parent_location_id: location_edges.iter().find_map(|edge| {
                (edge.kind == AiBookLocationEdgeKind::Contains && edge.target == location.name)
                    .then(|| stable_location_id(&edge.source))
            }),
            description: location.description.clone(),
            current_status: location.status.clone(),
            importance: "moderate".to_string(),
            first_seen_chapter_index: None,
            last_seen_chapter_index: None,
            evidence: Vec::new(),
        })
        .collect()
}

fn select_relationship_views(
    characters: &[AiBookCharacterV3],
    relations: &[AiBookCharacterRelationV3],
) -> Vec<AiBookRelationView> {
    let character_name_map: HashMap<String, String> = characters
        .iter()
        .map(|character| {
            (
                stable_character_id(&character.name, &character.aliases),
                character.name.clone(),
            )
        })
        .collect();

    let mut grouped: BTreeMap<String, Vec<AiBookRelationView>> = BTreeMap::new();
    for relation in relations {
        if relation.status == AiBookRelationStatus::Broken {
            continue;
        }
        let view = relation_to_view(relation, &character_name_map);
        let group_key = canonical_pair_key(&view.source_character_id, &view.target_character_id);
        grouped.entry(group_key).or_default().push(view);
    }

    grouped
        .into_values()
        .map(|group| {
            let primary = group.first().cloned().unwrap_or_default();
            let mut current_dynamics = Vec::new();
            let mut evidence = Vec::new();
            let mut history = Vec::new();
            let facets = group
                .iter()
                .map(|relation| {
                    current_dynamics.extend(relation.current_dynamics.clone());
                    evidence.extend(relation.evidence.clone());
                    history.extend(relation.history.clone());
                    AiBookRelationFacetView {
                        kind: relation.kind.clone(),
                        subtype: relation.subtype.clone(),
                        polarity: relation.polarity.clone(),
                        status: relation.status.clone(),
                        summary: relation.summary.clone(),
                    }
                })
                .collect();
            AiBookRelationView {
                id: format!("pair:{}", canonical_pair_key(&primary.source_character_id, &primary.target_character_id)),
                source_character_id: primary.source_character_id.clone(),
                target_character_id: primary.target_character_id.clone(),
                kind: primary.kind.clone(),
                subtype: primary.subtype.clone(),
                label: primary.label.clone(),
                polarity: primary.polarity.clone(),
                strength: primary.strength.clone(),
                status: primary.status.clone(),
                direction: if group.len() > 1 {
                    "grouped".to_string()
                } else {
                    primary.direction.clone()
                },
                summary: primary.summary.clone(),
                current_dynamics: dedupe_strings(current_dynamics),
                facets,
                first_seen_chapter_index: primary.first_seen_chapter_index,
                last_updated_chapter_index: primary.last_updated_chapter_index,
                evidence: dedupe_evidence(evidence),
                history,
            }
        })
        .collect()
}

fn relation_to_view(
    relation: &AiBookCharacterRelationV3,
    character_name_map: &HashMap<String, String>,
) -> AiBookRelationView {
    let meta = decode_relation_meta(relation);
    let fallback_source_id = stable_character_id(&relation.source, &[]);
    let fallback_target_id = stable_character_id(&relation.target, &[]);
    let source_id = meta
        .as_ref()
        .map(|item| item.source_character_id.clone())
        .unwrap_or(fallback_source_id.clone());
    let target_id = meta
        .as_ref()
        .map(|item| item.target_character_id.clone())
        .unwrap_or(fallback_target_id.clone());
    let source_name = character_name_map
        .get(&source_id)
        .cloned()
        .unwrap_or_else(|| relation.source.clone());
    let target_name = character_name_map
        .get(&target_id)
        .cloned()
        .unwrap_or_else(|| relation.target.clone());
    let label = meta
        .as_ref()
        .map(|item| item.label.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{} · {}", source_name, target_name));
    let summary = meta
        .as_ref()
        .map(|item| item.summary.clone())
        .unwrap_or_else(|| relation.description.clone().unwrap_or_default());
    AiBookRelationView {
        id: meta.as_ref().map(|item| item.id.clone()).unwrap_or_else(|| {
            stable_relation_id(
                &source_id,
                &target_id,
                &relation.kind,
                None,
                &if is_directed_relation_kind(&relation.kind) {
                    RelationDirectionV3::Directed
                } else {
                    RelationDirectionV3::Undirected
                },
            )
        }),
        source_character_id: source_id,
        target_character_id: target_id,
        kind: relation.kind.clone(),
        subtype: meta.as_ref().and_then(|item| item.subtype.clone()),
        label,
        polarity: relation.polarity.clone(),
        strength: relation.strength.clone(),
        status: relation.status.clone(),
        direction: meta
            .as_ref()
            .map(|item| item.direction.clone())
            .unwrap_or_else(|| {
                if is_directed_relation_kind(&relation.kind) {
                    "directed".to_string()
                } else {
                    "undirected".to_string()
                }
            }),
        summary: summary.clone(),
        current_dynamics: meta
            .as_ref()
            .map(|item| item.current_dynamics.clone())
            .unwrap_or_else(|| vec![summary.clone()]),
        facets: Vec::new(),
        first_seen_chapter_index: meta
            .as_ref()
            .and_then(|item| item.history.first().map(|history| history.chapter_index)),
        last_updated_chapter_index: meta
            .as_ref()
            .and_then(|item| item.history.last().map(|history| history.chapter_index)),
        evidence: meta
            .as_ref()
            .map(|item| item.evidence.clone())
            .unwrap_or_default(),
        history: meta.as_ref().map(|item| item.history.clone()).unwrap_or_default(),
    }
}

fn summarize_dropped_facts(dropped_facts: &[AiBookDroppedFactV3]) -> AiBookDroppedFactsSummary {
    let mut dropped_by_reason = BTreeMap::new();
    for dropped in dropped_facts {
        *dropped_by_reason.entry(dropped.reason.clone()).or_insert(0) += 1;
    }
    AiBookDroppedFactsSummary {
        dropped_facts_count: dropped_facts.len() as i32,
        dropped_by_reason,
        old_schema_backed_up: false,
    }
}

fn normalize_character_patch(
    patch: &AiBookCharacterPatchV3,
    context: &AiBookWorkingContextV3,
    evidence: &[AiBookEvidenceV3],
) -> Option<NormalizedCharacterV3> {
    if !has_required_evidence(evidence) {
        return None;
    }
    let names = merge_name_candidates(std::slice::from_ref(&patch.name), &patch.aliases);
    if names.is_empty() {
        return None;
    }
    let resolved = names.iter().find_map(|name| find_character_ref(context, name));
    let (id, canonical_name) = if let Some(resolved) = resolved {
        let canonical_name = if resolved.name.is_empty() {
            names[0].clone()
        } else {
            resolved.name
        };
        (resolved.id, canonical_name)
    } else {
        let canonical_name = names[0].clone();
        (
            stable_character_id(&canonical_name, &[]),
            canonical_name,
        )
    };
    let aliases = merge_name_candidates(std::slice::from_ref(&canonical_name), &names);
    Some(NormalizedCharacterV3 {
        id,
        canonical_name,
        aliases,
        status: clean_optional(patch.status.clone()),
        faction: clean_optional(patch.faction.clone()),
        location: clean_optional(patch.location.clone()),
        description: clean_optional(patch.description.clone()),
        evidence: dedupe_evidence(evidence.to_vec()),
    })
}

fn normalize_character_state_patch(
    patch: &AiBookCharacterStatePatchV3,
    context: &AiBookWorkingContextV3,
    evidence: &[AiBookEvidenceV3],
) -> Option<NormalizedCharacterStateV3> {
    if !has_required_evidence(evidence) {
        return None;
    }
    let name = clean_required(&patch.name);
    if name.is_empty() {
        return None;
    }
    let resolved = find_character_ref(context, &name).unwrap_or(CharacterRef {
        id: stable_character_id(&name, &[]),
        name: name.clone(),
    });
    Some(NormalizedCharacterStateV3 {
        character_id: resolved.id,
        canonical_name: resolved.name,
        current_status: clean_optional(Some(patch.status.clone())),
        evidence: dedupe_evidence(evidence.to_vec()),
        ..NormalizedCharacterStateV3::default()
    })
}

fn normalize_fact_patch(
    patch: &AiBookKnowledgeFactPatchV3,
    evidence: &[AiBookEvidenceV3],
) -> Option<NormalizedKnowledgeFactV3> {
    if !has_required_evidence(evidence) {
        return None;
    }
    let title = clean_required(&patch.title);
    let content = clean_required(&patch.content);
    if title.is_empty() || content.is_empty() {
        return None;
    }
    let category = normalize_fact_category(&patch.category);
    Some(NormalizedKnowledgeFactV3 {
        id: stable_fact_id(&category, &title),
        title,
        content,
        category,
        confidence: normalize_fact_confidence(&patch.confidence),
        importance: normalize_fact_importance(&patch.importance),
        evidence: dedupe_evidence(evidence.to_vec()),
    })
}

fn normalize_location_patch(
    patch: &AiBookLocationPatchV3,
    context: &AiBookWorkingContextV3,
    evidence: &[AiBookEvidenceV3],
) -> Option<NormalizedLocationV3> {
    if !has_required_evidence(evidence) {
        return None;
    }
    let name = clean_required(&patch.name);
    if name.is_empty() {
        return None;
    }
    let resolved = find_location_ref(context, &name).unwrap_or(LocationRef {
        id: stable_location_id(&name),
        name: name.clone(),
    });
    Some(NormalizedLocationV3 {
        id: resolved.id,
        canonical_name: resolved.name,
        kind: clean_optional(patch.kind.clone()),
        description: clean_optional(Some(patch.description.clone())),
        status: clean_optional(patch.status.clone()),
        evidence: dedupe_evidence(evidence.to_vec()),
    })
}

fn normalize_location_edge_patch(
    patch: &AiBookLocationEdgePatchV3,
    context: &AiBookWorkingContextV3,
    evidence: &[AiBookEvidenceV3],
) -> Option<NormalizedLocationEdgeV3> {
    if !has_required_evidence(evidence) {
        return None;
    }
    let source = find_location_ref(context, &patch.source)?;
    let target = find_location_ref(context, &patch.target)?;
    let kind = normalize_location_edge_kind(&patch.kind);
    if kind == AiBookLocationEdgeKind::Unknown {
        return None;
    }
    Some(NormalizedLocationEdgeV3 {
        id: stable_location_edge_id(&source.id, &target.id, &kind),
        source_location_id: source.id,
        source_name: source.name,
        target_location_id: target.id,
        target_name: target.name,
        kind,
        label: clean_optional(patch.description.clone()),
        evidence: dedupe_evidence(evidence.to_vec()),
    })
}

fn relation_candidate_from_patch(
    patch: &AiBookCharacterRelationPatchV3,
    context: &AiBookWorkingContextV3,
    evidence: &[AiBookEvidenceV3],
) -> RelationCandidateV3 {
    let description = clean_optional(patch.description.clone());
    let fallback_note = description.clone().unwrap_or_else(|| format!("{} {} {}", patch.source, patch.kind, patch.target));
    RelationCandidateV3 {
        source_name: patch.source.clone(),
        target_name: patch.target.clone(),
        kind_raw: patch.kind.clone(),
        polarity_raw: patch.polarity.clone(),
        strength_raw: patch.strength.clone(),
        status_raw: patch.status.clone(),
        description,
        evidence: if evidence.is_empty() {
            patch_evidence(context, 0)
        } else {
            evidence
                .iter()
                .map(|item| AiBookEvidenceV3 {
                    note: if item.note.is_empty() { fallback_note.clone() } else { item.note.clone() },
                    ..item.clone()
                })
                .collect()
        },
    }
}

fn extend_context(
    context: &AiBookWorkingContextV3,
    characters: &[NormalizedCharacterV3],
    locations: &[NormalizedLocationV3],
) -> AiBookWorkingContextV3 {
    let mut next = context.clone();
    for character in characters {
        if !next.relevant_characters.iter().any(|item| item.id == character.id) {
            next.relevant_characters.push(WorkingContextCharacterV3 {
                id: character.id.clone(),
                name: character.canonical_name.clone(),
                aliases: character.aliases.clone(),
                status: character.status.clone(),
                affiliations: Vec::new(),
                abilities: Vec::new(),
            });
        }
    }
    for location in locations {
        if !next.relevant_locations.iter().any(|item| item.id == location.id) {
            next.relevant_locations.push(WorkingContextLocationV3 {
                id: location.id.clone(),
                name: location.canonical_name.clone(),
                aliases: Vec::new(),
                parent_name: None,
                kind: location.kind.clone().unwrap_or_else(|| "unknown".to_string()),
                scale: "unknown".to_string(),
            });
        }
    }
    next
}

fn patch_evidence(context: &AiBookWorkingContextV3, chapter_index: i32) -> Vec<AiBookEvidenceV3> {
    match (context.current_chapter_title.clone(), context.current_chapter_index.or(Some(chapter_index))) {
        (Some(chapter_title), Some(chapter_index)) if !chapter_title.is_empty() => vec![AiBookEvidenceV3 {
            chapter_index,
            chapter_title,
            quote: None,
            note: context.summary_current.clone(),
        }],
        _ => Vec::new(),
    }
}

fn merge_character_into_memory(existing: &mut AiBookCharacterV3, patch: &NormalizedCharacterV3) {
    existing.aliases = merge_name_candidates(
        std::slice::from_ref(&existing.name),
        &merge_name_candidates(std::slice::from_ref(&patch.canonical_name), &patch.aliases),
    );
    existing.status = patch.status.clone().unwrap_or_else(|| existing.status.clone());
    existing.faction = patch.faction.clone().or_else(|| existing.faction.clone());
    existing.location = patch.location.clone().or_else(|| existing.location.clone());
    existing.description = patch.description.clone().or_else(|| existing.description.clone());
}

fn merge_state_into_memory(existing: &mut AiBookCharacterStateV3, patch: &NormalizedCharacterStateV3) {
    existing.status = patch.current_status.clone().unwrap_or_else(|| existing.status.clone());
    if let Some(description) = clean_optional(Some(render_state_description(patch))) {
        existing.description = Some(description);
    }
    existing.updated_at = Some(now_ts());
}

fn render_state_description(patch: &NormalizedCharacterStateV3) -> String {
    let mut parts = Vec::new();
    if !patch.abilities.is_empty() {
        parts.push(format!("abilities={}", patch.abilities.join("|")));
    }
    if !patch.affiliations.is_empty() {
        parts.push(format!("affiliations={}", patch.affiliations.join("|")));
    }
    if !patch.resources.is_empty() {
        parts.push(format!("resources={}", patch.resources.join("|")));
    }
    if let Some(location_id) = &patch.current_location_id {
        parts.push(format!("location={location_id}"));
    }
    parts.join(";")
}

fn parse_state_abilities(description: &str) -> Vec<String> {
    description
        .split(';')
        .find_map(|segment| segment.strip_prefix("abilities="))
        .map(|value| value.split('|').map(|item| item.to_string()).collect())
        .unwrap_or_default()
}

fn encode_relation_meta(relation: &NormalizedCharacterRelationV3) -> String {
    let meta = RelationStorageMetaV3 {
        id: relation.id.clone(),
        source_character_id: relation.source_character_id.clone(),
        target_character_id: relation.target_character_id.clone(),
        source_name: relation.source_name.clone(),
        target_name: relation.target_name.clone(),
        subtype: relation.subtype.clone(),
        label: relation.label.clone(),
        direction: relation.direction.as_str().to_string(),
        summary: relation.summary.clone(),
        current_dynamics: relation.current_dynamics.clone(),
        evidence: relation.evidence.clone(),
        history: relation.history.clone(),
    };
    format!("{V3_RELATION_META_PREFIX}{}", serde_json::to_string(&meta).unwrap_or_default())
}

fn decode_relation_meta(relation: &AiBookCharacterRelationV3) -> Option<RelationStorageMetaV3> {
    let raw = relation.description.as_deref()?;
    let payload = raw.strip_prefix(V3_RELATION_META_PREFIX)?;
    serde_json::from_str(payload).ok()
}

fn relation_storage_id(relation: &AiBookCharacterRelationV3) -> String {
    decode_relation_meta(relation)
        .map(|meta| meta.id)
        .unwrap_or_else(|| {
            stable_relation_id(
                &stable_character_id(&relation.source, &[]),
                &stable_character_id(&relation.target, &[]),
                &relation.kind,
                None,
                &if is_directed_relation_kind(&relation.kind) {
                    RelationDirectionV3::Directed
                } else {
                    RelationDirectionV3::Undirected
                },
            )
        })
}

impl RelationDirectionV3 {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Directed => "directed",
            Self::Undirected => "undirected",
        }
    }
}

fn is_directed_relation_kind(kind: &AiBookRelationKind) -> bool {
    matches!(kind, AiBookRelationKind::Supervision)
}

fn normalize_relation_kind(raw: &str) -> Option<(AiBookRelationKind, RelationDirectionV3)> {
    let value = raw.replace('_', "-");
    match value.as_str() {
        "family" | "亲属" | "家人" => Some((AiBookRelationKind::Family, RelationDirectionV3::Undirected)),
        "romantic" | "romance" | "恋爱" | "感情" => Some((AiBookRelationKind::Romance, RelationDirectionV3::Undirected)),
        "friend" | "friendship" | "ally" | "peer" | "朋友" | "互助" | "借贷互助" => {
            Some((AiBookRelationKind::Friendship, RelationDirectionV3::Undirected))
        }
        "alliance" | "盟友" => Some((AiBookRelationKind::Alliance, RelationDirectionV3::Undirected)),
        "enemy" | "conflict" | "hostile" | "敌对" => {
            Some((AiBookRelationKind::Conflict, RelationDirectionV3::Undirected))
        }
        "rival" | "rivalry" | "竞争" => Some((AiBookRelationKind::Rivalry, RelationDirectionV3::Undirected)),
        "mentor-student" | "mentorstudent" | "superior-subordinate" | "teacher" | "supervision" | "师生" | "诱导" => {
            Some((AiBookRelationKind::Supervision, RelationDirectionV3::Directed))
        }
        _ => None,
    }
}

fn normalize_relation_polarity(raw: &str) -> AiBookRelationPolarity {
    match canonical_key(raw).as_str() {
        "positive" | "正向" => AiBookRelationPolarity::Positive,
        "negative" | "负向" => AiBookRelationPolarity::Negative,
        "mixed" | "复杂" => AiBookRelationPolarity::Mixed,
        _ => AiBookRelationPolarity::Neutral,
    }
}

fn normalize_relation_strength(raw: &str) -> AiBookRelationStrength {
    match canonical_key(raw).as_str() {
        "major" | "strong" | "critical" | "强" => AiBookRelationStrength::Strong,
        "moderate" | "中" => AiBookRelationStrength::Moderate,
        "minor" | "weak" | "弱" => AiBookRelationStrength::Weak,
        _ => AiBookRelationStrength::Moderate,
    }
}

fn normalize_relation_status(raw: &str) -> AiBookRelationStatus {
    match canonical_key(raw).as_str() {
        "ended" | "broken" | "结束" => AiBookRelationStatus::Broken,
        "changed" | "developing" | "变化" => AiBookRelationStatus::Developing,
        "active" | "ongoing" | "持续" => AiBookRelationStatus::Active,
        _ => AiBookRelationStatus::Active,
    }
}

fn normalize_location_edge_kind(raw: &str) -> AiBookLocationEdgeKind {
    match canonical_key(raw).as_str() {
        "contains" | "part-of" | "包含" => AiBookLocationEdgeKind::Contains,
        "adjacent" | "相邻" => AiBookLocationEdgeKind::Adjacent,
        "route" | "leadsto" | "路径" => AiBookLocationEdgeKind::LeadsTo,
        _ => AiBookLocationEdgeKind::Unknown,
    }
}

fn normalize_fact_category(raw: &str) -> AiBookFactCategory {
    match canonical_key(raw).as_str() {
        "basic-rule" | "基础规则" => AiBookFactCategory::BasicRule,
        "power-faction" | "势力制度" => AiBookFactCategory::PowerFaction,
        "history-legend" | "历史传说" => AiBookFactCategory::HistoryLegend,
        "tech-magic" | "技术魔法" => AiBookFactCategory::TechMagic,
        "social-culture" | "社会文化" => AiBookFactCategory::SocialCulture,
        "geography" | "地理环境" => AiBookFactCategory::Geography,
        "organization" | "组织体系" => AiBookFactCategory::Organization,
        _ => AiBookFactCategory::Unknown,
    }
}

fn normalize_fact_confidence(raw: &str) -> AiBookFactConfidence {
    match canonical_key(raw).as_str() {
        "high" | "已知" => AiBookFactConfidence::High,
        "medium" | "推断" => AiBookFactConfidence::Medium,
        "low" | "未知" => AiBookFactConfidence::Low,
        _ => AiBookFactConfidence::Unknown,
    }
}

fn normalize_fact_importance(raw: &str) -> AiBookFactImportance {
    match canonical_key(raw).as_str() {
        "high" | "major" => AiBookFactImportance::High,
        "medium" | "moderate" => AiBookFactImportance::Medium,
        "low" | "minor" => AiBookFactImportance::Low,
        _ => AiBookFactImportance::Unknown,
    }
}

fn kind_label(kind: &AiBookRelationKind) -> &'static str {
    match kind {
        AiBookRelationKind::Family => "family",
        AiBookRelationKind::Romance => "romantic",
        AiBookRelationKind::Friendship => "friend",
        AiBookRelationKind::Rivalry => "rival",
        AiBookRelationKind::Alliance => "ally",
        AiBookRelationKind::Conflict => "enemy",
        AiBookRelationKind::Supervision => "superior_subordinate",
        AiBookRelationKind::Affiliation => "affiliation",
        AiBookRelationKind::Unknown => "unknown_significant",
    }
}

fn make_dropped_fact(
    reason: &str,
    preview: String,
    redirected_to: Option<&str>,
    chapter_index: Option<i32>,
) -> AiBookDroppedFactV3 {
    AiBookDroppedFactV3 {
        source: "model_patch".to_string(),
        reason: reason.to_string(),
        original_value_preview: preview.clone(),
        original_value_hash: md5_hex(&preview),
        redirected_to: redirected_to.map(str::to_string),
        chapter_index,
        created_at: now_ts(),
    }
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for value in values {
        let cleaned = clean_required(&value);
        if cleaned.is_empty() {
            continue;
        }
        let key = canonical_key(&cleaned);
        if seen.insert(key) {
            items.push(cleaned);
        }
    }
    items
}

fn dedupe_evidence(values: Vec<AiBookEvidenceV3>) -> Vec<AiBookEvidenceV3> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for value in values {
        let key = format!(
            "{}:{}:{}:{}",
            value.chapter_index,
            value.chapter_title,
            value.quote.clone().unwrap_or_default(),
            value.note
        );
        if seen.insert(key) {
            items.push(value);
        }
    }
    items
}

fn has_required_evidence(evidence: &[AiBookEvidenceV3]) -> bool {
    !evidence.is_empty()
}

fn canonical_key(value: &str) -> String {
    let mut output = String::new();
    let mut last_sep = false;
    for ch in value.trim().chars() {
        if ch.is_alphanumeric() {
            for lowered in ch.to_lowercase() {
                output.push(lowered);
            }
            last_sep = false;
        } else if !last_sep && !output.is_empty() {
            output.push('-');
            last_sep = true;
        }
    }
    output.trim_matches('-').to_string()
}

fn clean_required(value: &str) -> String {
    value.trim().to_string()
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn merge_name_candidates<'a>(primary: impl IntoIterator<Item = &'a String>, aliases: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    for value in primary {
        values.push(value.clone());
    }
    values.extend(aliases.iter().cloned());
    dedupe_strings(values)
}

fn stable_character_id(name: &str, _aliases: &[String]) -> String {
    format!("character:{}", canonical_key(name))
}

fn stable_location_id(name: &str) -> String {
    format!("location:{}", canonical_key(name))
}

fn stable_fact_id(category: &AiBookFactCategory, title: &str) -> String {
    format!("fact:{:?}:{}", category, canonical_key(title))
}

fn stable_relation_id(
    source_character_id: &str,
    target_character_id: &str,
    kind: &AiBookRelationKind,
    subtype: Option<&str>,
    direction: &RelationDirectionV3,
) -> String {
    let subtype = subtype.map(canonical_key).unwrap_or_default();
    let direction_key = direction.as_str();
    let (left, right) = match direction {
        RelationDirectionV3::Directed => (source_character_id.to_string(), target_character_id.to_string()),
        RelationDirectionV3::Undirected => {
            if source_character_id <= target_character_id {
                (source_character_id.to_string(), target_character_id.to_string())
            } else {
                (target_character_id.to_string(), source_character_id.to_string())
            }
        }
    };
    format!("relation:{left}:{right}:{:?}:{subtype}:{direction_key}", kind)
}

fn stable_location_edge_id(source_location_id: &str, target_location_id: &str, kind: &AiBookLocationEdgeKind) -> String {
    format!("location-edge:{source_location_id}:{target_location_id}:{:?}", kind)
}

fn canonical_pair_key(left: &str, right: &str) -> String {
    if left <= right {
        format!("{left}:{right}")
    } else {
        format!("{right}:{left}")
    }
}

fn matches_location_relation(kind_key: &str) -> bool {
    matches!(kind_key, "located-in" | "in" | "at" | "位于" | "在" | "就读")
}

fn is_person_ability_relation(kind_key: &str, target_name: &str, source_is_character: bool) -> bool {
    source_is_character
        && (matches!(
            kind_key,
            "ability" | "skill" | "cultivate" | "learn" | "master" | "修炼" | "学习" | "掌握"
        ) || target_name.contains('式')
            || target_name.contains('术')
            || target_name.contains('法'))
}

fn is_person_item_relation(kind_key: &str) -> bool {
    matches!(kind_key, "item" | "owns" | "purchase" | "拥有" | "购买")
}

fn is_transient_relation(kind_key: &str, description: Option<&str>) -> bool {
    matches!(kind_key, "see" | "hear" | "pass-by" | "说话" | "出现")
        || description
            .map(|text| text.contains("路过") || text.contains("看到") || text.contains("出现在大屏幕"))
            .unwrap_or(false)
}

fn is_low_value_relation(kind_key: &str, description: Option<&str>) -> bool {
    matches!(kind_key, "know" | "acquaintance" | "schoolmate" | "classmate" | "认识")
        || description
            .map(|text| {
                [
                    "同校",
                    "认识",
                    "同屏出现",
                    "同班",
                    "只是认识",
                    "仅仅认识",
                    "路人互动",
                ]
                .iter()
                .any(|needle| text.contains(needle))
            })
            .unwrap_or(false)
}

#[derive(Debug, Clone)]
struct CharacterRef {
    id: String,
    name: String,
}

#[derive(Debug, Clone)]
struct LocationRef {
    id: String,
    name: String,
}

fn find_character_ref(context: &AiBookWorkingContextV3, value: &str) -> Option<CharacterRef> {
    let candidate = canonical_key(value);
    context.relevant_characters.iter().find_map(|character| {
        let mut names = vec![character.name.clone()];
        names.extend(character.aliases.clone());
        names.into_iter().any(|name| canonical_key(&name) == candidate).then(|| CharacterRef {
            id: character.id.clone(),
            name: character.name.clone(),
        })
    })
}

fn find_location_ref(context: &AiBookWorkingContextV3, value: &str) -> Option<LocationRef> {
    let candidate = canonical_key(value);
    context.relevant_locations.iter().find_map(|location| {
        let mut names = vec![location.name.clone()];
        names.extend(location.aliases.clone());
        names.into_iter().any(|name| canonical_key(&name) == candidate).then(|| LocationRef {
            id: location.id.clone(),
            name: location.name.clone(),
        })
    })
}

fn collect_mentions(
    memory: &AiBookMemoryV3,
    chapter_digest: Option<&AiBookChapterDigestV3>,
    chapter_text: &str,
) -> MentionSetV3 {
    let mut characters = BTreeSet::new();
    let mut locations = BTreeSet::new();
    let mut fact_titles = BTreeSet::new();

    if let Some(digest) = chapter_digest {
        for character in &digest.characters {
            characters.insert(stable_character_id(&character.name, &character.aliases));
        }
        for location in &digest.locations {
            locations.insert(stable_location_id(&location.name));
        }
        for fact in &digest.knowledge_facts {
            fact_titles.insert(fact.title.clone());
        }
    }

    for character in &memory.characters {
        let mut candidates = vec![character.name.clone()];
        candidates.extend(character.aliases.clone());
        if candidates.iter().any(|name| !name.is_empty() && chapter_text.contains(name)) {
            characters.insert(stable_character_id(&character.name, &character.aliases));
        }
    }
    for location in &memory.locations {
        if chapter_text.contains(&location.name) {
            locations.insert(stable_location_id(&location.name));
        }
    }
    for fact in &memory.knowledge_facts {
        if chapter_text.contains(&fact.title) {
            fact_titles.insert(fact.title.clone());
        }
    }

    if characters.is_empty() {
        for character in memory.characters.iter().take(MAX_RELEVANT_CHARACTERS) {
            characters.insert(stable_character_id(&character.name, &character.aliases));
        }
    }
    if locations.is_empty() {
        for location in memory.locations.iter().take(MAX_RELEVANT_LOCATIONS) {
            locations.insert(stable_location_id(&location.name));
        }
    }

    MentionSetV3 {
        characters,
        locations,
        fact_titles,
    }
}

#[derive(Default)]
struct MentionSetV3 {
    characters: BTreeSet<String>,
    locations: BTreeSet<String>,
    fact_titles: BTreeSet<String>,
}

fn state_map(states: &[AiBookCharacterStateV3]) -> HashMap<String, AiBookCharacterStateV3> {
    states
        .iter()
        .map(|state| (stable_character_id(&state.name, &[]), state.clone()))
        .collect()
}

fn parent_location_name(location: &AiBookLocationV3, location_edges: &[AiBookLocationEdgeV3]) -> Option<String> {
    location_edges.iter().find_map(|edge| {
        (edge.kind == AiBookLocationEdgeKind::Contains && edge.target == location.name)
            .then(|| edge.source.clone())
    })
}

fn dedupe_context_facts(facts: Vec<WorkingContextFactV3>) -> Vec<WorkingContextFactV3> {
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for fact in facts {
        if seen.insert(fact.id.clone()) {
            items.push(fact);
        }
    }
    items.truncate(MAX_RELEVANT_FACTS);
    items
}

fn dedupe_by_id<T, F, M>(items: Vec<T>, mut key_fn: F, mut merge_fn: M) -> Vec<T>
where
    F: FnMut(&T) -> String,
    M: FnMut(&mut T, T),
{
    let mut ordered = Vec::new();
    let mut index = HashMap::new();
    for item in items {
        let key = key_fn(&item);
        if let Some(existing_index) = index.get(&key).copied() {
            merge_fn(&mut ordered[existing_index], item);
        } else {
            index.insert(key, ordered.len());
            ordered.push(item);
        }
    }
    ordered
}

fn merge_character_patch(existing: &mut NormalizedCharacterV3, incoming: NormalizedCharacterV3) {
    existing.aliases = merge_name_candidates(
        std::slice::from_ref(&existing.canonical_name),
        &merge_name_candidates(std::slice::from_ref(&incoming.canonical_name), &incoming.aliases),
    );
    existing.status = incoming.status.or_else(|| existing.status.clone());
    existing.faction = incoming.faction.or_else(|| existing.faction.clone());
    existing.location = incoming.location.or_else(|| existing.location.clone());
    existing.description = incoming.description.or_else(|| existing.description.clone());
    existing.evidence = dedupe_evidence([
        existing.evidence.clone(),
        incoming.evidence,
    ]
    .concat());
}

fn merge_character_state_patch(existing: &mut NormalizedCharacterStateV3, incoming: NormalizedCharacterStateV3) {
    existing.current_status = incoming.current_status.or_else(|| existing.current_status.clone());
    existing.affiliations = dedupe_strings([
        existing.affiliations.clone(),
        incoming.affiliations,
    ]
    .concat());
    existing.abilities = dedupe_strings([existing.abilities.clone(), incoming.abilities].concat());
    existing.resources = dedupe_strings([existing.resources.clone(), incoming.resources].concat());
    existing.current_location_id = incoming.current_location_id.or_else(|| existing.current_location_id.clone());
    existing.evidence = dedupe_evidence([existing.evidence.clone(), incoming.evidence].concat());
}

fn merge_relation_patch(existing: &mut NormalizedCharacterRelationV3, incoming: NormalizedCharacterRelationV3) {
    existing.polarity = incoming.polarity;
    existing.strength = incoming.strength;
    existing.status = incoming.status;
    existing.summary = incoming.summary.clone();
    existing.current_dynamics = dedupe_strings([
        existing.current_dynamics.clone(),
        incoming.current_dynamics,
    ]
    .concat());
    existing.evidence = dedupe_evidence([existing.evidence.clone(), incoming.evidence].concat());
    existing.history.extend(incoming.history);
}

fn merge_fact_patch(existing: &mut NormalizedKnowledgeFactV3, incoming: NormalizedKnowledgeFactV3) {
    existing.content = incoming.content;
    existing.confidence = incoming.confidence;
    existing.importance = incoming.importance;
    existing.evidence = dedupe_evidence([existing.evidence.clone(), incoming.evidence].concat());
}

fn merge_location_patch(existing: &mut NormalizedLocationV3, incoming: NormalizedLocationV3) {
    existing.kind = incoming.kind.or_else(|| existing.kind.clone());
    existing.description = incoming.description.or_else(|| existing.description.clone());
    existing.status = incoming.status.or_else(|| existing.status.clone());
    existing.evidence = dedupe_evidence([existing.evidence.clone(), incoming.evidence].concat());
}

fn merge_location_edge_patch(existing: &mut NormalizedLocationEdgeV3, incoming: NormalizedLocationEdgeV3) {
    existing.label = incoming.label.or_else(|| existing.label.clone());
    existing.evidence = dedupe_evidence([existing.evidence.clone(), incoming.evidence].concat());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ai_book::{AiBookSummaryV3, AiBookCharacterRelationV3};
    use crate::model::ai_book_generation::{
        AiBookCharacterPatchV3, AiBookCharacterRelationPatchV3, AiBookKnowledgeFactPatchV3,
        AiBookKnowledgePatchV3, AiBookLocationPatchV3,
    };

    #[test]
    fn ai_book_v3_merges_character_by_alias() {
        let mut memory = create_empty_ai_book_memory_v3("book://test", None, None);
        memory.characters.push(AiBookCharacterV3 {
            name: "张羽".to_string(),
            aliases: vec!["小羽".to_string()],
            ..AiBookCharacterV3::default()
        });
        let digest = AiBookChapterDigestV3 {
            chapter_index: 12,
            chapter_title: "第十二章".to_string(),
            summary: "羽哥再次现身".to_string(),
            ..AiBookChapterDigestV3::default()
        };
        let context = select_working_context_v3(&memory, Some(&digest), "羽哥再次现身");
        let patch = AiBookKnowledgePatchV3 {
            chapter_index: 12,
            characters: vec![AiBookCharacterPatchV3 {
                name: "羽哥".to_string(),
                aliases: vec!["张羽".to_string()],
                description: Some("主角".to_string()),
                ..AiBookCharacterPatchV3::default()
            }],
            ..AiBookKnowledgePatchV3::default()
        };

        let normalized = normalize_knowledge_patch_v3(patch, &context);
        let merged = merge_ai_book_memory_v3(memory, normalized);

        assert_eq!(merged.characters.len(), 1);
        assert_eq!(merged.characters[0].name, "张羽");
        assert!(merged.characters[0].aliases.iter().any(|item| item == "羽哥"));
        assert!(merged.characters[0].aliases.iter().any(|item| item == "张羽"));
    }

    #[test]
    fn ai_book_v3_drops_person_location_relation() {
        let context = working_context_with_entities(
            vec![("张羽", vec![])],
            vec!["嵩阳高中"],
        );
        let result = classify_relation_candidate_v3(
            RelationCandidateV3 {
                source_name: "张羽".to_string(),
                target_name: "嵩阳高中".to_string(),
                kind_raw: "located_in".to_string(),
                description: Some("张羽在嵩阳高中".to_string()),
                evidence: vec![test_evidence()],
                ..RelationCandidateV3::default()
            },
            &context,
        );

        match result {
            RelationClassificationV3::Drop(dropped) => {
                assert_eq!(dropped.reason, "person_location_relation");
            }
            other => panic!("expected drop, got {other:?}"),
        }
    }

    #[test]
    fn ai_book_v3_redirects_person_ability_to_state() {
        let context = working_context_with_entities(vec![("张羽", vec![])], Vec::<&str>::new());
        let result = classify_relation_candidate_v3(
            RelationCandidateV3 {
                source_name: "张羽".to_string(),
                target_name: "健体三十六式".to_string(),
                kind_raw: "cultivate".to_string(),
                description: Some("张羽修炼健体三十六式".to_string()),
                evidence: vec![test_evidence()],
                ..RelationCandidateV3::default()
            },
            &context,
        );

        match result {
            RelationClassificationV3::Redirect(RelationRedirectV3::CharacterState(state)) => {
                assert!(state.abilities.iter().any(|item| item == "健体三十六式"));
                assert_eq!(state.canonical_name, "张羽");
            }
            other => panic!("expected redirect to state, got {other:?}"),
        }
    }

    #[test]
    fn ai_book_v3_redirects_location_contains_to_location_edge() {
        let context = working_context_with_entities(
            Vec::<(&str, Vec<&str>)>::new(),
            vec!["嵩阳高中", "法力教室"],
        );
        let result = classify_relation_candidate_v3(
            RelationCandidateV3 {
                source_name: "嵩阳高中".to_string(),
                target_name: "法力教室".to_string(),
                kind_raw: "contains".to_string(),
                description: Some("嵩阳高中包含法力教室".to_string()),
                evidence: vec![test_evidence()],
                ..RelationCandidateV3::default()
            },
            &context,
        );

        match result {
            RelationClassificationV3::Redirect(RelationRedirectV3::LocationEdge(edge)) => {
                assert_eq!(edge.kind, AiBookLocationEdgeKind::Contains);
                assert_eq!(edge.source_name, "嵩阳高中");
                assert_eq!(edge.target_name, "法力教室");
            }
            other => panic!("expected redirect to location edge, got {other:?}"),
        }
    }

    #[test]
    fn ai_book_v3_redirects_location_adjacent_to_location_edge() {
        let context = working_context_with_entities(
            Vec::<(&str, Vec<&str>)>::new(),
            vec!["嵩阳高中", "训练馆"],
        );
        let result = classify_relation_candidate_v3(
            RelationCandidateV3 {
                source_name: "嵩阳高中".to_string(),
                target_name: "训练馆".to_string(),
                kind_raw: "adjacent".to_string(),
                description: Some("嵩阳高中与训练馆相邻".to_string()),
                evidence: vec![test_evidence()],
                ..RelationCandidateV3::default()
            },
            &context,
        );

        match result {
            RelationClassificationV3::Redirect(RelationRedirectV3::LocationEdge(edge)) => {
                assert_eq!(edge.kind, AiBookLocationEdgeKind::Adjacent);
                assert_eq!(edge.id, "location-edge:location:嵩阳高中:location:训练馆:Adjacent");
            }
            other => panic!("expected adjacent redirect, got {other:?}"),
        }
    }

    #[test]
    fn ai_book_v3_generates_stable_ids_without_trusting_model_ids() {
        let mut memory = create_empty_ai_book_memory_v3("book://test", None, None);
        memory.characters.push(AiBookCharacterV3 {
            name: "张羽".to_string(),
            ..AiBookCharacterV3::default()
        });
        let digest = AiBookChapterDigestV3 {
            chapter_index: 3,
            chapter_title: "第三章".to_string(),
            summary: "temp_001 实际是张羽".to_string(),
            ..AiBookChapterDigestV3::default()
        };
        let context = select_working_context_v3(&memory, Some(&digest), "temp_001 实际是张羽");
        let patch = AiBookKnowledgePatchV3 {
            chapter_index: 3,
            characters: vec![AiBookCharacterPatchV3 {
                name: "temp_001".to_string(),
                aliases: vec!["张羽".to_string()],
                ..AiBookCharacterPatchV3::default()
            }],
            ..AiBookKnowledgePatchV3::default()
        };

        let normalized = normalize_knowledge_patch_v3(patch, &context);
        let merged = merge_ai_book_memory_v3(memory, normalized);
        let display = select_ai_book_display_memory_v3(&merged);

        assert_eq!(display.characters.len(), 1);
        assert_eq!(display.characters[0].id, "character:张羽");
    }

    #[test]
    fn ai_book_v3_preserves_resolved_character_id_on_alias_merge() {
        let context = AiBookWorkingContextV3 {
            summary_current: "上下文摘要".to_string(),
            relevant_characters: vec![WorkingContextCharacterV3 {
                id: "character:stable-legacy".to_string(),
                name: "张羽".to_string(),
                aliases: vec!["羽哥".to_string()],
                status: None,
                affiliations: Vec::new(),
                abilities: Vec::new(),
            }],
            current_chapter_index: Some(2),
            current_chapter_title: Some("第二章".to_string()),
            schema_hint: "KnowledgePatchV3".to_string(),
            ..AiBookWorkingContextV3::default()
        };
        let patch = AiBookKnowledgePatchV3 {
            chapter_index: 2,
            characters: vec![AiBookCharacterPatchV3 {
                name: "羽哥".to_string(),
                aliases: vec!["张羽".to_string(), "小羽".to_string()],
                ..AiBookCharacterPatchV3::default()
            }],
            ..AiBookKnowledgePatchV3::default()
        };

        let normalized = normalize_knowledge_patch_v3(patch, &context);

        assert_eq!(normalized.characters.len(), 1);
        assert_eq!(normalized.characters[0].id, "character:stable-legacy");
    }

    #[test]
    fn ai_book_v3_alias_growth_does_not_shorten_character_id() {
        let mut memory = create_empty_ai_book_memory_v3("book://test", None, None);
        memory.characters.push(AiBookCharacterV3 {
            name: "Alexandra".to_string(),
            ..AiBookCharacterV3::default()
        });
        let digest = AiBookChapterDigestV3 {
            chapter_index: 6,
            chapter_title: "第六章".to_string(),
            summary: "Alex 出场".to_string(),
            ..AiBookChapterDigestV3::default()
        };
        let context = select_working_context_v3(&memory, Some(&digest), "Alex 出场");
        let patch = AiBookKnowledgePatchV3 {
            chapter_index: 6,
            characters: vec![AiBookCharacterPatchV3 {
                name: "Alex".to_string(),
                aliases: vec!["Alexandra".to_string()],
                ..AiBookCharacterPatchV3::default()
            }],
            ..AiBookKnowledgePatchV3::default()
        };

        let normalized = normalize_knowledge_patch_v3(patch, &context);
        assert_eq!(normalized.characters[0].id, "character:alexandra");

        let merged = merge_ai_book_memory_v3(memory, normalized);
        let display = select_ai_book_display_memory_v3(&merged);
        let next_context = select_working_context_v3(&merged, Some(&digest), "Alex 出场");

        assert_eq!(display.characters[0].id, "character:alexandra");
        assert_eq!(next_context.relevant_characters[0].id, "character:alexandra");
    }

    #[test]
    fn ai_book_v3_working_context_relation_names_do_not_come_from_label() {
        let mut memory = memory_with_characters(&["张羽", "白真真"]);
        let digest = AiBookChapterDigestV3 {
            chapter_index: 8,
            chapter_title: "第八章".to_string(),
            summary: "张羽与白真真继续合作".to_string(),
            ..AiBookChapterDigestV3::default()
        };
        let context = select_working_context_v3(&memory, Some(&digest), "张羽与白真真继续合作");
        let patch = AiBookKnowledgePatchV3 {
            chapter_index: 8,
            character_relations: vec![AiBookCharacterRelationPatchV3 {
                source: "张羽".to_string(),
                target: "白真真".to_string(),
                kind: "friend".to_string(),
                polarity: "positive".to_string(),
                strength: "major".to_string(),
                status: "active".to_string(),
                description: Some("两人继续合作".to_string()),
            }],
            ..AiBookKnowledgePatchV3::default()
        };

        let normalized = normalize_knowledge_patch_v3(patch, &context);
        memory = merge_ai_book_memory_v3(memory, normalized);
        let next_context = select_working_context_v3(&memory, Some(&digest), "张羽与白真真继续合作");

        assert_eq!(next_context.relevant_relations.len(), 1);
        assert_eq!(next_context.relevant_relations[0].label, "friend");
        assert_eq!(next_context.relevant_relations[0].source_name, "张羽");
        assert_eq!(next_context.relevant_relations[0].target_name, "白真真");
    }

    #[test]
    fn ai_book_v3_preserves_directed_relation_storage() {
        let mut memory = memory_with_characters(&["苏海峰", "张羽"]);
        let digest = AiBookChapterDigestV3 {
            chapter_index: 10,
            chapter_title: "第十章".to_string(),
            summary: "师生博弈".to_string(),
            ..AiBookChapterDigestV3::default()
        };
        let context = select_working_context_v3(&memory, Some(&digest), "苏海峰诱导张羽，张羽反制苏海峰");

        let forward = normalize_knowledge_patch_v3(
            AiBookKnowledgePatchV3 {
                chapter_index: 10,
                character_relations: vec![AiBookCharacterRelationPatchV3 {
                    source: "苏海峰".to_string(),
                    target: "张羽".to_string(),
                    kind: "superior_subordinate".to_string(),
                    polarity: "negative".to_string(),
                    strength: "major".to_string(),
                    status: "active".to_string(),
                    description: Some("苏海峰诱导张羽签债务合同".to_string()),
                }],
                ..AiBookKnowledgePatchV3::default()
            },
            &context,
        );
        memory = merge_ai_book_memory_v3(memory, forward);

        let reverse = normalize_knowledge_patch_v3(
            AiBookKnowledgePatchV3 {
                chapter_index: 10,
                character_relations: vec![AiBookCharacterRelationPatchV3 {
                    source: "张羽".to_string(),
                    target: "苏海峰".to_string(),
                    kind: "superior_subordinate".to_string(),
                    polarity: "negative".to_string(),
                    strength: "major".to_string(),
                    status: "active".to_string(),
                    description: Some("张羽反制苏海峰".to_string()),
                }],
                ..AiBookKnowledgePatchV3::default()
            },
            &context,
        );
        memory = merge_ai_book_memory_v3(memory, reverse);

        assert_eq!(memory.character_relations.len(), 2);
        assert_ne!(memory.character_relations[0].source, memory.character_relations[1].source);
    }

    #[test]
    fn ai_book_v3_status_only_state_patch_preserves_existing_description_and_new_empty_state_is_none() {
        let mut existing_state = AiBookCharacterStateV3 {
            name: "张羽".to_string(),
            status: "旧状态".to_string(),
            description: Some("abilities=健体三十六式;resources=灵石".to_string()),
            ..AiBookCharacterStateV3::default()
        };
        let status_only_patch = NormalizedCharacterStateV3 {
            character_id: "character:张羽".to_string(),
            canonical_name: "张羽".to_string(),
            current_status: Some("新状态".to_string()),
            ..NormalizedCharacterStateV3::default()
        };

        merge_state_into_memory(&mut existing_state, &status_only_patch);

        assert_eq!(existing_state.status, "新状态");
        assert_eq!(
            existing_state.description.as_deref(),
            Some("abilities=健体三十六式;resources=灵石")
        );

        let mut memory = memory_with_characters(&["白真真"]);
        let digest = AiBookChapterDigestV3 {
            chapter_index: 11,
            chapter_title: "第十一章".to_string(),
            summary: "白真真状态更新".to_string(),
            ..AiBookChapterDigestV3::default()
        };
        let context = select_working_context_v3(&memory, Some(&digest), "白真真状态更新");
        let patch = NormalizedKnowledgePatchV3 {
            chapter_index: 11,
            character_states: vec![NormalizedCharacterStateV3 {
                character_id: "character:白真真".to_string(),
                canonical_name: "白真真".to_string(),
                current_status: Some("警觉".to_string()),
                evidence: vec![test_evidence()],
                ..NormalizedCharacterStateV3::default()
            }],
            ..NormalizedKnowledgePatchV3::default()
        };

        memory = merge_ai_book_memory_v3(memory, patch);
        let context_after = select_working_context_v3(&memory, Some(&digest), "白真真状态更新");

        assert_eq!(memory.character_states.len(), 1);
        assert_eq!(memory.character_states[0].status, "警觉");
        assert_eq!(memory.character_states[0].description, None);
        assert_eq!(context_after.relevant_characters[0].abilities, Vec::<String>::new());
        drop(context);
    }

    #[test]
    fn ai_book_v3_view_can_group_same_character_pair() {
        let memory = AiBookMemoryV3 {
            book_url: "book://test".to_string(),
            characters: vec![
                AiBookCharacterV3 { name: "白真真".to_string(), ..AiBookCharacterV3::default() },
                AiBookCharacterV3 { name: "张羽".to_string(), ..AiBookCharacterV3::default() },
            ],
            character_relations: vec![
                stored_relation("白真真", "张羽", AiBookRelationKind::Friendship, AiBookRelationStatus::Active, "借贷互助"),
                stored_relation("张羽", "白真真", AiBookRelationKind::Conflict, AiBookRelationStatus::Active, "彼此试探"),
            ],
            ..AiBookMemoryV3::default()
        };

        let view = select_ai_book_display_memory_v3(&memory);

        assert_eq!(view.relationships.len(), 1);
        assert_eq!(view.relationships[0].facets.len(), 2);
    }

    #[test]
    fn ai_book_v3_ended_relation_hidden_from_default_view() {
        let memory = AiBookMemoryV3 {
            book_url: "book://test".to_string(),
            characters: vec![
                AiBookCharacterV3 { name: "白真真".to_string(), ..AiBookCharacterV3::default() },
                AiBookCharacterV3 { name: "张羽".to_string(), ..AiBookCharacterV3::default() },
            ],
            character_relations: vec![stored_relation(
                "白真真",
                "张羽",
                AiBookRelationKind::Friendship,
                AiBookRelationStatus::Broken,
                "关系结束",
            )],
            ..AiBookMemoryV3::default()
        };

        let view = select_ai_book_display_memory_v3(&memory);
        assert!(view.relationships.is_empty());
    }

    #[test]
    fn ai_book_v3_working_context_has_hard_caps() {
        let mut memory = create_empty_ai_book_memory_v3("book://big", Some("大书".to_string()), None);
        memory.summary = AiBookSummaryV3 { current: "总览".to_string(), ..AiBookSummaryV3::default() };
        memory.chapter_digests = (0..10)
            .map(|index| AiBookChapterDigestV3 {
                chapter_index: index,
                chapter_title: format!("第{index}章"),
                summary: format!("摘要{index}"),
                key_points: vec![format!("事件{index}")],
                ..AiBookChapterDigestV3::default()
            })
            .collect();
        memory.characters = (0..30)
            .map(|index| AiBookCharacterV3 {
                name: format!("角色{index}"),
                ..AiBookCharacterV3::default()
            })
            .collect();
        memory.character_relations = (0..20)
            .map(|index| stored_relation(&format!("角色{}", index), &format!("角色{}", index + 1), AiBookRelationKind::Friendship, AiBookRelationStatus::Active, "关系"))
            .collect();
        memory.knowledge_facts = (0..20)
            .map(|index| AiBookKnowledgeFactV3 {
                title: format!("设定{index}"),
                content: "内容".to_string(),
                category: AiBookFactCategory::BasicRule,
                confidence: AiBookFactConfidence::High,
                importance: AiBookFactImportance::High,
            })
            .collect();
        memory.locations = (0..20)
            .map(|index| AiBookLocationV3 {
                name: format!("地点{index}"),
                kind: Some("school".to_string()),
                description: "地点".to_string(),
                ..AiBookLocationV3::default()
            })
            .collect();
        let digest = AiBookChapterDigestV3 {
            chapter_index: 10,
            chapter_title: "第十章".to_string(),
            summary: "多实体章节".to_string(),
            characters: memory.characters.clone(),
            locations: memory.locations.clone(),
            knowledge_facts: memory.knowledge_facts.clone(),
            ..AiBookChapterDigestV3::default()
        };

        let chapter_text = (0..30)
            .map(|index| format!("角色{index} 在 地点{} 讨论 设定{}", index.min(19), index.min(19)))
            .collect::<Vec<_>>()
            .join("\n");
        let context = select_working_context_v3(&memory, Some(&digest), &chapter_text);

        assert_eq!(context.recent_chapter_digests.len(), 8);
        assert!(context.relevant_characters.len() <= 20);
        assert!(context.relevant_relations.len() <= 12);
        assert!(context.relevant_knowledge_facts.len() <= 12);
        assert!(context.relevant_locations.len() <= 15);
    }

    #[test]
    fn ai_book_v3_requires_evidence_for_semantic_entities() {
        let context = AiBookWorkingContextV3 {
            summary_current: "无标题上下文".to_string(),
            relevant_characters: vec![
                WorkingContextCharacterV3 {
                    id: "character:张羽".to_string(),
                    name: "张羽".to_string(),
                    aliases: Vec::new(),
                    status: None,
                    affiliations: Vec::new(),
                    abilities: Vec::new(),
                },
                WorkingContextCharacterV3 {
                    id: "character:白真真".to_string(),
                    name: "白真真".to_string(),
                    aliases: Vec::new(),
                    status: None,
                    affiliations: Vec::new(),
                    abilities: Vec::new(),
                },
            ],
            relevant_locations: vec![WorkingContextLocationV3 {
                id: "location:嵩阳高中".to_string(),
                name: "嵩阳高中".to_string(),
                aliases: Vec::new(),
                parent_name: None,
                kind: "school".to_string(),
                scale: "site".to_string(),
            }],
            current_chapter_index: Some(5),
            current_chapter_title: None,
            schema_hint: "KnowledgePatchV3".to_string(),
            ..AiBookWorkingContextV3::default()
        };
        let patch = AiBookKnowledgePatchV3 {
            chapter_index: 5,
            characters: vec![AiBookCharacterPatchV3 {
                name: "张羽".to_string(),
                ..AiBookCharacterPatchV3::default()
            }],
            character_relations: vec![AiBookCharacterRelationPatchV3 {
                source: "张羽".to_string(),
                target: "白真真".to_string(),
                kind: "friend".to_string(),
                polarity: "positive".to_string(),
                strength: "major".to_string(),
                status: "active".to_string(),
                description: Some("两人互相信任".to_string()),
            }],
            knowledge_facts: vec![AiBookKnowledgeFactPatchV3 {
                title: "债务规则".to_string(),
                content: "欠债要还".to_string(),
                category: "基础规则".to_string(),
                confidence: "已知".to_string(),
                importance: "major".to_string(),
            }],
            locations: vec![AiBookLocationPatchV3 {
                name: "嵩阳高中".to_string(),
                description: "重要场景".to_string(),
                ..AiBookLocationPatchV3::default()
            }],
            ..AiBookKnowledgePatchV3::default()
        };

        let normalized = normalize_knowledge_patch_v3(patch, &context);

        assert!(normalized.characters.is_empty());
        assert!(normalized.character_states.is_empty());
        assert!(normalized.character_relations.is_empty());
        assert!(normalized.knowledge_facts.is_empty());
        assert!(normalized.locations.is_empty());
        assert!(normalized.location_edges.is_empty());
        assert_eq!(normalized.dropped_facts.len(), 1);
        assert_eq!(normalized.dropped_facts[0].reason, "missing_evidence");
    }

    #[test]
    fn ai_book_v3_drops_low_value_cooccurrence_relation() {
        let context = working_context_with_entities(
            vec![("张羽", vec![]), ("白真真", vec![])],
            Vec::<&str>::new(),
        );
        let result = classify_relation_candidate_v3(
            RelationCandidateV3 {
                source_name: "张羽".to_string(),
                target_name: "白真真".to_string(),
                kind_raw: "friend".to_string(),
                description: Some("两人只是同校认识，同屏出现".to_string()),
                evidence: vec![test_evidence()],
                ..RelationCandidateV3::default()
            },
            &context,
        );

        match result {
            RelationClassificationV3::Drop(dropped) => {
                assert_eq!(dropped.reason, "low_value_relation");
            }
            other => panic!("expected low-value drop, got {other:?}"),
        }
    }

    fn test_evidence() -> AiBookEvidenceV3 {
        AiBookEvidenceV3 {
            chapter_index: 1,
            chapter_title: "第一章".to_string(),
            quote: None,
            note: "证据".to_string(),
        }
    }

    fn working_context_with_entities(
        characters: Vec<(&str, Vec<&str>)>,
        locations: Vec<&str>,
    ) -> AiBookWorkingContextV3 {
        AiBookWorkingContextV3 {
            summary_current: "上下文摘要".to_string(),
            relevant_characters: characters
                .into_iter()
                .map(|(name, aliases)| WorkingContextCharacterV3 {
                    id: stable_character_id(name, &aliases.iter().map(|item| item.to_string()).collect::<Vec<_>>()),
                    name: name.to_string(),
                    aliases: aliases.into_iter().map(|item| item.to_string()).collect(),
                    status: None,
                    affiliations: Vec::new(),
                    abilities: Vec::new(),
                })
                .collect(),
            relevant_locations: locations
                .into_iter()
                .map(|name| WorkingContextLocationV3 {
                    id: stable_location_id(name),
                    name: name.to_string(),
                    aliases: Vec::new(),
                    parent_name: None,
                    kind: "unknown".to_string(),
                    scale: "unknown".to_string(),
                })
                .collect(),
            current_chapter_index: Some(1),
            current_chapter_title: Some("第一章".to_string()),
            schema_hint: "KnowledgePatchV3".to_string(),
            ..AiBookWorkingContextV3::default()
        }
    }

    fn memory_with_characters(names: &[&str]) -> AiBookMemoryV3 {
        let mut memory = create_empty_ai_book_memory_v3("book://test", None, None);
        memory.characters = names
            .iter()
            .map(|name| AiBookCharacterV3 {
                name: (*name).to_string(),
                ..AiBookCharacterV3::default()
            })
            .collect();
        memory
    }

    fn stored_relation(
        source: &str,
        target: &str,
        kind: AiBookRelationKind,
        status: AiBookRelationStatus,
        summary: &str,
    ) -> AiBookCharacterRelationV3 {
        let normalized = NormalizedCharacterRelationV3 {
            id: stable_relation_id(
                &stable_character_id(source, &[]),
                &stable_character_id(target, &[]),
                &kind,
                None,
                &if is_directed_relation_kind(&kind) {
                    RelationDirectionV3::Directed
                } else {
                    RelationDirectionV3::Undirected
                },
            ),
            source_character_id: stable_character_id(source, &[]),
            source_name: source.to_string(),
            target_character_id: stable_character_id(target, &[]),
            target_name: target.to_string(),
            kind: kind.clone(),
            label: format!("{} · {}", source, target),
            polarity: AiBookRelationPolarity::Neutral,
            strength: AiBookRelationStrength::Moderate,
            status: status.clone(),
            direction: if is_directed_relation_kind(&kind) {
                RelationDirectionV3::Directed
            } else {
                RelationDirectionV3::Undirected
            },
            summary: summary.to_string(),
            current_dynamics: vec![summary.to_string()],
            evidence: vec![test_evidence()],
            history: vec![AiBookRelationChangeView {
                chapter_index: 1,
                chapter_title: "第一章".to_string(),
                previous_kind: None,
                next_kind: kind.clone(),
                previous_polarity: None,
                next_polarity: AiBookRelationPolarity::Neutral,
                previous_status: None,
                next_status: status.clone(),
                note: summary.to_string(),
                evidence: vec![test_evidence()],
            }],
            ..NormalizedCharacterRelationV3::default()
        };
        AiBookCharacterRelationV3 {
            source: source.to_string(),
            target: target.to_string(),
            kind,
            polarity: AiBookRelationPolarity::Neutral,
            strength: AiBookRelationStrength::Moderate,
            status,
            description: Some(encode_relation_meta(&normalized)),
        }
    }
}
