# V4 Frontend Focus Design

## Goal

Redesign the AI资料 V4 frontend into a **memory database console** for novel AI material: a left-rail shell with focused pages for Overview, Task Progress, Characters, Relationships, Knowledge, Places, and Quality. The design follows the current Visual Companion at `.trellis/workspace/visual-companion/index.html` and treats the mock at `http://localhost:8766/?v=state-knowledge-atlas#rail-pages` as the visual target.

## Non-Goals

- Do not touch V3.
- Do not reopen backend boundary refactor.
- Do not make `Identity / 身份` a first-class rail item.
- Do not expose raw ledger/debug fields as default UI.
- Do not add a large new workflow engine or frontend state management system.
- Do not require new DB schema for the first implementation pass.

## Design Decisions

### Information Architecture

The V4 page becomes a full-page console with a persistent left rail:

1. `总览 / Overview`
2. `任务 / Task`
3. `角色 / Characters`
4. `关系 / Relationships`
5. `知识 / Knowledge`
6. `地点 / Places`
7. `质量 / Quality`

`Identity / 身份` is folded into:

- Character detail identity clues.
- Task/stage inspector when identity claims are processed or quarantined.
- Quality governance queues for identity conflicts, merge candidates, and correction actions.

### Page Shell

`AiBookV4View.vue` should become a composition surface:

- It owns route parsing, book loading, V4 status loading, top-level actions, active rail state, and shared shell layout.
- It should not own full domain implementations.
- It should render a left rail and a single active panel.
- Existing action buttons remain available, but task progress gets a first-class rail page and an overview live card.

The current top tab bar is replaced by a left rail. Rail labels should expose compact domain status, such as `running · 42%`, `48 canonical`, `126 edges`, `4 queues`, when data is available.

### Visual System

This is a dense product console, not a landing page.

- Use restrained, work-focused layout with clear scanning density.
- Use cards only for repeated items, inspector panes, and framed tools; avoid nested cards.
- Use CSS transitions and simple state-driven animation; do not add a motion dependency.
- Use existing Vue/CSS stack. No new UI library.
- Keep text readable at mobile and desktop widths.
- Keep raw IDs and debug payloads hidden behind explicit disclosure controls.

### Data Display Policy

Default frontend display should show projected reader-value concepts:

- Canonical display names, aliases, summaries, chapter ranges.
- Character state matrix from `currentStates`.
- Relationship graph/list/inspector from relationship projection.
- Knowledge topic/category cards before assertions.
- Place hierarchy/graph/detail from map/place projection.
- Quality queues and review inspector from quality APIs.

Inspector-only/debug disclosure can show:

- Claim/source/ai run ids.
- Source spans and raw excerpts.
- Confidence numbers.
- Raw claim value payloads.
- Prompt/schema/model metadata.

Do not surface backend fields merely because they exist.

## Domain Page Designs

### Overview

Purpose: answer "Is this book memory healthy, current, and actionable?"

Inputs:

- `getV4Memory`
- `getV4MemoryStatus`
- `getV4CatchupStatus`
- `getV4Quality`
- existing domain APIs as needed for counts already available in overview data

Layout:

- Live progress card with current/target chapter, processed boundary, and last error.
- Domain health cards: Characters, Relationships, Knowledge, Places.
- Attention list composed from task state, last error, quarantine count, quality findings, and recent projection/state availability.

First-pass behavior:

- Use existing status and overview APIs.
- Do not invent stage-level data when backend does not expose it.

### Task Progress

Purpose: answer "After I clicked catchup, what is happening and where did it fail?"

Inputs:

- `getV4CatchupStatus`
- `getV4MemoryStatus`
- `getV4QualityQuarantine`
- optionally `getV4QualityReprocessJobs` for reprocess state

Layout:

- Hero progress summary.
- Pipeline timeline:
  - First pass: coarse stages derived from status: queued/idle, running, failed, completed/cancelled.
  - Later: real stage trace can populate Parse / Decision / Reducer / Projection rows.
- Stage inspector:
  - First pass: status, current chapter, target chapter, max processed chapter, last error.
  - Later: claim counts, write/quarantine/no-write counts, projection refresh list.
- Compressed chapter navigator:
  - First pass: processed/current/target boundary visualization, not per-chapter run truth.
  - Later: per-chapter run state when a read API exists.

Backend gap explicitly deferred:

- No first-pass requirement for a stage trace API.
- No first-pass requirement for persisted projection refresh details.

### Characters

Purpose: answer "Who is this person now?"

Inputs:

- `getV4Characters`
- `getV4CharacterCard`
- `getV4CharacterRelationships`
- `getV4CharacterIdentity` only as inspector/governance context, not as a standalone page

Layout:

- Entity directory list with search/filter and visibility-first ordering.
- Character inspector with:
  - canonical display name
  - aliases
  - summary
  - chapter range
  - relationship count
  - prominent state matrix from `currentStates`
  - identity clues panel
  - evidence/debug disclosure

State matrix groups should be frontend-derived from dimension keys:

- Identity/social: identity, affiliation, occupation, rank.
- Power/combat: realm, ability, equipment.
- Story-now: location, mental_state, goal, life_status.
- Unknown dimensions fall into "其他状态".

### Relationships

Purpose: answer "Who is connected to whom, how, and how strongly?"

Inputs:

- `getV4Relationships`
- `getV4CharacterRelationships`

Layout:

- Relationship cluster/network view using current nodes/edges.
- Edge list/filter by group, polarity, confidence/importance.
- Edge inspector with group, label, current state, strength, polarity, confidence tier, event count, chapter range, and evidence disclosure.

First-pass graph:

- Use deterministic frontend layout or simple grouped lanes.
- Do not add graph libraries until data volume or interaction requirements demand it.

### Knowledge

Purpose: answer "What does the book's world currently know?"

Inputs:

- `getV4Knowledge`
- `getV4KnowledgeCategory`
- `getV4KnowledgeCard`

Layout:

- Knowledge Atlas, not global fact stream.
- Category/topic cards by power system, faction structure, world rule, history, secret, prophecy, politics, geography, custom.
- Topic inspector with current summary, confidence tier, assertion count, first/last chapters, assertions grouped by status, referenced entities.

Do not default to a flat fact stream. Assertions appear only inside a selected topic/card.

### Places

Purpose: answer "Where are people and events in the story space?"

Inputs:

- `getV4Map`
- `getV4MapPlaces`
- `getV4MapPlaceDetail`
- `getV4MapGraph`
- `getV4MapLayout`
- `getV4MapConflicts`

Layout:

- Spatial map/graph using layout when available.
- Place list and hierarchy.
- Place inspector with place type, linked organizations, conflict status, and evidence/debug disclosure.

First-pass limitation:

- Current place detail is thin. Do not promise routes/recent scenes unless available from graph/conflict APIs or future projection enrichment.

### Quality

Purpose: answer "What needs human attention before data can be trusted?"

Inputs:

- `getV4Quality`
- `getV4QualityQuarantine`
- `getV4QualityAuditFindings`
- `getV4QualityCorrections`
- `getV4QualityReprocessJobs`
- `getV4QualityPromptRegressionRuns`
- action APIs already defined in `frontend/src/api/v4/book.ts`

Layout:

- Queue navigation: quarantine, identity clues, run failures/reprocess, audit findings, corrections, prompt regression.
- Triage list sorted by priority/severity/open status.
- Review inspector with reason, suggested action, claim/source evidence, and safe action buttons.

Boundary rule:

- Quality actions call explicit backend APIs. Frontend and projection must not mutate canonical state directly.

## Component Architecture

Use Vue 3 Composition API with `<script setup lang="ts">`.

Recommended component map:

| File | Responsibility |
| --- | --- |
| `frontend/src/views/AiBookV4View.vue` | Route/book/status orchestration and shell composition only. |
| `frontend/src/components/reader/v4/V4ConsoleShell.vue` | Shared V4 console shell with header, rail, action area, and active content slot. |
| `frontend/src/components/reader/v4/V4DomainRail.vue` | Left rail rendering and active-domain selection. |
| `frontend/src/components/reader/v4/V4MetricCard.vue` | Compact metric/domain health card. |
| `frontend/src/components/reader/v4/V4InspectorSection.vue` | Reusable labeled inspector section/disclosure container. |
| `frontend/src/components/reader/v4/V4ConfidenceBadge.vue` | Confidence tier display from numeric confidence. |
| `frontend/src/components/reader/V4TaskProgressPanel.vue` | Task progress page. |
| existing `V4BookOverviewPanel.vue` | Redesign as console overview landing page. |
| existing `V4CharacterPanel.vue` | Redesign as directory + detail inspector with state matrix. |
| existing `V4RelationshipPanel.vue` | Redesign as graph/list + edge inspector. |
| existing `V4KnowledgePanel.vue` | Redesign as Knowledge Atlas + topic inspector. |
| existing `V4MapPanel.vue` | Redesign as map/list/hierarchy + place inspector. |
| existing `V4QualityPanel.vue` | Redesign as governance workbench. |

State/data rules:

- Keep fetched API data in panel-level state/composables.
- Derive filters, sorted lists, grouped states, badges, and rail summaries with `computed`.
- Use watchers only for side effects such as reload-on-book-url-change.
- Props down, events up; child components do not mutate parent state.
- Use `shallowRef` for primitive active keys/loading flags in new code where practical.
- Keep SFC sections in order: `<script setup lang="ts">`, `<template>`, `<style scoped>`.

## Testing Strategy

Focused tests should cover:

- V4 shell/rail renders target domain labels and no longer renders Identity as a first-class rail item.
- Overview shows progress/attention data from mocked APIs.
- Task panel derives coarse timeline states from `catchup/status` and `memory/status`.
- Character panel renders grouped `currentStates`.
- Relationship panel renders graph/list and selected edge inspector from mocked graph API.
- Knowledge panel renders category/topic cards by default and assertions only after selecting a card.
- Map panel handles map graph/layout and empty/thin place detail.
- Quality panel renders queue counts, triage list, and inspector actions from mocked quality APIs.

Validation commands:

- `cd frontend && npm test -- V4`
- `cd frontend && npm run build`
- `git diff --check`

## Risks

- Task progress mock is richer than current backend status APIs. First pass must clearly degrade to coarse state and avoid fake claim/stage counts.
- Relationship graph layout can become complex. First pass should use simple deterministic layout and avoid adding a graph dependency.
- Knowledge data volume can be high. Default view must remain topic/card based and avoid rendering all assertions.
- Existing V4 panels are already implemented; redesign should be incremental enough for focused tests.
- Identity removal from first-class rail must not remove identity debugging APIs; it only changes IA.
