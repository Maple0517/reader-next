# AI Book Memory V4 — Phase 5 Places and Map Implementation Plan

Phase 4 已 Product Ready Frozen。
本文件只定义 Phase 5 Places and Map implementation plan + task breakdown。
本阶段不写代码、不修改代码、不开始 Phase 5 coding。

## Phase 5 Final Result Update

Phase 5 implementation and freeze verification are complete.

Final report result:

- Phase 5 Backend: PASS
- Phase 5 Frontend: PASS
- Phase 5 Real AI Smoke: PASS
- Phase 1 / 2 / 3 / 4 regressions: PASS
- Phase 5 Product Readiness: PASS
- Phase 5 Freeze: YES
- Ready for Phase 6: YES
- Entering Phase 6 blocker: none within Phase 5 scope

Strict-gap follow-up completed after initial implementation:

- Direction inverse normalization now happens before duplicate detection.
- `inside` / `part_of` canonicalize to `contains` and update the same hierarchy parent path.
- Existing parent changes are protected unless there is an explicit accept decision.
- Real AI map judge accepts blank `conflict_type` as `None` for accept decisions while still rejecting invalid non-empty conflict types.
- Real AI smoke fixture was expanded to 3 chapters and covers inverse duplicate, hierarchy cycle rejection, conflict recording, organization/place link, and distractor rejection.

Final smoke summary:

```text
[MAP_SMOKE] places=5 active_edges=4 conflicts=1 layout_nodes=5 layout_edges=4 real_calls=6 scripted_conflicts=1 relationships=1 org_place_links=1 aliases=4 edge_sources=5 inverse_duplicate_edges=1 rejected_cycle_claims=1 false_positive_edges=0
```

Verification evidence:

- `cargo test --lib`: 602 passed
- `cargo test`: passed; one live YCKCeo test ignored
- `cargo test phase5_ --lib`: 4 passed
- `cargo test map --lib`: 36 passed
- `cargo test place --lib`: 55 passed
- `cargo test v4 --lib`: 419 passed
- `cargo test pipeline --lib`: 30 passed
- `cargo test projection --lib`: 29 passed
- Frontend focused map/API/AiBookView tests: 3 files / 10 tests passed
- `npm run build`: passed
- Real AI `real_ai_smoke_test_map_phase5_fixture`: passed
- Gortex review/MCP review: no findings

## 1. Phase 5 Goal

Phase 5 目标是结构化地点层级和拓扑地图。

系统应能从已读章节中提取、归一、修正并展示：

- 地点第一次出现。
- 地点别名。
- 地点类型。
- 父子层级，例如：大陆 -> 东域 -> 青云国 -> 青云城。
- 包含关系，例如：青云门 contains 藏经阁。
- 邻近关系，例如：青云城 near 落霞山。
- 路线关系，例如：青云城 route_to 黑风谷。
- 方位关系，例如：圣城 north_of 黑暗森林。
- 秘境 / 山脉 / 河流 / 宗门建筑等空间结构。
- map view model。
- layout snapshots。

Phase 5 只维护事实层和拓扑 view。
地图布局只是 projection，可以自动布局和美化，但不能反过来成为事实来源。
AI image map、复杂地图编辑器、GIS、pathfinding 都不是 Phase 5 范围。

## 2. Phase 5 Scope

必须覆盖：

1. `place_details` schema。
2. `place_edges` schema。
3. `map_layout_snapshots` schema。
4. Optional but recommended `place_edge_conflicts` schema。
5. Concrete `entity_links` support if not already present, for organization/place association。
6. `location_introduction` observation。
7. `location_edge` observation。
8. Place Resolver。
9. Map Structural Gate。
10. Real AI Map Conflict Judge。
11. `place_reducer`。
12. Parent hierarchy and cycle checks。
13. Directed / undirected edge handling。
14. Duplicate edge idempotency。
15. Conflict edge handling。
16. Place / organization same-name linking。
17. Map projection and layout snapshot rebuild。
18. Map APIs。
19. Lightweight frontend Map Panel。
20. Mock E2E tests。
21. Real AI smoke test。

Phase 5 使用 `entities(entity_type=place)` 作为 canonical place identity。
Phase 5 不新增独立 `place_nodes` / `place_aliases` 表；地点别名复用 `entity_aliases`。

## 3. Explicit Non-scope

不要实现：

- user correction UI
- manual map editor
- batch audit
- AI image map as fact source
- exact coordinates / GIS
- pathfinding
- complex graph layout algorithm
- cross-book map ontology
- knowledge system 新功能
- identity merge 新功能
- relationship judge 重写
- Legacy compatibility

## 4. Dependencies on Phase 1 / 2 / 3 / 4

Phase 5 继承 V4 shared foundation：

- Source -> Claim -> Canonical -> View。
- AI 不直接写 canonical tables。
- Reducer 是唯一 canonical writer。
- accepted claim 只能在 reducer transaction 成功后标记。
- 所有进入 canonical map 的 place facts 必须有 source span evidence。
- Additive migration only。

复用现有 Phase 1-4：

| Existing component | Current owner | Phase 5 usage |
|---|---|---|
| `entities` | Phase 1 | canonical place identity, `entity_type=place` |
| `entity_aliases` | Phase 1 | place aliases |
| `claims` / `claim_source_spans` | Phase 1 | `location_introduction`, `location_edge`, optional `map_conflict` ledger |
| `source_spans` | Phase 1 | evidence |
| `ai_runs` | Phase 1 | `map_conflict_judge` lifecycle |
| `view_model_cache` | Phase 1 | map overview/detail/graph/layout view cache |
| `relationships` | Phase 2 | context only; never place edge source |
| `entity_identity_links` | Phase 3 | redirect resolution for referenced entities |
| `knowledge_cards` / `knowledge_assertions` | Phase 4 | geography context only; never direct map edge source |

Important current-code note:

- `design.md` references `entity_links` as shared concept for cross-type links.
- Current committed code has `entity_identity_links`, but no concrete generic `entity_links` table.
- Phase 5 must add generic `entity_links` additively if it is still absent when coding starts.
- `entity_links` is for organization/place association, not identity merge and not relationship graph.

Frozen boundaries:

- Do not rewrite Phase 1 character/property foundation。
- Do not allow person-place facts into Phase 2 relationships。
- Do not change Phase 3 identity merge semantics。
- Do not turn Phase 4 geography knowledge summary into `place_edges`。
- Do not hardcode complex Chinese location/direction dictionaries in backend.

## 5. Core Invariants

1. AI 不直接写 `place_details` / `place_edges` / `map_layout_snapshots` / `entity_links`。
2. Extractor 只输出 `location_introduction` / `location_edge` observations。
3. Map Conflict Judge 只输出 decision，不直接写 canonical。
4. `place_reducer` 是唯一能写 `place_details` / `place_edges` 的入口。
5. Place entity 必须复用 `entities(entity_type=place)`。
6. Place aliases 必须复用 `entity_aliases`。
7. `place_details` 是 place entity 的扩展表。
8. `place_edges` 必须用 place entity ID，不用原始名字。
9. `layout_snapshot` 是 projection，不是事实源。
10. 地点和组织同名时不要强行合并；通过 `entity_links` 关联 organization <-> place。
11. Phase 3 redirect resolution 必须用于 referenced entities。
12. Phase 4 geography knowledge 不等于 Phase 5 map edge。
13. `claim.status = accepted` 只能在 `place_reducer` transaction 成功后标记。
14. 高风险 / 冲突 / topology-changing edge 必须进 Map Conflict Judge。
15. 低风险 `location_introduction` 可以不走 AI Judge，但必须经过 resolver + reducer + evidence gate。
16. 不要因为一次性提到地点就创建高重要度地图节点。
17. 不要把人物当前位置写成 `place_edges`。
18. 不要把 organization / faction affiliation 写成 `place_edges`。
19. 不要把 knowledge geography summary 写成 `place_edges`。
20. `place_details.parent_place_id` is the canonical hierarchy source。
21. `contains` / `inside` / `part_of` location edges are hierarchy update candidates, not a second hierarchy authority。
22. Direction / inverse edge normalization must happen before duplicate detection and conflict detection。
23. Directed canonical forms preserve one normalized representation; undirected edges canonicalize pair。
24. `place_edges` stores accepted canonical edges only: `active` / `deprecated`。
25. Conflict / uncertain / rejected candidate edges stay in `claims.value_json`, `ai_runs.output_json`, and `place_edge_conflicts`。
26. Place reducer failure 不得回滚 Phase 1/2/3/4 canonical state。

