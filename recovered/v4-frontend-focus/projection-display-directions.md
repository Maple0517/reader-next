# Projection Display Directions

## Purpose

This note records the longer-term product/design thinking raised during V4 frontend planning. It is not the implementation plan for the current slice. It should guide future backend projection, API, schema, and frontend IA work after the first V4 memory console redesign lands.

The framing decision is: V4 frontend is a **novel memory browser**, not a raw V4 database browser. Backend/API fields should appear in the default UI only when they help a reader understand characters, relationships, world knowledge, places, processing state, or data quality.

## Display Classification

### Default Frontend Display

These are suitable as first-layer UI after projection:

| Domain | Display Concept | Current Support | Notes |
| --- | --- | --- | --- |
| Overview | read/processed boundary, processing state, counts, attention items | partial | `memory`, `memory/status`, `catchup/status`, quality summary support this, but attention items need frontend composition. |
| Task | current/target chapter, status, last error, cancel/retry actions | partial | Existing APIs support coarse status; stage-level trace is missing. |
| Character | canonical display name, aliases, summary, chapter range, visibility, relationship count | yes | Existing character list/card projection supports these. |
| Character | state matrix: realm, ability, equipment, location, goal, mental state, affiliation, life status | yes | Backed by `CharacterCardView.currentStates`; frontend should make this prominent. |
| Relationship | graph nodes, relationship edges, group, label, current state, polarity, strength, confidence, event count | yes | Suitable for cluster/network view plus edge inspector. |
| Knowledge | category/topic cards, current summary, confidence, importance, first/last chapter, assertion count | yes | Default should be `Knowledge Atlas`, not a global fact stream. |
| Knowledge | assertion timeline inside selected topic/card | yes | `KnowledgeCardDetailView.assertionsByStatus` supports scoped inspection. |
| Place | place list, place hierarchy, map graph/layout, top places, conflicts | partial | Existing projections support map/graph/layout, but place detail is still thin. |
| Quality | quarantine queue, audit findings, corrections, reprocess jobs, prompt regression runs | yes | Should be a governance workbench, not a normal entity directory. |

### Inspector-Only / Debug Disclosure

These should exist behind explicit disclosure controls, not in the default visual layer:

| Data | Why Not Default |
| --- | --- |
| `claimId`, `sourceClaimId`, `aiRunId`, entity ids | Useful for audit/debug, noisy for readers. |
| `claims.valueJson`, `ai_runs.output_json` | Raw backend payload; should not define product UI. |
| prompt version, schema version, model, input hash | Useful for diagnosis and regression work only. |
| full source span text | Valuable evidence, but too dense for normal scanning. |
| identity links and merge operations as standalone debug tables | Identity should surface as character/quality/task context unless debugging. |
| view model cache keys/scope ids | Implementation detail. |
| raw confidence decimals everywhere | Default should use readable confidence tiers; numeric values can be disclosed. |

### Not Needed In Frontend

These should generally not appear in the user-facing V4 console:

| Data | Reason |
| --- | --- |
| Reducer internals and command payload shapes | Backend ownership boundary, not a reader concept. |
| Judge raw output fields | Decision internals; frontend should see result state and reason. |
| Projection cache implementation details | Operational detail. |
| Full ledger tables as pages | Quality and inspector flows should expose only relevant slices. |
| Low-level migration/schema names | Development detail. |

## Missing But Reader-Valuable

### Missing API / Projection, Existing Underlying Data

These can likely be added later without major schema changes:

| Product Concept | Why Valuable | Likely Source |
| --- | --- | --- |
| Task stage timeline | Shows where catchup is stuck: parse, decision, reducer, projection. | `chapter_processing_runs`, `ai_runs`, `claims`, `quarantined_claims`, projection cache metadata. |
| Per-chapter output summary | Shows parsed claims, decisions, writes, quarantines, projection refreshes. | Claim lifecycle plus canonical write results; needs aggregation API. |
| Character state grouping/order | Turns `currentStates` into readable groups like cultivation, combat, social, location, intent. | `property_dimensions`, `entity_current_properties`. |
| Relationship clusters/community grouping | Makes graph usable for large casts. | Relationship graph plus frontend or backend clustering. |
| Knowledge topic health | Highlights conflicted, recently revised, highly important, or stale topics. | `knowledge_cards`, `knowledge_assertions`, assertion links/status. |
| Place detail richness | Shows routes, containment, linked organizations, conflict history, recent scenes. | `place_edges`, `place_edge_sources`, `entity_links`, conflicts. |
| Overview attention feed | Combines running tasks, failures, quarantines, recent canonical changes. | Existing status and quality APIs plus recent run/claim data. |

### Likely Missing Schema / Canonical Concepts

These are product-expansion ideas, not current implementation requirements:

| Product Concept | Why Valuable | Schema Gap |
| --- | --- | --- |
| Character arc | Shows meaningful evolution over time: realm changes, alliance shifts, death/reveal events. | Current property history exists, but no product-level arc/event model. |
| Plot/event nodes | Helps readers recall what happened, not just what facts exist. | No first-class event model. |
| Foreshadowing/secret lifecycle | Tracks planted, developed, revealed, contradicted clues. | `knowledge` has `secret`/`prophecy`, but no lifecycle state. |
| Reader focus/pinning | Lets users track favorite characters, relationships, places, or topics. | No user preference layer for V4 memory. |
| Chapter delta summary | Shows what changed in V4 memory after processing a chapter. | Needs per-chapter projection delta or write summary. |
| Spoiler-aware memory | Separates current reading-safe view from future processed/known facts. | Needs strict read-bound projection and possibly spoiler policy. |

## Current Design Implications

1. Default UI should privilege canonical projected concepts, not raw records.
2. Character detail must make `currentStates` a first-class visual region.
3. Relationship can use a graph/cluster mental model, with list and inspector as support.
4. Knowledge should start from topic/category cards; assertion streams belong inside a selected card.
5. Place/map can remain visual and spatial, but detail APIs may need future enrichment.
6. Quality should be a governance workbench and the main place for quarantines, audit findings, corrections, reprocess jobs, and prompt regression.
7. Identity should not be a first-class left-rail item unless future product evidence shows users need it as a standalone domain.
8. Future backend work should add frontend-ready projections rather than forcing the frontend to reinterpret ledger/canonical internals.