## 6. Data Model and Migration Plan

新增 migration 建议：

`src/storage/db/migrations/0008_v4_places_map.sql`

### 6.1 Tables to Add

#### `place_details`

字段建议：

- `entity_id TEXT PRIMARY KEY`
- `book_id TEXT NOT NULL`
- `place_type TEXT NOT NULL`
- `parent_place_id TEXT`
- `scale_level INTEGER NOT NULL DEFAULT 0`
- `importance_score REAL NOT NULL DEFAULT 0.5`
- `map_visible INTEGER NOT NULL DEFAULT 1`
- `first_seen_chapter INTEGER NOT NULL`
- `last_seen_chapter INTEGER NOT NULL`
- `status TEXT NOT NULL DEFAULT 'active'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Rules:

- `entity_id FK -> entities(id)`。
- `entity_id` is primary key or unique。
- `entity_id` must refer to `entities.entity_type = 'place'`; SQLite may not enforce this directly, so reducer/service must check。
- `parent_place_id nullable`, FK -> `entities(id)`。
- `parent_place_id` must refer to place entity when present。
- `parent_place_id != entity_id`。
- Parent-child cycle must be prevented by reducer。
- `map_visible=false` hides from overview but not detail/search。
- `place_details` stores current place extension state; historical evidence remains in claims/source spans。

`place_type` enum:

- `world`
- `continent`
- `region`
- `country`
- `city`
- `sect_site`
- `building`
- `room`
- `mountain`
- `river`
- `forest`
- `road`
- `route`
- `secret_realm`
- `battlefield`
- `dungeon`
- `unknown`

Enum notes:

- `sect_site` means physical sect site/base/territory, not organization entity。
- Organization remains `entity_type=organization`。
- Named stable roads/routes can be place entities, e.g. 青云古道 / 天门栈道。
- Generic travel relation should be a `place_edges.route_to` edge。
- Do not create unnamed route place unless the route itself is named or repeatedly important。

`status` enum:

- `active`
- `inactive`
- `uncertain`
- `deprecated`

#### `place_edges`

字段建议：

- `id TEXT PRIMARY KEY`
- `book_id TEXT NOT NULL`
- `from_place_id TEXT NOT NULL`
- `to_place_id TEXT NOT NULL`
- `edge_type TEXT NOT NULL`
- `direction_hint TEXT`
- `distance_hint TEXT`
- `confidence REAL NOT NULL DEFAULT 0.5`
- `source_claim_id TEXT NOT NULL`
- `latest_source_claim_id TEXT`
- `first_seen_chapter INTEGER NOT NULL`
- `last_seen_chapter INTEGER NOT NULL`
- `status TEXT NOT NULL DEFAULT 'active'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

Rules:

- FK `from_place_id -> entities(id)`。
- FK `to_place_id -> entities(id)`。
- FK `source_claim_id -> claims(id)`。
- FK `latest_source_claim_id -> claims(id)` nullable。
- `from_place_id != to_place_id`。
- from/to must be place entities; reducer/service must check。
- same book + normalized pair + `edge_type` + `direction_hint` should not create duplicate active edge。
- `source_claim_id` is the first accepted evidence claim for the edge。
- `latest_source_claim_id` points to the most recent accepted duplicate/supplement claim when available。
- Undirected edges canonicalize pair。
- Directed edges normalize inverse forms before duplicate/conflict detection。
- Extractor / claim payload may contain inverse forms like `south_of` / `west_of` / `inside` / `part_of`。
- Accepted `place_edges.edge_type` rows must store normalized canonical forms such as `north_of` / `east_of` / `contains`。
- Conflict / uncertain / rejected candidate edges are not stored as canonical `place_edges` rows。

`edge_type` enum:

- `contains`
- `part_of`
- `near`
- `adjacent_to`
- `route_to`
- `north_of`
- `south_of`
- `east_of`
- `west_of`
- `northeast_of`
- `northwest_of`
- `southeast_of`
- `southwest_of`
- `upstream_of`
- `downstream_of`
- `inside`
- `entrance_to`
- `connects_to`
- `unknown_spatial`

`status` enum:

- `active`
- `deprecated`

Hierarchy canonicalization:

- `A contains B`
- `B inside A`
- `B part_of A`

All normalize to canonical `A contains B`。
Reducer sets `B.parent_place_id = A` when cycle-safe and non-conflicting。
If B already has a different active parent, this is topology-changing and must go through Map Structural Gate / Map Conflict Judge。
`place_edges` may store a corresponding `contains` edge for graph display, but it must not contradict `place_details.parent_place_id`。
`place_hierarchy` projection reads `parent_place_id` as source of truth。

Direction / inverse canonicalization:

- `A north_of B == B south_of A`; prefer `north_of` by swapping pair。
- `A east_of B == B west_of A`; prefer `east_of` by swapping pair。
- `A northeast_of B == B southwest_of A`; prefer `northeast_of` by swapping pair。
- `A northwest_of B == B southeast_of A`; prefer `northwest_of` by swapping pair。
- `A upstream_of B == B downstream_of A`; prefer `upstream_of` by swapping pair。

Duplicate detection and conflict detection must run after hierarchy/direction normalization。

#### `place_edge_sources`

Lightweight evidence history for accepted duplicate/supplement edge claims.

字段建议：

- `id TEXT PRIMARY KEY`
- `book_id TEXT NOT NULL`
- `edge_id TEXT NOT NULL`
- `source_claim_id TEXT NOT NULL`
- `chapter_index INTEGER NOT NULL`
- `confidence REAL NOT NULL DEFAULT 0.5`
- `created_at TEXT NOT NULL`

Rules:

- FK `edge_id -> place_edges(id)`。
- FK `source_claim_id -> claims(id)`。
- `UNIQUE(edge_id, source_claim_id)`。
- Duplicate same edge must not create duplicate active edge。
- Duplicate same edge may update `place_edges.last_seen_chapter`, `confidence`, and `latest_source_claim_id` when appropriate。
- Earlier source evidence must remain queryable via `place_edge_sources`。
- This is not a full revision/audit system; conflict candidates still use `place_edge_conflicts`。

#### `map_layout_snapshots`

字段建议：

- `id TEXT PRIMARY KEY`
- `book_id TEXT NOT NULL`
- `max_chapter INTEGER NOT NULL`
- `layout_version TEXT NOT NULL`
- `layout_json TEXT NOT NULL`
- `source_edge_hash TEXT NOT NULL`
- `created_at TEXT NOT NULL`

Rules:

- `layout_json` is projection artifact, not canonical fact。
- `source_edge_hash` decides whether rebuild is needed。
- `max_chapter` indicates map state through that chapter。
- No AI image / drawing can become fact source。
- Layout can fail without failing topology graph API。

#### Optional but recommended `place_edge_conflicts`

字段建议：

- `id TEXT PRIMARY KEY`
- `book_id TEXT NOT NULL`
- `new_edge_claim_id TEXT NOT NULL`
- `existing_edge_id TEXT`
- `conflict_type TEXT NOT NULL`
- `reason_code TEXT NOT NULL`
- `judge_output_json TEXT`
- `status TEXT NOT NULL DEFAULT 'open'`
- `created_at TEXT NOT NULL`

Rules:

- Use this table unless implementation intentionally stores all conflict detail in `claims.value_json` + `ai_runs.output_json` only。
- Recommended because API needs `GET /map/conflicts` and UI conflict indicator。
- FK `new_edge_claim_id -> claims(id)`。
- FK `existing_edge_id -> place_edges(id)` nullable。
- Conflict records are audit/projection support, not active map edges。

`conflict_type` enum:

- `opposite_direction`
- `hierarchy_cycle`
- `duplicate_conflicting_direction`
- `impossible_containment`
- `unclear_reference`
- `low_evidence`
- `none`

#### Generic `entity_links`

Add only if not present at coding start.

字段建议：

- `id TEXT PRIMARY KEY`
- `book_id TEXT NOT NULL`
- `entity_a_id TEXT NOT NULL`
- `entity_b_id TEXT NOT NULL`
- `link_type TEXT NOT NULL`
- `source_claim_id TEXT NOT NULL`
- `confidence REAL NOT NULL DEFAULT 0.5`
- `status TEXT NOT NULL DEFAULT 'active'`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

`link_type` enum for Phase 5:

- `organization_place_pair`
- `based_at`
- `headquarters_of`
- `related_entity`

Rules:

- FK both entities。
- FK `source_claim_id -> claims(id)`。
- `entity_links` are cross-type semantic links, not identity redirects and not relationship graph。
- Never use `entity_links` to merge organization/place。
- Canonicalize pair only for symmetric `related_entity`; preserve direction for `based_at` and `headquarters_of`。

### 6.2 Existing Tables to Reuse

Must reuse:

- `entities`
- `entity_aliases`
- `entity_links` if already present; otherwise add it as above。
- `claims`
- `claim_source_spans`
- `source_spans`
- `ai_runs`
- `view_model_cache`
- `knowledge_cards` / `knowledge_assertions` as context only
- `relationships` as context only
- `entity_identity_links` redirect resolution

### 6.3 Indexes and Constraints

Suggested indexes:

- `idx_place_details_book_status ON place_details(book_id, status)`
- `idx_place_details_book_type ON place_details(book_id, place_type, status)`
- `idx_place_details_parent ON place_details(book_id, parent_place_id)`
- `idx_place_edges_book_status ON place_edges(book_id, status)`
- `idx_place_edges_from ON place_edges(book_id, from_place_id, edge_type, status)`
- `idx_place_edges_to ON place_edges(book_id, to_place_id, edge_type, status)`
- `idx_place_edges_source_claim ON place_edges(source_claim_id)`
- `idx_map_layout_snapshots_book_chapter ON map_layout_snapshots(book_id, max_chapter, layout_version)`
- `idx_map_layout_snapshots_edge_hash ON map_layout_snapshots(book_id, source_edge_hash)`
- `idx_place_edge_conflicts_book_status ON place_edge_conflicts(book_id, status, conflict_type)`
- `idx_place_edge_conflicts_claim ON place_edge_conflicts(new_edge_claim_id)`
- `idx_entity_links_book_pair ON entity_links(book_id, entity_a_id, entity_b_id, link_type, status)`
- `idx_entity_links_source_claim ON entity_links(source_claim_id)`

Constraints / service checks:

- `place_details.entity_id` unique / primary key。
- `place_details.parent_place_id != entity_id`。
- Parent cycle rejected by reducer。
- `place_edges.from_place_id != to_place_id`。
- Undirected edge pair canonicalized before insert。
- Directed edge direction preserved。
- Active duplicate edge prevented by repo/service。
- `entity_links` never link entity to itself。
- Enum CHECK constraints for table statuses/types where practical。

### 6.4 Additive Migration Safety

Migration must be additive:

1. Add Phase 5 tables only。
2. Do not alter Phase 1-4 table semantics。
3. Do not rewrite existing `entities`, `entity_aliases`, `relationships`, `knowledge_*`, `entity_identity_links` rows。
4. Do not backfill place_edges from Phase 4 geography knowledge。
5. Do not narrow global `claims.status` lifecycle。
6. Add `entity_links` only if concrete table is absent。
7. Keep rollback simple: dropping Phase 5 tables leaves Phase 1-4 data intact。

### 6.5 reset_v4 FK-safe Order

Phase 5 tables must be inserted into existing `reset_v4` order; do not replace existing reset.

Recommended order:

1. `map_layout_snapshots`
2. `place_edge_conflicts`
3. `place_edge_sources`
4. `place_edges`
5. `place_details`
6. `entity_links`
7. `knowledge_assertion_entities`
8. `knowledge_assertion_links`
9. `knowledge_assertions`
10. `knowledge_cards`
11. `entity_merge_conflicts`
12. `entity_merge_operations`
13. `entity_identity_links`
14. `relationship_events`
15. `relationships`
16. `claim_source_spans`
17. `entity_current_properties`
18. `entity_properties`
19. `entity_aliases`
20. `claims`
21. `entities`

If current schema also clears `view_model_cache`, `chapter_summaries`, `chapter_processing_runs`, `processing_progress`, `ai_runs`, `source_spans`, preserve existing FK-safe order.

Core principles:

- All tables referencing `place_edges` delete before `place_edges`。
- `place_edges` delete before `place_details`。
- `place_details` delete before place entities。
- All claim-referencing tables delete before `claims`。
- All entity-referencing tables delete before `entities`。

## 7. Observation / Claim Extensions

### 7.1 `location_introduction` Observation Schema

Purpose:

- first place appearance
- important place information update
- place alias
- place type
- parent hierarchy
- map visibility / importance hint

Fields:

- `place_mention: string`
- `place_type: string`
- `parent_place_mention: string | null`
- `aliases: string[]`
- `description: string | null`
- `importance_score: number`
- `map_visible_hint: boolean | null`
- `evidence_span_ids: string[]`
- `confidence: number`

Rules:

- Must be place / region / building / secret realm / route / natural geographic object。
- Do not treat organization as place unless text explicitly says the name also refers to a place。
- If same-name organization and place both exist, create separate place entity and link through `entity_links`。
- `place_type` must be enum。
- `parent_place_mention` may be null。
- Evidence spans required。
- Low-risk introduction may skip AI judge but cannot skip resolver/reducer/evidence checks。

### 7.2 `location_edge` Observation Schema

Purpose:

- stable / meaningful spatial relation between places
- route / direction / containment / adjacency / topology

Fields:

- `from_place_mention: string`
- `to_place_mention: string`
- `edge_type: string`
- `direction_hint: string | null`
- `distance_hint: string | null`
- `evidence_span_ids: string[]`
- `confidence: number`
- `is_topological_hint: boolean`

Rules:

- `edge_type` must be enum。
- Evidence spans required。
- Do not write character movement route as map topology。
- Do not write current character location as `place_edge`。
- Do not write faction affiliation as `place_edge`。
- Do not write relationship or identity reveal as `place_edge`。
- Do not write Phase 4 geography knowledge summary as `place_edge`。
- Map edge must express two-place stable or meaningful spatial relation。

### 7.3 Claim Mapping

New claim types:

- `location_introduction`
- `location_edge`
- `map_conflict` optional audit-only

Mapping rules:

- `location_introduction` default `status = proposed`。
- `location_edge` default `status = proposed`。
- `map_conflict` is audit-only and does not directly enter canonical。
- All `location_edge` claims go through Map Structural Gate。
- Conflicting / directional / topology-changing `location_edge` claims go through Map Conflict Judge。
- Low-risk `location_introduction` can skip AI Judge only when it creates a new place or adds aliases/details without changing existing hierarchy。
- If `location_introduction` sets `parent_place_id` for an existing place, changes an existing `parent_place_id`, or conflicts with an existing `contains` / `inside` / `part_of` edge, it is topology-changing and must go through Map Structural Gate / Map Conflict Judge。
- accepted only after `place_reducer` succeeds。
- rejected / uncertain do not enter canonical place tables。
- conflict edge can be preserved in claims/conflict table, but cannot enter active map。
- Do not narrow global `claims.status` enum。

### 7.4 Evidence Requirements

- `location_introduction` must have source span evidence。
- `location_edge` must have source span evidence。
- Place entity creation must be traceable to source claim/source spans。
- Place alias insert must be traceable to source claim/source spans。
- Place edge insert/update must be traceable to source claim/source spans。
- Conflict records must reference new edge claim and judge output。
- Layout snapshot source is active map graph hash, not textual evidence。
- Phase 4 geography knowledge may be included as context, but cannot be evidence for canonical edge by itself。

### 7.5 Place vs Knowledge / Relationship / Identity Boundary

Place/map examples:

- “青云城位于东域” -> `location_introduction` + parent/region if supported。
- “青云门山门在青云山中” -> place + contains/inside edge。
- “黑风谷在青云城以北” -> `north_of` edge。
- “从青云城到黑风谷有一条古道” -> `route_to` edge。
- “青云门山门属于青云门” -> organization_place link + place, not merge。

Not place/map examples:

- “张三去了青云城” -> character location property or minor_event, not `place_edge`。
- “张三属于青云门” -> relationship/faction membership/property, not `place_edge`。
- “青云门是正道大派” -> knowledge / organization, not `place_edge`。
- “黑衣人其实是张三” -> identity reveal。
- “北境常年冰封” -> geography knowledge or place detail context, not topology edge unless paired with place-local detail policy。

## 8. Place Resolver Contract

Place Resolver maps place mentions to place entities.

Input:

- place mention
- place_type
- parent_place_mention
- aliases
- existing place entities
- existing `entity_aliases`
- `entity_links`
- Phase 3 redirects
- current knowledge context
- current map context

Output:

- `action`:
  - `use_existing_place`
  - `create_new_place`
  - `uncertain`
- `place_entity_id` if existing
- `canonical_name`
- `display_name`
- `place_type`
- `parent_place_id`
- `confidence`
- `reason`

Rules:

- Same-name character / organization / place cannot blindly merge。
- Place entity only uses `entity_type = place`。
- Organization “青云门” and place “青云门山门” may be linked via `entity_links`。
- If source text explicitly says “青云门” is both organization and place, create distinct place entity and link to organization entity。
- Parent place best-effort resolves。
- Parent unresolved can create place without parent; fill later。
- Low-confidence resolution -> claim uncertain, no canonical write。
- Phase 3 merged victim entity must resolve survivor。
- No embedding / vector search。
- Deterministic lookup order:
  1. same book exact normalized place entity name
  2. same book place alias
  3. same type + parent candidate
  4. recent/nearby place candidates
  5. organization same-name candidate for possible `entity_links`, not merge

## 9. Map Conflict Judge Contract

Map Conflict Judge is for risky topology, conflicts, and direction/containment decisions.

### 9.1 Code Structural Gate

Reject:

- missing evidence
- invalid `edge_type`
- from/to both unresolved
- from == to
- from/to resolved but not place entity
- edge is clearly character movement / current location / affiliation / relationship / identity / pure knowledge summary
- confidence extremely low
- edge would create immediate impossible self-cycle in contains hierarchy

Pass:

- valid evidence
- valid place mentions
- valid edge_type
- from/to resolved or resolvable place entities
- no obvious structural contradiction

Uncertain:

- one side unresolved
- ambiguous place vs organization
- parent hierarchy unclear
- edge may be map-relevant but evidence is weak

### 9.2 Real AI Map Conflict Judge

Implement:

- `MockMapConflictJudge`: deterministic tests only。
- `DefaultMapConflictJudge`: safe fallback, not Product Ready。
- `RealAiMapConflictJudge`: required for Product Ready; gated real smoke must pass。

`ai_runs` fields:

- `run_type = map_conflict_judge`
- `model`
- `prompt_version`
- `schema_version`
- `input_hash`
- `output_json`
- `status`
- `error`
- `started_at`
- `finished_at`

Prompt requirements:

- include lowercase `json` for OpenAI-compatible `json_object` mode
- include source spans and existing map context
- remind model: geography knowledge is context only
- require conservative conflict handling
- forbid final map art / coordinate invention
- require `reason_code` and `explanation_for_log`

### 9.3 Judge Input Schema

Input includes:

- new topology candidate claim:
  - `location_edge` claim
  - or `location_introduction` claim with `parent_place_id` set/change
  - or derived hierarchy update candidate normalized from `parent_place_mention`
- resolved from/to place candidates
- source spans
- existing active `place_edges` involving from/to
- existing parent hierarchy
- existing map conflicts
- relevant knowledge geography cards
- recent chapter context if needed
- boundary warnings from structural gate

### 9.4 Judge Output Schema

Fields:

- `decision`:
  - `accept`
  - `conflict`
  - `uncertain`
  - `reject`
- `normalized_edge_type`
- `normalized_direction_hint`
- `normalized_distance_hint`
- `confidence`
- `reason_code`
- `explanation_for_log`
- `affected_edge_ids`
- `conflict_type`

`conflict_type` enum:

- `opposite_direction`
- `hierarchy_cycle`
- `duplicate_conflicting_direction`
- `impossible_containment`
- `unclear_reference`
- `low_evidence`
- `none`

Validation:

- strict JSON parse
- repair once if invalid
- enum validation
- confidence clamp/validation
- `conflict` requires meaningful conflict_type
- `accept` cannot use conflict_type other than `none`
- invalid output -> uncertain / no canonical edge

### 9.5 Decision Handling

- `accept`: claim may enter place_reducer and create/update active edge。
- `conflict`: claim status = uncertain/conflict-like payload; do not create active edge; record conflict。
- `uncertain`: claim status = uncertain; no active edge。
- `reject`: claim status = rejected。
- Judge cannot directly write place tables。
- Reducer success required before `claim.status = accepted`。
- If global claim status enum has no `conflict`, store conflict in `value_json` and/or `place_edge_conflicts`, with claim `status = uncertain`。

## 10. Place Reducer Contract

Input:

- proposed `location_introduction` / `location_edge` claim
- resolver output
- map judge output if applicable
- source spans

### `location_introduction` flow

1. Open transaction。
2. Find or create `entities(entity_type=place)`。
3. Insert / update `entity_aliases`。
4. Insert / update `place_details`。
5. Set `parent_place_id` if resolved and cycle-safe。
6. Update `importance_score` / `map_visible` / `last_seen_chapter`。
7. Create `entity_links` for organization/place pair if evidence supports it。
8. If setting/changing parent for an existing place or conflicting with existing hierarchy edge, require Map Structural Gate / Map Conflict Judge acceptance first。
9. Set claim `accepted` after transaction success。
10. Invalidate map projection。
11. Commit。

### `location_edge` flow

1. Open transaction。
2. Ensure from_place and to_place exist or create if allowed。
3. Ensure both are `entity_type=place`。
4. Normalize containment and direction/inverse forms。
5. Canonicalize pair for undirected edge types。
6. Check duplicate existing active edge after normalization。
7. If duplicate same edge: do not create duplicate active edge; insert `place_edge_sources`; update confidence / `last_seen_chapter` / `latest_source_claim_id` when appropriate。
8. Preserve original `source_claim_id`; do not lose earlier claim/source span evidence。
9. If conflict: do not create active edge; record conflict; mark claim uncertain/rejected as appropriate。
10. Insert or update `place_edges` only for accepted canonical edges。
11. If `contains` / `inside` / `part_of`, update `parent_place_id` as canonical hierarchy source when cycle-safe。
12. Set claim `accepted` after transaction success。
13. Invalidate map projection。
14. Commit。

Failure:

- rollback
- claim remains proposed or becomes uncertain
- no partial canonical map write
- no rollback of Phase 1/2/3/4 canonical state

## 11. Place / Organization Linking Contract

Phase 5 must handle same-name organization/place safely.

Rules:

- `entity_type=organization` and `entity_type=place` are distinct entities。
- Never merge organization and place。
- If source text implies relation, create `entity_links`:
  - `organization_place_pair`
  - `based_at`
  - `headquarters_of`
  - `related_entity`
- `entity_links` are not map edges。
- Projection may display linked organization on place card。
- `place_edges` only connect place entities。
- Relationships still only connect character entities。
- Knowledge can reference both organization and place。

Implementation note:

- If `entity_links` table is absent, Task 1 adds it。
- If later Phase 3 identity redirect merges one side, projection must resolve active redirect before display where applicable。

## 12. Map Projection and Layout Snapshot Contract

Projection outputs:

- `map_overview`
- `place_detail`
- `place_hierarchy`
- `map_graph`
- `map_layout_snapshot`

`view_model_cache.view_type` values:

- `map_overview`
- `place_detail`
- `place_hierarchy`
- `map_graph`
- `map_layout_snapshot`

Cache key must include:

- `book_id`
- `view_type`
- `scope_key`

`scope_key` examples:

- `map_overview: "__book__"`
- `place_detail: place_entity_id`
- `place_hierarchy: "__book__"`
- `map_graph: "__book__"`
- `map_layout_snapshot: max_chapter or latest`

Map projection rules:

- only active places / active edges by default
- uncertain/conflict/rejected edges excluded from normal map
- `map_visible=false` places hidden from overview but available in search/detail
- parent-child hierarchy built from `place_details.parent_place_id` as the only hierarchy source of truth
- graph edges built from active `place_edges`
- `layout_json` generated from graph structure
- layout may include x/y coordinates for display only
- coordinates are not canonical facts
- layout snapshot should regenerate when active edge hash changes
- no AI image map as fact source

Layout strategy Phase 5:

- simple deterministic layout is enough
- hierarchy first: world / region / country / city / sect_site / building / room
- non-hierarchical edges as graph links
- if layout fails, API still returns topology graph without coordinates
- complex visual layout can improve later

## 13. API Contract

V4 route convention:

- `/api/books/v4/...` + `bookUrl` query/body
- internal `book_id = md5_hex(book_url)`

New APIs:

- `GET /api/books/v4/map`
- `GET /api/books/v4/map/places`
- `GET /api/books/v4/map/places/:placeId`
- `GET /api/books/v4/map/graph`
- `GET /api/books/v4/map/layout`
- `GET /api/books/v4/map/conflicts`

Optional:

- `GET /api/books/v4/map/hierarchy`

### `MapOverviewView`

Fields:

- `placeCount`
- `edgeCount`
- `visiblePlaceCount`
- `conflictCount`
- `topPlaces`
- `hierarchySummary`
- `updatedAt`

### `PlaceView`

Fields:

- `id`
- `name`
- `aliases`
- `placeType`
- `parentPlaceId`
- `parentPlaceName`
- `scaleLevel`
- `importanceScore`
- `mapVisible`
- `linkedOrganizations`
- `firstSeenChapter`
- `lastSeenChapter`
- `evidenceAvailable`

### `MapGraphView`

Fields:

- `nodes`:
  - `id`
  - `name`
  - `placeType`
  - `scaleLevel`
  - `importanceScore`
  - `mapVisible`
  - `parentPlaceId`
- `edges`:
  - `id`
  - `fromPlaceId`
  - `toPlaceId`
  - `edgeType`
  - `directionHint`
  - `distanceHint`
  - `confidence`
  - `sourceClaimId`
  - `status`

### `MapLayoutView`

Fields:

- `maxChapter`
- `layoutVersion`
- `nodes`:
  - `placeId`
  - `x`
  - `y`
  - `label`
  - `placeType`
- `edges`:
  - `edgeId`
  - `fromPlaceId`
  - `toPlaceId`
- `warnings`

## 14. Frontend Plan

Phase 5 frontend should be lightweight but usable.

Add:

- `V4MapPanel.vue`
- frontend types
- frontend API client methods
- API tests
- component tests

UI:

- Map tab / “地图” tab
- place list
- hierarchy tree
- simple graph view
- place detail panel
- conflict indicator
- evidence indicator
- map layout if available
- fallback graph/table if layout missing

Do not add:

- manual map editor
- draggable layout persistence
- user correction UI
- complex visual style
- AI generated image map

Implementation notes:

- Preserve current V4 panel layout patterns。
- Do not replace Character / Relationship / Identity / Knowledge panels。
- Conflict / uncertain edges must be visually separated and not shown as active map truth。
- If layout is missing, graph/table fallback must still be readable。

## 15. End-to-end Processing Flow

1. Extract observations。
2. Initial resolver resolves place mentions and candidates。
3. ClaimWriter writes `location_introduction` / `location_edge` claims with `status = proposed`。
4. Existing Phase 1/2/3/4 reducers run as before。
5. Place Resolver re-resolves place mentions using updated entities / aliases / identity redirects。
6. Map Structural Gate。
7. RealAiMapConflictJudge / MockMapConflictJudge for topology-changing or risky edges。
8. `place_reducer` transaction。
9. map projection / layout snapshot rebuild。
10. API。
11. Frontend。

Notes:

- Place stage should run after Phase 1/2/3/4 reducers in the same chapter pipeline, so referenced entities, knowledge context, and redirects are up to date。
- Place reducer failure must not roll back Phase 1/2/3/4 canonical state。
- If map processing fails, claim remains proposed/uncertain and can be reprocessed。

## 16. Failure Handling and Idempotency

Must cover:

- same place mention repeated -> no duplicate place
- same alias repeated -> no duplicate alias
- same edge repeated -> no duplicate active edge
- conflicting edge -> does not overwrite old edge
- parent self-reference rejected
- parent cycle rejected
- hierarchy conflict does not write active edge
- unresolved parent can be filled later
- map conflict judge failure -> claim uncertain/quarantined, no canonical edge
- layout rebuild failure -> graph still available
- reset_v4 cleans map tables FK-safely
- ai_run failure recorded
- organization/place same-name -> link, not merge
- Phase 3 merged victim -> survivor in projection/resolver
- active map graph excludes conflict/rejected/uncertain edges

## 17. Task Breakdown

### Task 0: Inspect Phase 1/2/3/4 Frozen Boundary

**Goal**

Confirm frozen contracts and Phase 5 integration points before coding.

**Files / Areas likely affected**

- `.trellis/tasks/07-02-ai-redesign/design.md`
- `.trellis/tasks/07-02-ai-redesign/phase1-implmenet.md`
- `.trellis/tasks/07-02-ai-redesign/phase2-implement.md`
- `.trellis/tasks/07-02-ai-redesign/phase3-implement.md`
- `.trellis/tasks/07-02-ai-redesign/phase4-implmenet.md`
- `.trellis/tasks/07-02-ai-redesign/暂存勿看/PHASE_4_FINAL_VERIFICATION_REPORT.md`
- current V4 modules under `src/service/v4`, `src/storage/db/v4`, `src/api`, `frontend/src`

**Detailed steps**

1. Re-read Phase 4 Final Freeze Status。
2. Verify `entity_links` existence; if absent, keep Task 1 migration addition。
3. Record the current design/implementation alignment decision before coding:
   - `entity_identity_links` owns Phase 3 identity semantic links and reducer-created `redirect` links。
   - Generic `entity_links` is not an identity redirect table。
   - Generic `entity_links` is only for cross-type entity association, especially organization/place pairs。
   - Current code has `entity_identity_links` but no generic `entity_links`; this is an explicit Phase 5 prerequisite, not a hidden Phase 3 implementation bug。
4. Confirm no Phase 1-4 rewrite is required for Phase 5:
   - `entities(entity_type=place)` and `entity_aliases` are the place identity/alias foundation。
   - `view_model_cache` is already generic enough for map view types。
   - Phase 3 redirect resolution must be reused through `entity_identity_links` wherever Phase 5 displays referenced entities。
   - Phase 4 geography knowledge may be context only and must not be backfilled into `place_edges`。
5. Confirm Phase 1-4 regression commands。
6. Confirm non-scope boundaries。

**Acceptance criteria**

- Coding starts only after frozen boundaries are confirmed。
- No Phase 1-4 rewrite planned。
- `entity_links` handling decision is explicit:
  - do not move identity redirect semantics out of `entity_identity_links`。
  - add generic `entity_links` in Task 1 if still absent。
  - use generic `entity_links` for organization/place association, not identity merge and not relationship graph。
- Task 1 schema work remains additive and owns the first concrete generic `entity_links` implementation if the table is still absent。

**Tests**

- Targeted doc/code review only。
- `git status --short` baseline captured。

### Task 1: Add Place / Map Schema Migration

**Goal**

Add additive map schema using `entities(entity_type=place)`.

**Files / Areas likely affected**

- `src/storage/db/migrations/0008_v4_places_map.sql`
- `src/storage/db/v4/mod.rs`

**Detailed steps**

1. Add `place_details`。
2. Add `place_edges`。
3. Add `place_edge_sources`。
4. Add `map_layout_snapshots`。
5. Add `place_edge_conflicts` unless conflict details are intentionally stored only in claim/ai_run。
6. Add `entity_links` if absent。
7. Add indexes/checks。
8. Insert Phase 5 tables into `reset_v4` order。
9. Add schema tests before implementation。

**Acceptance criteria**

- Fresh DB has all required tables。
- FK/self-reference constraints work。
- Parent cycle service test exists。
- `place_edges.status` only allows accepted canonical statuses: `active` / `deprecated`。
- Duplicate edge source evidence can be preserved through `place_edge_sources`。
- reset clears map tables FK-safely。
- Existing Phase 1-4 schema tests remain green。

**Tests**

- `cargo test place_details_schema --lib`
- `cargo test place_edges_schema --lib`
- `cargo test map_layout_snapshots_schema --lib`
- `cargo test place_edge_sources_schema --lib`
- `cargo test entity_links_schema --lib`
- `cargo test reset_v4_cleans_map_tables --lib`
- `cargo test v4 --lib`

### Task 2: Add Place Repository

**Goal**

Add repository helpers for place details, edges, edge sources, conflicts, layout snapshots, and entity links.

**Files / Areas likely affected**

- `src/storage/db/v4/place_repo.rs`
- `src/storage/db/v4/mod.rs`

**Detailed steps**

1. Add place detail row structs/helpers。
2. Add edge find/create/update helpers。
3. Add undirected pair canonicalization helper。
4. Add duplicate active edge detection。
5. Add `place_edge_sources` insert/list helpers。
6. Add conflict insert/list helpers。
7. Add layout snapshot helpers。
8. Add entity link helpers if no generic repo exists。
9. Add `*_with_conn` variants for reducer transaction use。

**Acceptance criteria**

- Place entity helpers require `entity_type=place`。
- Duplicate edges are idempotent。
- Duplicate edge source evidence is preserved。
- Conflict records are queryable。
- Layout hash helpers are deterministic。

**Tests**

- `cargo test place_repo --lib`
- `cargo test undirected_edge_canonicalizes_pair --lib`
- `cargo test directed_edge_preserves_direction --lib`
- `cargo test duplicate_edge_idempotent --lib`
- `cargo test duplicate_edge_preserves_source_history --lib`
- `cargo test conflict_table_records_conflict --lib`

### Task 3: Extend Extractor Observation Schema and Prompt

**Goal**

Add `location_introduction` and `location_edge` observations.

**Files / Areas likely affected**

- `src/service/v4/extractor.rs`

**Detailed steps**

1. Add observation variants。
2. Parse and validate fields。
3. Add prompt schema and boundary examples。
4. Reject invalid place/edge types。
5. Reject missing evidence。
6. Add distractor examples for movement, organization, knowledge, relationship, identity。

**Acceptance criteria**

- Valid observations parse。
- Missing evidence/invalid enum rejects。
- Non-map facts do not parse as `location_edge`。
- Existing parser tests remain green。

**Tests**

- `cargo test location_introduction_parser --lib`
- `cargo test location_edge_parser --lib`
- `cargo test missing_place_evidence_rejected --lib`
- `cargo test invalid_place_type_rejected --lib`
- `cargo test invalid_edge_type_rejected --lib`
- `cargo test character_current_location_not_accepted_as_place_edge --lib`
- `cargo test relationship_not_accepted_as_place_edge --lib`
- `cargo test knowledge_geography_summary_not_accepted_as_place_edge --lib`

### Task 4: Add Location ClaimWriter + Resolver Mapping

**Goal**

Write location observations into claims and prepare resolver inputs.

**Files / Areas likely affected**

- `src/service/v4/claim_writer.rs`
- `src/service/v4/resolver.rs`

**Detailed steps**

1. Map `location_introduction` -> claim。
2. Map `location_edge` -> claim。
3. Preserve source spans。
4. Store normalized payload in `value_json`。
5. Keep claims `proposed` until reducer success。
6. Prevent location claims from generic reducer path。

**Acceptance criteria**

- Claims have source spans and complete `value_json`。
- `location_edge` always enters Phase 5 special path。
- Existing claim writer tests remain green。

**Tests**

- `cargo test location_introduction_creates_proposed_claim --lib`
- `cargo test location_edge_creates_proposed_claim --lib`
- `cargo test location_claims_preserve_source_spans --lib`

### Task 5: Add Place Resolver

**Goal**

Resolve place mentions to `entities(entity_type=place)`.

**Files / Areas likely affected**

- `src/service/v4/place_resolver.rs`
- `src/service/v4/mod.rs`
- `src/storage/db/v4/place_repo.rs`
- `src/storage/db/v4/entity_repo.rs`

**Detailed steps**

1. Lookup existing place entity by normalized name。
2. Lookup place aliases via `entity_aliases`。
3. Resolve parent place best-effort。
4. Resolve Phase 3 redirects。
5. Detect same-name organization as link candidate, not merge。
6. Return `use_existing_place`, `create_new_place`, or `uncertain`。
7. No vector/embedding search。

**Acceptance criteria**

- Exact and alias lookup work。
- Same-name organization is not merged with place。
- Parent unresolved can defer。
- Low confidence returns uncertain。

**Tests**

- `cargo test place_resolver_exact_match --lib`
- `cargo test place_resolver_alias_match --lib`
- `cargo test organization_not_merged_with_place --lib`
- `cargo test phase3_merged_victim_resolves_survivor_for_place_context --lib`

### Task 6: Add Map Structural Gate

**Goal**

Add Map Structural Gate for `location_edge` and hierarchy-changing `location_introduction` claims.

**Files / Areas likely affected**

- `src/service/v4/map_conflict_judge.rs` or `src/service/v4/map_gate.rs`

**Detailed steps**

1. Define structural gate result。
2. Reject missing evidence, invalid edge_type, self-edge。
3. Reject non-place from/to。
4. Reject character movement/current location/affiliation/relationship/identity/knowledge-only facts。
5. Detect immediate self-cycle in containment。
6. Treat `location_introduction` that sets or changes `parent_place_id` as topology-changing。
7. Mark ambiguous place-vs-organization and weak evidence uncertain。

**Acceptance criteria**

- Obvious non-map claims never reach reducer。
- Strong valid map edges can pass。
- Ambiguous claims do not write active map。
- `location_introduction` that sets or changes `parent_place_id` must pass structural gate / judge before reducer updates hierarchy。

**Tests**

- `cargo test map_gate_rejects_missing_evidence --lib`
- `cargo test map_gate_rejects_self_edge --lib`
- `cargo test map_gate_rejects_character_movement --lib`
- `cargo test map_gate_rejects_knowledge_summary --lib`
- `cargo test map_gate_rejects_immediate_hierarchy_cycle --lib`

### Task 7: Add Real AI Map Conflict Judge

**Goal**

Implement `MockMapConflictJudge`, `DefaultMapConflictJudge`, and `RealAiMapConflictJudge`.

**Files / Areas likely affected**

- `src/service/v4/map_conflict_judge.rs`
- `src/service/v4/mod.rs`
- `src/storage/db/v4/ai_run_repo.rs` if helper changes are needed

**Detailed steps**

1. Define trait/input/output。
2. Add mock/default/real implementations。
3. Build prompt with strict JSON。
4. Record `ai_runs.run_type = map_conflict_judge`。
5. Parse/repair output once。
6. Validate decisions/conflict_type。
7. Add lifecycle and invalid output tests。

**Acceptance criteria**

- Real judge records successful output_json。
- Invalid output does not write canonical map。
- Decision compatibility enforced。
- Product Ready requires real smoke later。

**Tests**

- `cargo test map_conflict_judge --lib`
- `cargo test validates_map_conflict_judge_decisions --lib`
- `cargo test map_conflict_judge_ai_run_lifecycle_records_success --lib`
- `cargo test invalid_map_conflict_output_repairs_once --lib`

### Task 8: Add Place Reducer

**Goal**

Write place details, edges, conflicts, aliases, and entity links safely.

**Files / Areas likely affected**

- `src/service/v4/reducer.rs`
- `src/service/v4/place_reducer.rs` if split
- `src/storage/db/v4/place_repo.rs`
- `src/storage/db/v4/entity_repo.rs`

**Detailed steps**

1. Add independent `reduce_location_claims` transaction。
2. Implement `location_introduction` flow。
3. Implement `location_edge` flow。
4. Validate place entity types。
5. Prevent parent cycles。
6. Handle duplicate edges idempotently。
7. Record conflicts without active edge writes。
8. Mark claim accepted only after transaction success。
9. Invalidate map projection cache。

**Acceptance criteria**

- Place entities created as `entity_type=place`。
- Aliases stored in `entity_aliases`。
- Parent hierarchy works。
- Duplicate edges are idempotent。
- Conflicts do not pollute active map。
- Rollback leaves no partial map state。

**Tests**

- `cargo test place_reducer_location_introduction_creates_place_entity --lib`
- `cargo test place_reducer_alias_stored_in_entity_aliases --lib`
- `cargo test place_reducer_parent_cycle_rejected --lib`
- `cargo test place_reducer_add_edge_active --lib`
- `cargo test place_reducer_conflicting_edge_records_conflict_not_active --lib`
- `cargo test place_reducer_rolls_back_on_failure --lib`

### Task 9: Add Place / Organization Linking

**Goal**

Support organization/place same-name cases through `entity_links`.

**Files / Areas likely affected**

- `src/storage/db/v4/place_repo.rs`
- `src/storage/db/v4/entity_repo.rs`
- `src/service/v4/place_resolver.rs`
- `src/service/v4/reducer.rs`
- `src/service/v4/place_projection.rs`

**Detailed steps**

1. Add/create generic entity link helpers。
2. Detect organization/place link candidates。
3. Create `organization_place_pair`, `based_at`, `headquarters_of`, or `related_entity` when source supports it。
4. Ensure link is not a map edge。
5. Expose linked organizations in place projection。

**Acceptance criteria**

- Organization and place are distinct entities。
- Link is created when evidence supports it。
- Relationship graph remains character-character only。
- Place card can display linked organization。

**Tests**

- `cargo test organization_place_entity_link_created --lib`
- `cargo test organization_not_merged_with_place --lib`
- `cargo test entity_links_are_not_map_edges --lib`
- `cargo test place_projection_shows_linked_organization --lib`

### Task 10: Add Map Projection / Layout Snapshot

**Goal**

Build map overview, place detail, hierarchy, graph, and layout snapshots.

**Files / Areas likely affected**

- `src/service/v4/place_projection.rs`
- `src/service/v4/map_projection.rs`
- `src/service/v4/map_layout_projection.rs`
- `src/service/v4/mod.rs`

**Detailed steps**

1. Project map overview counts/top places。
2. Project place detail including aliases/linked organizations/evidence。
3. Build hierarchy from `parent_place_id`。
4. Build graph from active edges。
5. Generate deterministic layout snapshot。
6. Hash active edges for rebuild。
7. Exclude uncertain/conflict/rejected edges from normal graph。
8. Return graph without layout if layout fails。

**Acceptance criteria**

- Map graph uses active place/edge state。
- Conflicts hidden from normal map but visible via conflict API。
- Layout snapshot generated from active edges。
- Layout failure still returns graph。

**Tests**

- `cargo test map_projection_overview_counts --lib`
- `cargo test place_hierarchy_from_parent_place_id --lib`
- `cargo test map_graph_hides_conflict_edges --lib`
- `cargo test layout_snapshot_generated_from_active_edges --lib`
- `cargo test layout_failure_still_returns_graph --lib`

### Task 11: Add Map APIs

**Goal**

Expose map views through V4 routes.

**Files / Areas likely affected**

- `src/api/handlers/v4.rs`
- `src/api/router.rs`
- `frontend/src/api/v4/book.ts`
- `frontend/src/types/v4.ts`

**Detailed steps**

1. Add backend response structs。
2. Add handlers for map overview, places, place detail, graph, layout, conflicts。
3. Register routes。
4. Add frontend types/methods。
5. Keep `bookUrl` convention。

**Acceptance criteria**

- All requested routes work。
- Existing V4 APIs still work。
- Frontend API methods call expected URLs。

**Tests**

- `cargo test v4_map_api_returns_overview_places_graph_layout_conflicts --lib`
- `cargo test v4_routes_are_registered --lib`
- `cd frontend && npx vitest run src/api/v4/book.map.test.ts`
- `cargo test v4 --lib`

### Task 12: Add Frontend Map Panel

**Goal**

Add lightweight usable Map tab.

**Files / Areas likely affected**

- `frontend/src/components/reader/V4MapPanel.vue`
- `frontend/src/components/reader/V4MapPanel.test.ts`
- `frontend/src/views/AiBookView.vue`
- `frontend/src/views/AiBookView.test.ts`
- `frontend/src/api/v4/book.ts`
- `frontend/src/types/v4.ts`

**Detailed steps**

1. Add “地图” tab。
2. Load map overview/places/graph/layout。
3. Render place list。
4. Render hierarchy tree。
5. Render simple graph or table fallback。
6. Render place detail panel。
7. Show conflict and evidence indicators。
8. Do not add manual editor/correction/image map UI。

**Acceptance criteria**

- Map tab visible。
- Place list and hierarchy render。
- Graph fallback works without layout。
- Conflicts/uncertain states are clear。
- Build passes。
- Existing panels unaffected。

**Tests**

- `cd frontend && npx vitest run src/api/v4/book.map.test.ts src/components/reader/V4MapPanel.test.ts src/views/AiBookView.test.ts`
- `cd frontend && npm run build`

### Task 13: Integrate into Pipeline

**Goal**

Run Phase 5 place/map stage after Phase 1/2/3/4 reducers.

**Files / Areas likely affected**

- `src/service/v4/pipeline.rs`
- `src/service/v4/context_builder.rs`
- `src/service/v4/reducer.rs`
- `src/service/v4/place_resolver.rs`
- `src/service/v4/map_conflict_judge.rs`

**Detailed steps**

1. Add `process_chapter_with_map_conflict_judge` wrapper。
2. Preserve existing process wrappers。
3. Split `location_introduction` / `location_edge` out of generic reducer input。
4. Run after Phase 4 knowledge path。
5. Use updated entities/aliases/identity redirects/knowledge context。
6. On map processing failure, keep claim proposed/uncertain and continue。
7. Invalidate/rebuild map projection as needed。

**Acceptance criteria**

- Location claims use Phase 5 special path。
- Existing Phase 1-4 processing order stable。
- Place reducer failure does not fail whole chapter after earlier phases succeed。
- Knowledge summary cannot directly create map edge。

**Tests**

- `cargo test phase5_pipeline_creates_place_and_edge --lib`
- `cargo test phase5_pipeline_failure_does_not_rollback_phase1_4 --lib`
- `cargo test phase5_pipeline_rejects_knowledge_summary_as_edge --lib`
- `cargo test pipeline --lib`

### Task 14: Add Phase 5 Tests

**Goal**

Complete unit/E2E/regression matrix.

**Files / Areas likely affected**

- Rust tests across Phase 5 modules
- frontend tests
- this implement doc progress notes if needed

**Detailed steps**

1. Add all required unit tests。
2. Add E2E positive map cases。
3. Add distractor cases。
4. Add conflict/cycle cases。
5. Add Phase 1-4 regression checks。
6. Keep tests deterministic。

**Acceptance criteria**

- Required test matrix covered by named tests。
- Mock E2E passes。
- Phase 1-4 regressions pass。
- Frontend focused tests/build pass。

**Tests**

- `cargo test phase5_ --lib`
- `cargo test map --lib`
- `cargo test place --lib`
- `cargo test v4 --lib`
- `cargo test --lib`
- frontend focused tests
- `git diff --check`

### Task 15: Real AI Map Smoke Test

**Goal**

Prove Product Ready real LLM map path with 3-5 chapters max.

**Files / Areas likely affected**

- `src/service/v4/pipeline.rs`
- `src/service/v4/map_conflict_judge.rs`

**Detailed steps**

1. Add gated `RUN_REAL_AI_TESTS=1` smoke。
2. Use env `AI_BASE_URL`, `AI_API_KEY`, `AI_MODEL`。
3. Fixture includes:
   - place introduction
   - parent/containment
   - directional edge
   - route edge
   - organization/place same-name case
   - character movement distractor
   - knowledge geography distractor
   - conflicting map statement
4. Print required diagnostic output。
5. Run with local `gpt-5.4` when available。
6. Record final result before freeze report。

**Acceptance criteria**

- RealAiMapConflictJudge actually called。
- `ai_runs.output_json` has real output。
- Places created。
- Edges created。
- Conflicts recorded。
- Layout snapshots created。
- False positives into map = 0。
- Phase 1/2/3/4 regressions pass。

**Tests**

- `RUN_REAL_AI_TESTS=1 AI_BASE_URL=http://127.0.0.1:8090 AI_MODEL=gpt-5.4 cargo test real_ai_smoke_test_map_phase5_fixture --lib -- --nocapture`
- `cargo test phase5_ --lib`
- `cargo test v4 --lib`

## 18. Test Plan

Unit tests:

- `place_details` schema constraints
- `place_edges` schema constraints
- `parent_place_id` cannot self-reference
- parent cycle rejected
- place entity must be `entity_type=place`
- undirected edge canonicalize pair
- inverse direction edge normalizes before duplicate/conflict detection
- containment edge normalizes to canonical `A contains B`
- directed edge preserves direction
- duplicate edge idempotent
- duplicate edge preserves earlier and latest source evidence
- conflicting edge not active
- conflict table records conflict
- `location_introduction` parser
- `location_edge` parser
- missing evidence rejected
- invalid `place_type` rejected
- invalid `edge_type` rejected
- character current location not accepted as `place_edge`
- relationship not accepted as `place_edge`
- knowledge geography summary not accepted as `place_edge`
- organization not merged with place
- organization_place `entity_link` created
- Phase 3 merged victim resolves survivor
- layout snapshot generated from active edges
- layout failure still returns graph

E2E tests:

- “青云城位于东域” -> place introduction + parent/region if supported
- “青云门山门在青云山中” -> place + contains/inside edge
- “黑风谷在青云城以北” -> `north_of` edge
- “青云城在黑风谷以南” -> same canonical edge as 黑风谷 `north_of` 青云城 after inverse normalization
- “从青云城到黑风谷有一条古道” -> `route_to` edge
- “张三去了青云城” -> character location property / minor_event, not `place_edge`
- “青云门是正道大派” -> knowledge / organization, not `place_edge`
- “青云门山门属于青云门” -> organization_place link + place, not merge
- conflicting direction edge -> conflict / uncertain, not active
- hierarchy cycle attempt -> reject
- map graph hides conflict edges
- layout snapshot generated
- Phase 1 character card still works
- Phase 2 relationship graph still works
- Phase 3 identity redirect still works
- Phase 4 knowledge panel still works

Regression commands:

- `cargo test phase5_ --lib`
- `cargo test map --lib`
- `cargo test place --lib`
- `cargo test v4 --lib`
- `cargo test --lib`
- `cd frontend && npx vitest run src/api/v4/book.map.test.ts src/components/reader/V4MapPanel.test.ts src/views/AiBookView.test.ts`
- `cd frontend && npm run build`
- `git diff --check`

## 19. Real AI Smoke Test

Gated by `RUN_REAL_AI_TESTS=1`。

Fixture: 3-5 chapters max。

Must include:

- place introduction
- parent/containment
- directional edge
- route edge
- organization/place same-name case
- character movement distractor
- knowledge geography distractor
- conflicting map statement

Required output:

- `AI_MODEL`
- `prompt_version`
- `schema_version`
- processed chapters
- `location_introduction` count
- `location_edge` count
- accepted / rejected / uncertain / conflict count
- places created
- edges created
- conflicts recorded
- layout snapshots created
- false positives into map
- false negatives
- Phase 1 regression
- Phase 2 regression
- Phase 3 regression
- Phase 4 regression

Product Ready pass criteria:

- RealAiMapConflictJudge actually called。
- `ai_runs.output_json` contains real judge output。
- At least one place entity created。
- At least one parent/containment relation processed。
- At least one directional edge processed。
- At least one route edge processed。
- At least one conflict recorded。
- Character movement distractor does not become `place_edge`。
- Knowledge geography distractor does not become `place_edge`。
- Organization/place same-name case creates link, not merge。
- Layout snapshot created from active edges。
- Phase 1/2/3/4 regressions pass。

## 20. Done Criteria

Phase 5 done iff:

- `place_details` schema implemented
- `place_edges` schema implemented
- `place_edge_sources` schema implemented
- `map_layout_snapshots` schema implemented
- `location_introduction` / `location_edge` observations parsed and validated
- Place Resolver implemented
- Map Structural Gate implemented
- RealAiMapConflictJudge implemented and recorded in `ai_runs`
- `place_reducer` transaction safe
- place entities created as `entity_type=place`
- aliases stored in `entity_aliases`
- parent hierarchy works
- parent cycle rejected
- directed / undirected edge handling correct
- inverse direction normalization correct
- `parent_place_id` is canonical hierarchy source
- duplicate edges idempotent
- duplicate edge source evidence preserved
- conflicting edges do not pollute active map
- organization/place same-name handled via `entity_links`, not merge
- layout snapshot generated from active edges
- layout failure still returns topology graph
- Map APIs work
- frontend Map Panel works
- Mock E2E passes
- Real AI smoke passes
- `cargo test` passes
- `npm run build` passes
- Phase 1 regression passes
- Phase 2 relationship regression passes
- Phase 3 identity regression passes
- Phase 4 knowledge regression passes

## 21. Implementation Risks

1. place / organization same-name false merge.
   - Mitigation: separate entities by `entity_type`; use `entity_links`; tests for organization not merged with place.

2. Character current location pollutes `place_edges`.
   - Mitigation: extractor prompt boundaries, structural gate, E2E distractor.

3. Geography knowledge summary pollutes map topology.
   - Mitigation: Phase 4 knowledge as context only; gate rejects knowledge-only statements.

4. Direction edge conflict or inverse duplicate.
   - Mitigation: normalize inverse directions before duplicate/conflict detection, then use Map Conflict Judge and conflict table when incompatible.

5. Containment cycle or hierarchy double-source conflict.
   - Mitigation: `parent_place_id` is hierarchy source of truth; `contains` / `inside` / `part_of` normalize to parent update candidates; reducer checks self-parent and ancestor traversal.

6. Duplicate place caused by aliases.
   - Mitigation: `entity_aliases` lookup, normalized exact match, resolver candidate ranking.

7. Secret realm / route place_type boundary unclear.
   - Mitigation: use `sect_site` for physical sect sites; named roads/routes may be place entities, generic travel stays `route_to`; uncertain on low confidence; real smoke coverage.

8. Layout snapshot mistaken as fact.
   - Mitigation: projection-only docs, source_edge_hash, no reducer reads layout.

9. Real AI schema drift.
   - Mitigation: strict JSON, lowercase json prompt, repair once, ai_run audit.

10. Map UI conflict / uncertain edge display unclear.
    - Mitigation: conflict indicator, separate conflict API, active map hides conflicts.

11. Phase 5 accidentally expands into manual map editor.
    - Mitigation: frontend task explicitly excludes editor, draggable persistence, correction UI, AI image map.

12. Duplicate edge update loses evidence.
    - Mitigation: preserve first `source_claim_id`, maintain `latest_source_claim_id`, and insert duplicate/supplement evidence into `place_edge_sources`.
