# AI Book Memory V3 Backend Engine Rewrite

> Scope: destructive rewrite for Reader AI资料. V1/V2 AI Book memory is generated cache and is not runtime-compatible with V3.

## 0. Current Codebase Reality

Current AI Book semantics are split between frontend and backend.

Backend today:

- `src/model/ai_book.rs` still defines the old display-oriented memory shape: `summary`, `worldview`, `characters`, `relationships`, `locations`, `map`, `mapDirty`, `lastError`.
- `src/service/ai_book_service.rs` stores AI memory as `serde_json::Value`, validates shallow JSON constraints, and writes the whole JSON blob to `ai_book_memories`.
- Current validation only accepts schemaVersion 1 or 2.
- `src/api/handlers/ai_book.rs` exposes raw-memory handlers: `get_ai_book_memory`, `save_ai_book_memory`, and `delete_ai_book_memory`.
- `src/api/router.rs` wires raw memory routes: `/reader3/getAiBookMemory`, `/reader3/saveAiBookMemory`, `/reader3/deleteAiBookMemory`.
- `src/service/ai_book_catchup_service.rs` already has background catch-up and cancel support, but it still builds prompts itself and sends full current memory context.

Frontend today:

- `frontend/src/api/aiBook.ts` exposes raw memory APIs: `getAiBookMemory`, `saveAiBookMemory`, `deleteAiBookMemory`.
- `frontend/src/stores/aiBook.ts` owns semantic generation, save, map redraw, and memory update flow.
- `frontend/src/utils/aiBookGeneration.ts` constructs model prompts, calls model/tool APIs, parses model output, and produces memory updates.
- `frontend/src/utils/aiBookV2.ts` owns V2 merge/reconcile logic.
- `frontend/src/types/index.ts` contains display types, V2 internal semantic memory types, and patch types.

This architecture lets frontend/backend semantics drift and lets invalid relationships leak into memory.

---

## 1. Decision

AI Book Memory V3 is a destructive backend-owned rewrite.

Explicit decisions:

- No V1/V2 runtime compatibility.
- No V2-to-V3 semantic migration.
- No legacy browser-side AI Book generation in the default path.
- No frontend prompt construction.
- No frontend model/provider calls for AI Book generation.
- No frontend model response parsing.
- No frontend semantic memory merge.
- No frontend raw memory save.
- No model response that returns full memory.
- Backend owns generation, normalization, merge, lifecycle, validation, persistence, and view-model projection.
- Frontend only triggers actions and renders backend view models.

Cutover decision:

- The local database and current book library are intentionally cleared for this rewrite.
- Do not build V1/V2 backup, migration, adapter, or recovery paths.
- If a non-V3 AI Book row is encountered after cutover, treat it as invalid generated cache and reset/delete it without preserving runtime data.

---

## 2. Goals

1. Relationships page shows only meaningful person-person relationships.
2. Character status, abilities, affiliations, locations, and world settings are not stored as relationships.
3. Frontend AI Book code becomes API/view-model presentation code only.
4. Raw JSON memory save APIs are removed from the default route set.
5. Model output never returns full memory; semantic changes are patch-only.
6. Invalid model candidates cannot persist as relationship cards.
7. Catch-up cost does not grow with full memory size.
8. Catch-up exposes observability: call counts, skipped patch count, latency, input/output sizes, and current stage.
9. Fresh or reset local data starts directly as V3.

## 3. Non-goals

- No graph database.
- No V2 data migration.
- No compatibility adapter for old AI Book memory.
- No browser-side AI Book model generation in the default UI.
- No frontend semantic validation beyond display sorting/search/collapse.
- No multi-writer merge semantics for the same book in the first version.
- Do not allow concurrent writes for the same `user_ns + bookUrl`; reject or serialize them with a per-book guard.
- No server-side model registry in the first version; use the existing backend text/image model config.

---

## 4. Files to Delete, Replace, or Add

Delete from the default path after the new API/view flow is green:

```text
frontend/src/utils/aiBookGeneration.ts
frontend/src/utils/aiBookV2.ts
```

If temporary compile scaffolding is needed, move legacy code under a non-imported `legacy/` folder and delete it before completion.

Replace or heavily rewrite:

```text
src/model/ai_book.rs
src/service/ai_book_service.rs
src/service/ai_book_catchup_service.rs
src/api/handlers/ai_book.rs
src/api/router.rs
src/api/mod.rs
src/app/bootstrap.rs
frontend/src/api/aiBook.ts
frontend/src/types/index.ts AI Book section
frontend/src/stores/aiBook.ts
frontend/src/views/AiBookView.vue AI Book data flow
```

Add:

```text
src/service/ai_book_memory_v3.rs
src/service/ai_book_generation_service.rs
src/model/ai_book_generation.rs
```

Optional split if `ai_book_memory_v3.rs` grows too large:

```text
src/service/ai_book_memory_v3/
  mod.rs
  normalize.rs
  merge.rs
  validate.rs
  display.rs
  working_context.rs
```

---

## 5. Target Architecture

```text
Frontend
  ├─ AiBookView.vue
  ├─ ReaderView.vue chapter AI actions
  └─ frontend/src/api/aiBook.ts
        │
        ▼
Backend API actions
  ├─ get memory view
  ├─ get chapter memory view
  ├─ reset memory
  ├─ set enabled
  ├─ generate chapter memory
  ├─ generate map image
  ├─ start catchup
  ├─ get catchup status
  └─ cancel catchup
        │
        ▼
AI Memory Engine
  ├─ create/reset V3 memory
  ├─ build working context
  ├─ build prompt
  ├─ call model
  ├─ parse KnowledgePatchV3
  ├─ normalize candidates
  ├─ classify relation candidates
  ├─ merge into AiBookMemoryV3
  ├─ validate memory
  ├─ project view models
  └─ save typed V3
```

Frontend may let the user trigger actions. It does not know provider payloads, prompts, patches, raw memory internals, or merge rules.

Backend remains the only executor of text/image model calls and resolves endpoints through existing backend model config.

---

## 6. V3 Data Model

### Root

```ts
interface AiBookMemoryV3 {
  schemaVersion: 3
  bookUrl: string
  bookName?: string
  author?: string
  enabled: boolean

  processedChapterIndex?: number
  processedChapterTitle?: string
  updatedAt: number

  summary: AiBookSummaryV3
  chapterDigests: AiBookChapterDigestV3[]

  characters: AiBookCharacterV3[]
  characterStates: AiBookCharacterStateV3[]
  characterRelations: AiBookCharacterRelationV3[]

  knowledgeFacts: AiBookKnowledgeFactV3[]

  locations: AiBookLocationV3[]
  locationEdges: AiBookLocationEdgeV3[]

  mapState: AiBookMapStateV3
  renderArtifacts: AiBookRenderArtifactsV3

  droppedFacts: AiBookDroppedFactV3[]
  catchupStats?: AiBookCatchupStatsV3

  lastError?: string
  lastErrorChapterIndex?: number
  lastErrorChapterTitle?: string
}
```

### Summary

```ts
interface AiBookSummaryV3 {
  current: string
  recentChanges: string[]
  openQuestions: string[]
}
```

### Evidence

```ts
interface AiBookEvidenceV3 {
  chapterIndex: number
  chapterTitle: string
  quote?: string
  note: string
}
```

Evidence is mandatory for persisted semantic entities produced by the model.

### Chapter Digest

```ts
interface AiBookChapterDigestV3 {
  chapterIndex: number
  chapterTitle: string
  digest: string
  keyEvents: string[]
  mentionedCharacterNames: string[]
  mentionedLocationNames: string[]
  mentionedConceptNames: string[]
  hasImportantChanges: boolean
  touchedEntityIds: string[]
  createdAt: number
}
```

### Character

```ts
interface AiBookCharacterV3 {
  id: string
  name: string
  aliases: string[]
  importance: 'major' | 'moderate' | 'minor'
  description?: string
  firstSeenChapterIndex?: number
  lastSeenChapterIndex?: number
  evidence: AiBookEvidenceV3[]
}
```

### Character State

```ts
interface AiBookCharacterStateV3 {
  characterId: string
  currentStatus?: string
  currentLocationId?: string
  affiliations: string[]
  abilities: Array<{
    name: string
    level?: string
    status?: string
    knowledgeFactId?: string
    evidence: AiBookEvidenceV3[]
  }>
  resources: string[]
  lastSeenChapterIndex?: number
  evidence: AiBookEvidenceV3[]
}
```

Caps:

```text
MAX_ABILITIES_PER_CHARACTER = 30
MAX_RESOURCES_PER_CHARACTER = 30
MAX_AFFILIATIONS_PER_CHARACTER = 20
MAX_STATE_EVIDENCE = 8
```

Examples:

| Input semantics | V3 target |
| --- | --- |
| 张羽修炼健体三十六式 | `characterStates.abilities` + optional `knowledgeFacts` |
| 张羽就读嵩阳高中 | `characterStates.affiliations` |
| 张羽在嵩阳高中 | `characterStates.currentLocationId` |
| 张羽获得补助资格 | `characterStates.resources` or `currentStatus` |

---

## 7. Relationship Semantics

`characterRelations` is a narrow table. It only contains important person-person relationships.

It is not a generic graph edge table.

### Allowed kinds

```ts
type AiBookRelationKindV3 =
  | 'family'
  | 'romantic'
  | 'friend'
  | 'ally'
  | 'enemy'
  | 'rival'
  | 'mentor_student'
  | 'peer'
  | 'superior_subordinate'
  | 'benefactor_dependent'
  | 'unknown_significant'
```

`organization_member` is intentionally excluded. Organization, school, class, clan, company, sect, and faction membership belongs to `characterStates.affiliations`.

Only sustained story-relevant person-person interactions become `peer`, `ally`, `enemy`, `superior_subordinate`, etc.

### Relation fields

```ts
interface AiBookCharacterRelationV3 {
  id: string

  sourceCharacterId: string
  targetCharacterId: string

  kind: AiBookRelationKindV3
  subtype?: string
  label: string

  polarity: 'positive' | 'negative' | 'mixed' | 'neutral' | 'unknown'
  strength: 'major' | 'moderate' | 'minor'
  status: 'active' | 'changed' | 'ended' | 'uncertain'
  direction: 'directed' | 'undirected'

  summary: string
  currentDynamics: string[]

  facets?: Array<{
    kind: AiBookRelationKindV3
    subtype?: string
    polarity: 'positive' | 'negative' | 'mixed' | 'neutral' | 'unknown'
    status: 'active' | 'changed' | 'ended' | 'uncertain'
    summary: string
  }>

  firstSeenChapterIndex?: number
  lastUpdatedChapterIndex?: number

  evidence: AiBookEvidenceV3[]
  history: AiBookRelationChangeV3[]
}
```

First UI version displays `label`, `polarity`, `summary`, and `currentDynamics`; `facets` can stay collapsed or hidden.

### Relation history

```ts
interface AiBookRelationChangeV3 {
  chapterIndex: number
  chapterTitle: string
  previousKind?: AiBookRelationKindV3
  nextKind: AiBookRelationKindV3
  previousPolarity?: AiBookCharacterRelationV3['polarity']
  nextPolarity: AiBookCharacterRelationV3['polarity']
  previousStatus?: AiBookCharacterRelationV3['status']
  nextStatus: AiBookCharacterRelationV3['status']
  note: string
  evidence: AiBookEvidenceV3[]
}
```

### Forbidden in `characterRelations`

Never persist these as relationship cards:

- Person-location: 身处, 位于, 就读, 住在, 进入, 出现在.
- Person-skill/concept: 修炼, 学习, 掌握, 使用.
- Person-item: 拥有, 拿着, 购买.
- Location-location: 包含, 相邻, 路线.
- Transient actions: 看到, 听说, 路过, 站在, 说话, 出现在大屏幕.
- Low-value co-occurrence: 单纯认识, 同校, 同村, 同屏出现, 路人互动.

### Classification examples

| Input semantics | V3 target |
| --- | --- |
| 张羽和白真真有借贷互助 | `characterRelations(kind=peer, subtype=借贷牵连, polarity=mixed)` |
| 苏海峰以老师身份诱导张羽签债务合同 | `characterRelations(kind=superior_subordinate, subtype=债务诱导, polarity=negative)` |
| 赵天行怀疑并跟踪张羽 | Usually `characterRelations(kind=peer, subtype=怀疑跟踪, polarity=negative or mixed)` if sustained; only `enemy`/`rival` when the text confirms hostility or competition. |
| 张羽就读嵩阳高中 | `characterStates.affiliations` |
| 张羽修炼健体三十六式 | `characterStates.abilities` |
| 赵天行出现在学校大屏幕 | Usually `droppedFacts`; if sustained, `characterStates.currentStatus` |
| 嵩阳高中包含法力教室 | `locationEdges(kind=contains)` |

---

## 8. Knowledge Facts

```ts
interface AiBookKnowledgeFactV3 {
  id: string
  category:
    | '基础规则'
    | '势力制度'
    | '历史传说'
    | '技术/魔法'
    | '社会文化'
    | '地理环境'
    | '组织体系'
    | '资源经济'
    | '未确认信息'
  title: string
  content: string
  confidence: '已知' | '推断' | '未知'
  importance: 'major' | 'moderate'
  firstSeenChapterIndex?: number
  lastConfirmedChapterIndex?: number
  evidence: AiBookEvidenceV3[]
}
```

`knowledgeFacts` are reusable setting facts, not chapter summaries.

For skills/cultivation:

- `knowledgeFacts` stores what the skill/rule is.
- `characterStates.abilities` stores a character's mastery/progress.

---

## 9. Locations and Map

```ts
interface AiBookLocationV3 {
  id: string
  name: string
  aliases: string[]
  kind: string
  scale:
    | 'world'
    | 'continent'
    | 'country'
    | 'region'
    | 'city'
    | 'district'
    | 'site'
    | 'building'
    | 'room'
    | 'unknown'
  parentLocationId?: string
  description: string
  currentStatus?: string
  importance: 'major' | 'moderate'
  firstSeenChapterIndex?: number
  lastSeenChapterIndex?: number
  evidence: AiBookEvidenceV3[]
}

interface AiBookLocationEdgeV3 {
  id: string
  sourceLocationId: string
  targetLocationId: string
  kind: 'contains' | 'route' | 'adjacent'
  label?: string
  evidence: AiBookEvidenceV3[]
}
```

`contains` edges use `source=parent`, `target=child`. `parentLocationId` is stored as a display shortcut derived from or validated against `contains` edges.

Map state:

```ts
interface AiBookMapStateV3 {
  dirty: boolean
  reason?: string
  sourceChapterIndex?: number
  lastRenderedAt?: number
  mapPrompt?: string
}

interface AiBookRenderArtifactsV3 {
  mapImageUrl?: string
  mapImagePrompt?: string
  mapFallbackReason?: string
}
```

Do not include `character-movement` as a map edge. Character movement belongs to character state or chapter digest.

---

## 10. Dropped Facts

`droppedFacts` only records rejected or redirected V3 model candidates.

It does not record V1/V2 reset data.

```ts
interface AiBookDroppedFactV3 {
  source: 'model_patch'
  reason:
    | 'non_character_relation'
    | 'person_location_relation'
    | 'person_ability_relation'
    | 'person_item_relation'
    | 'location_edge_relation'
    | 'transient_action'
    | 'low_value_relation'
    | 'missing_evidence'
    | 'invalid_entity'
    | 'invalid_relation_kind'
  originalValuePreview: string
  originalValueHash: string
  redirectedTo?: 'character_state' | 'knowledge_fact' | 'location_edge'
  chapterIndex?: number
  createdAt: number
}
```

Limit:

```text
MAX_DROPPED_FACTS = 200
```

Keep counts in stats instead of keeping unbounded raw records.

---

## 11. Catchup Stats and Task Status

```ts
interface AiBookCatchupStatsV3 {
  totalModelCalls: number
  digestCalls: number
  patchCalls: number
  skippedPatchChapters: number

  totalInputBytes: number
  totalOutputBytes: number

  lastCallLatencyMs?: number
  averageCallLatencyMs?: number

  lastChapterIndex?: number
  updatedAt: number
}
```

Task status:

```ts
interface AiBookCatchupTaskViewV3 {
  bookUrl: string
  status: 'idle' | 'running' | 'canceling' | 'canceled' | 'completed' | 'failed'
  currentStage?: 'idle' | 'fetching' | 'digest' | 'patch' | 'saving'
  startChapterIndex?: number
  targetChapterIndex?: number
  totalChapters: number
  completedChapters: number
  currentChapterIndex?: number
  currentChapterTitle?: string
  processedChapterIndex?: number
  processedChapterTitle?: string
  error?: string
  stats?: AiBookCatchupStatsV3
  updatedAt: number
}
```

---

## 12. Backend API

All responses still use the existing `ApiResponse::ok(data)` wrapper. Frontend axios unwraps `data.data`, so TypeScript API helpers should type the unwrapped payload.

Remove raw memory save.

### Get memory view

```http
GET /reader3/aiBook/memory?bookUrl=...
```

Returns unwrapped payload:

```ts
interface AiBookMemoryViewResponse {
  memory: AiBookMemoryViewModel
}
```

If a row exists but is not valid V3, backend resets/deletes it without migration or backup.

### Get chapter memory view

```http
GET /reader3/aiBook/chapterMemory?bookUrl=...&chapterIndex=...
```

Returns cached chapter-scoped AI data without calling the model:

```ts
interface AiBookChapterMemoryViewResponse {
  chapter: AiBookChapterMemoryViewModel
  memory: AiBookMemoryViewModel
}
```

### Reset memory

```http
POST /reader3/aiBook/memory/reset
Body: { bookUrl: string }
```

Deletes or overwrites with empty V3.

### Enable/disable

```http
POST /reader3/aiBook/enabled
Body: { bookUrl: string, enabled: boolean }
```

Frontend must not save full memory just to toggle enabled state.

### Generate current chapter memory

```http
POST /reader3/aiBook/chapterMemory/generate
Body: {
  bookUrl: string
  chapterIndex: number
  mode: 'cached' | 'refresh'
}
```

Backend loads book, chapter content, current memory, builds context, calls model, normalizes, merges, saves, and returns the same view payload as `GET /chapterMemory`.

First version uses existing backend text model config. Do not add `modelId` until a server-side model registry exists.

### Generate map image

```http
POST /reader3/aiBook/map/generate
Body: {
  bookUrl: string
  sourceChapterIndex?: number
  prompt?: string
}
```

Backend generates/regenerates the map image with existing backend image model config, stores the artifact, updates V3 map fields, and returns the same memory view model.

### Catchup

```http
POST /reader3/aiBook/catchup/start
Body: {
  bookUrl: string
  targetChapterIndex?: number
}
```

```http
GET /reader3/aiBook/catchup/status?bookUrl=...
```

```http
POST /reader3/aiBook/catchup/cancel
Body: { bookUrl: string }
```

`cancel` replaces old `pause`. It stops after the current in-flight model request returns. It does not need to abort HTTP mid-flight in the first version.

Task status transitions through `canceling` before `canceled`. The task remains queryable until the loop observes cancel and persists final status.

---

## 13. Backend Responsibilities

### `src/model/ai_book.rs`

Replace old structs with V3 typed structs and view model structs.

Do not keep V1/V2 structs in this file.

Use Rust enums for relation kind, polarity, strength, status, location edge kind, fact confidence, and fact category.

### `src/service/ai_book_service.rs`

Replace raw JSON service with typed V3 service.

Public methods:

```rust
pub async fn get_or_create_v3(
    &self,
    user_ns: &str,
    book_url: &str,
    book_name: Option<&str>,
    author: Option<&str>,
) -> Result<AiBookMemoryV3, AppError>

pub async fn save_v3(
    &self,
    user_ns: &str,
    book_url: &str,
    memory: AiBookMemoryV3,
) -> Result<AiBookMemoryV3, AppError>

pub async fn reset_v3(
    &self,
    user_ns: &str,
    book_url: &str,
    book_name: Option<&str>,
    author: Option<&str>,
) -> Result<AiBookMemoryV3, AppError>

pub async fn set_enabled(
    &self,
    user_ns: &str,
    book_url: &str,
    enabled: bool,
) -> Result<AiBookMemoryV3, AppError>
```

Behavior:

- If no row exists, create empty V3.
- If row exists and `schemaVersion != 3`, overwrite/delete it and return empty V3.
- If row exists and V3 parse/validation fails, overwrite/delete it and return empty V3.
- Always validate before saving.
- Never expose raw JSON save in the default API path.
- `get_or_create_v3` always returns a renderable V3 object for normal page loads.
- Do not write backup files for old AI Book memory.

### `src/service/ai_book_memory_v3.rs`

Owns pure semantic logic.

Functions:

```rust
pub fn create_empty_ai_book_memory_v3(...) -> AiBookMemoryV3

pub fn validate_ai_book_memory_v3(memory: &AiBookMemoryV3) -> Result<(), AppError>

pub fn normalize_knowledge_patch_v3(
    patch: KnowledgePatchV3,
    context: &AiBookWorkingContextV3,
) -> NormalizedKnowledgePatchV3

pub fn classify_relation_candidate_v3(
    candidate: RelationCandidateV3,
    context: &AiBookWorkingContextV3,
) -> RelationClassificationV3

pub fn merge_ai_book_memory_v3(
    previous: AiBookMemoryV3,
    patch: NormalizedKnowledgePatchV3,
) -> AiBookMemoryV3

pub fn select_ai_book_display_memory_v3(
    memory: &AiBookMemoryV3,
) -> AiBookMemoryViewModel

pub fn select_ai_book_chapter_view_v3(
    memory: &AiBookMemoryV3,
    chapter_index: i32,
) -> AiBookChapterMemoryViewModel

pub fn select_working_context_v3(
    memory: &AiBookMemoryV3,
    chapter_digest: Option<&AiBookChapterDigestV3>,
    chapter_text: &str,
) -> AiBookWorkingContextV3
```

### `src/service/ai_book_generation_service.rs`

Owns model calls.

Responsibilities:

- Resolve final model endpoint from existing backend model config.
- Build digest prompt.
- Build patch prompt.
- Build map-image prompt and image-generation request.
- Call configured backend text/image model.
- Parse model response into internal structs.
- Never accept full memory from model.
- Never let model bypass normalizer.
- Return updated V3 memory and view model.

### `src/service/ai_book_catchup_service.rs`

Rewrite around generation service.

Flow:

```text
for chapter in target range:
  content = fetch chapter content
  digest = generate_digest(chapter)
  save digest
  if digest.hasImportantChanges or backend guard requires patch:
    working_context = select_working_context_v3(memory, digest, content)
    patch = generate_patch(digest, content snippets, working_context)
    normalized = normalize_knowledge_patch_v3(patch, context)
    memory = merge_ai_book_memory_v3(memory, normalized)
  else:
    stats.skippedPatchChapters += 1
  save memory
  update task status
```

No full memory in prompt. No full memory returned by model.

`AiBookCatchupService` should receive `AiBookGenerationService` through constructor injection in `bootstrap.rs`.

### `src/api/handlers/ai_book.rs`

Replace handlers with action handlers:

```rust
get_ai_book_memory_view
get_ai_book_chapter_memory_view
reset_ai_book_memory
set_ai_book_enabled
generate_ai_book_chapter_memory
generate_ai_book_map
start_ai_book_catchup
get_ai_book_catchup_status
cancel_ai_book_catchup
```

### `src/api/router.rs`

Remove old routes and pause route.

Add:

```rust
.route("/reader3/aiBook/memory", get(...))
.route("/reader3/aiBook/chapterMemory", get(...))
.route("/reader3/aiBook/memory/reset", post(...))
.route("/reader3/aiBook/enabled", post(...))
.route("/reader3/aiBook/chapterMemory/generate", post(...))
.route("/reader3/aiBook/map/generate", post(...))
.route("/reader3/aiBook/catchup/start", post(...))
.route("/reader3/aiBook/catchup/status", get(...))
.route("/reader3/aiBook/catchup/cancel", post(...))
```

---

## 14. Frontend Responsibilities

### `frontend/src/api/aiBook.ts`

Replace raw memory API with action API.

```ts
export function getAiBookMemory(bookUrl: string) {
  return http.get<AiBookMemoryViewResponse>('/aiBook/memory', { params: { bookUrl } }).then(r => r.data)
}

export function getAiBookChapterMemory(bookUrl: string, chapterIndex: number) {
  return http.get<AiBookChapterMemoryViewResponse>('/aiBook/chapterMemory', { params: { bookUrl, chapterIndex } }).then(r => r.data)
}

export function resetAiBookMemory(bookUrl: string) {
  return http.post<AiBookMemoryViewResponse>('/aiBook/memory/reset', { bookUrl }).then(r => r.data)
}

export function setAiBookEnabled(bookUrl: string, enabled: boolean) {
  return http.post<AiBookMemoryViewResponse>('/aiBook/enabled', { bookUrl, enabled }).then(r => r.data)
}

export function generateAiBookChapterMemory(params: {
  bookUrl: string
  chapterIndex: number
  mode: 'cached' | 'refresh'
}) {
  return http.post<AiBookChapterMemoryViewResponse>('/aiBook/chapterMemory/generate', params).then(r => r.data)
}

export function generateAiBookMap(params: {
  bookUrl: string
  sourceChapterIndex?: number
  prompt?: string
}) {
  return http.post<AiBookMemoryViewResponse>('/aiBook/map/generate', params).then(r => r.data)
}

export function startAiBookCatchup(params: {
  bookUrl: string
  targetChapterIndex?: number
}) {
  return http.post<AiBookCatchupStatus>('/aiBook/catchup/start', params).then(r => r.data)
}

export function getAiBookCatchupStatus(bookUrl: string) {
  return http.get<AiBookCatchupStatus>('/aiBook/catchup/status', { params: { bookUrl } }).then(r => r.data)
}

export function cancelAiBookCatchup(bookUrl: string) {
  return http.post<AiBookCatchupStatus>('/aiBook/catchup/cancel', { bookUrl }).then(r => r.data)
}
```

### `frontend/src/types/index.ts`

Keep only display and API types for the default AI Book path.

Remove default-path dependencies on:

```ts
AiBookAnyMemory
AiBookMemoryV2
AiBookRelationshipV2
AiBookChapterKnowledgePatch
AiBookPatchWorldFact
AiBookPatchCharacter
AiBookPatchRelationship
AiBookPatchLocation
AiBookModelUpdate
```

Add:

```ts
AiBookMemoryViewModel
AiBookChapterMemoryViewModel
AiBookCharacterView
AiBookCharacterStateView
AiBookRelationView
AiBookKnowledgeFactView
AiBookLocationView
AiBookMapView
AiBookCatchupStats
AiBookCatchupStatus
AiBookMemoryViewResponse
AiBookChapterMemoryViewResponse
```

### `frontend/src/stores/aiBook.ts`

Rewrite as a thin action/view-model store.

Allowed store responsibilities:

- load memory view
- load chapter memory view
- call generate/reset/enabled/map/catchup/cancel APIs
- hold loading/error/phase/catchup polling state

Forbidden store responsibilities:

- prompt construction
- provider calls
- model output parsing
- semantic merge
- raw memory save
- relation validity decisions

### `frontend/src/views/AiBookView.vue`

Allowed UI state:

- active tab
- search text
- collapsed groups
- selected graph node
- loading/error/toast
- catchup polling state

Forbidden:

- building prompts
- calling model directly
- calling provider/image endpoints directly
- parsing model output
- merging semantic memory
- deciding whether a relation is semantically valid
- saving raw memory

Frontend may keep user action controls; it must not hold provider credentials or provider-specific request logic for AI Book generation.

---

## 15. View Models

Backend returns display models.

```ts
interface AiBookMemoryViewModel {
  bookUrl: string
  bookName?: string
  author?: string
  enabled: boolean
  processedChapterIndex?: number
  processedChapterTitle?: string
  updatedAt: number

  summary: {
    current: string
    recentChanges: string[]
    openQuestions: string[]
  }

  characters: AiBookCharacterView[]
  relationships: AiBookRelationView[]
  knowledgeFacts: AiBookKnowledgeFactView[]
  locations: AiBookLocationView[]
  map: AiBookMapView | null

  cleanup: {
    droppedFactsCount: number
    droppedByReason: Record<string, number>
    oldSchemaBackedUp: boolean
  }

  catchupStats?: AiBookCatchupStatsV3

  lastError?: string
  lastErrorChapterIndex?: number
  lastErrorChapterTitle?: string
}

interface AiBookChapterMemoryViewModel {
  bookUrl: string
  chapterIndex: number
  chapterTitle?: string
  digest?: AiBookChapterDigestV3
  characters: AiBookCharacterView[]
  relationships: AiBookRelationView[]
  knowledgeFacts: AiBookKnowledgeFactView[]
  locations: AiBookLocationView[]
  generationStatus: 'missing' | 'cached' | 'running' | 'failed'
  lastError?: string
}
```

Relationship view reads only `characterRelations`.

Character view combines `characters + characterStates`.

Facts view reads `knowledgeFacts`.

Map view combines `locations + locationEdges + renderArtifacts`.

---

## 16. Internal Generation Contract

The model produces `KnowledgePatchV3`, not memory.

```ts
interface KnowledgePatchV3 {
  chapterDigest: {
    chapterIndex: number
    chapterTitle: string
    digest: string
    keyEvents: string[]
    mentionedCharacterNames: string[]
    mentionedLocationNames: string[]
    mentionedConceptNames: string[]
    hasImportantChanges: boolean
  }

  summary?: Partial<AiBookSummaryV3>

  characters?: Partial<AiBookCharacterV3>[]
  characterStates?: Partial<AiBookCharacterStateV3>[]
  characterRelations?: Partial<AiBookCharacterRelationV3>[]

  knowledgeFacts?: Partial<AiBookKnowledgeFactV3>[]

  locations?: Partial<AiBookLocationV3>[]
  locationEdges?: Partial<AiBookLocationEdgeV3>[]
}
```

Rules:

- Model may submit candidates.
- Backend owns normalization.
- Backend can redirect candidates.
- Backend can drop candidates.
- Backend owns stable IDs.
- Backend owns merge semantics.
- Backend owns lifecycle history.

---

## 17. Working Context

Do not pass full memory.

```ts
interface AiBookWorkingContextV3 {
  bookName?: string
  author?: string

  summaryCurrent: string
  recentChapterDigests: Array<{
    chapterIndex: number
    chapterTitle: string
    digest: string
    keyEvents: string[]
  }>

  relevantCharacters: Array<{
    id: string
    name: string
    aliases: string[]
    status?: string
    affiliations: string[]
    abilities: string[]
  }>

  relevantRelations: Array<{
    sourceCharacterId: string
    sourceName: string
    targetCharacterId: string
    targetName: string
    kind: string
    subtype?: string
    label: string
    polarity: string
    status: string
  }>

  relevantKnowledgeFacts: Array<{
    id: string
    title: string
    category: string
    content: string
  }>

  relevantLocations: Array<{
    id: string
    name: string
    aliases: string[]
    parentName?: string
    kind: string
    scale: string
  }>

  schemaHint: 'KnowledgePatchV3'
}
```

Selection rules:

- Last 8 chapter digests.
- Characters mentioned in current chapter/digest.
- Locations mentioned in current chapter/digest.
- High-importance facts first.
- Existing relations among mentioned characters.
- Hard caps per section.

Recommended caps:

```text
recentChapterDigests: 8
relevantCharacters: 20
relevantRelations: 12
relevantKnowledgeFacts: 12
relevantLocations: 15
```

Text snippet selection rules for patch stage:

```text
digest stage: first 8000 chars of chapter text
patch stage: entity-neighbor snippets around mentioned characters/locations/concepts
fallback: first 8000 chars if no entity snippet is found
always include a small head/tail snippet to preserve scene transitions
```

First implementation can use simple substring windows: head 1500 chars, tail 1000 chars, plus ±800 chars around mentioned names, deduped and capped.

---

## 18. Prompt Requirements

### Digest stage

Input:

- book name
- author
- chapter title
- chapter index
- trimmed chapter text

Output:

```json
{
  "chapterDigest": {
    "chapterIndex": 0,
    "chapterTitle": "...",
    "digest": "...",
    "keyEvents": [],
    "mentionedCharacterNames": [],
    "mentionedLocationNames": [],
    "mentionedConceptNames": [],
    "hasImportantChanges": true
  }
}
```

Digest stage max output tokens: `1024`.

### Patch stage

Input:

- chapter digest
- selected text snippets
- working context
- schema instructions

Output:

```json
{
  "chapterDigest": { "...": "..." },
  "summary": { "...": "..." },
  "characters": [],
  "characterStates": [],
  "characterRelations": [],
  "knowledgeFacts": [],
  "locations": [],
  "locationEdges": []
}
```

Patch stage max output tokens: `2048-3072`.

Strict prompt rules:

- `characterRelations` only contains important person-person relationships.
- Do not create a relationship merely because one person suspects, sees, mentions, follows, owes, helps, or appears with another once.
- Put such details into `currentDynamics` only when a sustained relationship already exists or is clearly formed.
- Do not output person-location, person-skill, person-item, location-location, transient actions, or low-value co-occurrence as relations.
- Skills and cultivation go to `characterStates.abilities`.
- School/work/sect/faction membership goes to `characterStates.affiliations`.
- World rules go to `knowledgeFacts`.
- Location containment/routes go to `locationEdges`.
- If no important changes, output digest only and empty patch arrays.
- Maximum per chapter:
  - relations: 3
  - character states: 5
  - knowledge facts: 5
  - locations: 5
  - location edges: 3

---

## 19. Normalization Pipeline

### Step 1: parse

```text
raw model response → KnowledgePatchV3 candidate
```

Reject invalid JSON.

### Step 2: field normalization

- Normalize aliases.
- Trim empty strings.
- Normalize relation kind.
- Normalize polarity/strength/status.
- Normalize evidence.
- Generate stable IDs.
- Drop low-value or empty candidates.

### Step 3: relation classification

Input: relation candidate.

Output:

```ts
type RelationClassificationV3 =
  | { kind: 'keep_relation'; relation: AiBookCharacterRelationV3 }
  | { kind: 'redirect_character_state'; statePatch: Partial<AiBookCharacterStateV3> }
  | { kind: 'redirect_knowledge_fact'; fact: Partial<AiBookKnowledgeFactV3> }
  | { kind: 'redirect_location_edge'; edge: Partial<AiBookLocationEdgeV3> }
  | { kind: 'drop'; droppedFact: AiBookDroppedFactV3 }
```

Rules:

- Source and target must both resolve to characters to keep relation.
- Relation kind must be an allowed enum.
- Evidence must exist.
- Relation must be story-relevant.
- `strength=minor` relations are hidden from default view and may be dropped unless they already have history.
- If both characters are minor and there is only one evidence entry, default to drop or minor.
- Transient actions are dropped.
- Skill/concept relations redirect to character state or fact.
- Location relations redirect to state or location edge.

### Step 4: merge

- Characters upsert by stable character ID.
- Character states merge into existing state by `characterId`.
- Abilities merge by normalized ability name.
- Facts upsert by stable title/category ID.
- Locations upsert by stable location ID.
- Location edges upsert by stable source-target-kind.
- `contains` edges always use `source=parent`, `target=child`.
- Directed relations preserve direction in storage IDs.
- Undirected relations may use normalized pair IDs.
- View aggregation may merge multiple stored relation facets into one displayed person-pair card.
- Relationship changes append to `history`.
- `status=ended` relations remain in memory but default view hides or weakens them.
- Missing from one chapter does not delete an entity.
- Evidence is deduplicated and capped.

Digest skip guard:

- If digest says `hasImportantChanges=false` but backend detects new named entities, relation-heavy keywords, affiliation changes, ability progression, debt/conflict/rescue patterns, or location moves, backend still runs patch stage.

Recommended caps:

```text
MAX_EVIDENCE_PER_ENTITY = 8
MAX_RELATION_HISTORY = 20
MAX_CHAPTER_DIGESTS = 300
MAX_CHARACTERS = 300
MAX_RELATIONS = 300
MAX_FACTS = 300
MAX_LOCATIONS = 300
MAX_LOCATION_EDGES = 500
MAX_DROPPED_FACTS = 200
```

---

## 20. Data Reset Strategy

Because the local DB and book library are cleared for this rewrite:

- Do not run semantic migration.
- Do not preserve old relationships in runtime memory.
- Do not attempt to clean old schema.
- Do not show old schema in UI.
- Do not write old-schema backup files.
- If invalid/non-V3 memory is found after cutover, treat it as corrupt generated cache and reset/delete it.

Implementation:

```rust
match read_schema_version(&value) {
    Some(3) => parse_v3(value).unwrap_or_else(|_| reset_v3(...)),
    _ => reset_v3(...),
}
```

---

## 21. Testing Strategy

### Rust tests

```text
cargo test ai_book
```

Required tests:

```text
ai_book_v3_empty_memory_is_valid
ai_book_v3_rejects_non_v3_save
ai_book_v3_backs_up_old_schema_before_reset
ai_book_v3_resets_old_schema_on_read
ai_book_v3_resets_invalid_v3_on_read
ai_book_v3_rejects_invalid_relation_kind
ai_book_v3_rejects_relation_without_evidence
ai_book_v3_drops_person_location_relation
ai_book_v3_redirects_person_ability_to_state
ai_book_v3_redirects_location_contains_to_location_edge
ai_book_v3_preserves_directed_relation_storage
ai_book_v3_view_can_group_same_character_pair
ai_book_v3_relation_change_appends_history
ai_book_v3_ended_relation_hidden_from_default_view
ai_book_v3_working_context_has_hard_caps
ai_book_v3_digest_only_skips_patch
ai_book_v3_digest_guard_forces_patch_when_needed
ai_book_v3_catchup_stats_increment
ai_book_v3_cancel_moves_to_canceling_then_canceled
ai_book_v3_get_memory_always_returns_renderable_view
ai_book_v3_get_memory_wraps_api_response
ai_book_v3_no_raw_save_route_registered
ai_book_v3_map_generate_updates_artifact
```

### Frontend tests

```text
cd frontend && npm test -- aiBook
```

Required tests:

```text
aiBook_api_has_no_save_raw_memory
aiBook_view_renders_relationships_from_view_model
aiBook_view_renders_character_state_from_view_model
aiBook_view_calls_generate_action
aiBook_view_calls_map_generate_action
aiBook_view_calls_reset_action
aiBook_view_calls_cancel_catchup
aiBook_frontend_has_no_aiBookGeneration_imports_in_default_path
```

### Fixture assertions

Use a small fixture from 《没钱修什么仙》:

- 张羽修炼健体三十六式。
- 张羽就读嵩阳高中。
- 赵天行怀疑/跟踪张羽。
- 白真真与张羽存在借贷互助。
- 苏海峰以老师身份诱导张羽签债务合同。
- 学校大屏幕出现人物名字。

Assertions:

- “张羽 修炼 健体三十六式” does not appear in relationships.
- “健体三十六式” appears in ability or knowledge facts.
- “张羽 就读 嵩阳高中” does not appear in relationships.
- “嵩阳高中” appears as affiliation/location.
- “张羽 白真真 借贷/互助” appears as relationship.
- “苏海峰 张羽 诱导/债务/师生权力” appears as relationship.
- “赵天行 学校大屏幕” is dropped or state-only.
- Long memory does not increase working context beyond caps.
- Digest-only chapter does not call patch stage.

### Sanity

```text
cargo test ai_book
cd frontend && npm test -- aiBook
git diff --check
```

---

## 22. Implementation Order

Use a staged cutover, not a one-shot delete.

1. Add V3 model/view types in `src/model/ai_book.rs`.
2. Add pure V3 helper module and tests in `src/service/ai_book_memory_v3.rs`.
3. Replace `AiBookService` with typed V3 get/save/reset/enabled behavior.
4. Add display/chapter view model projection.
5. Add new memory/chapter/reset/enabled backend endpoints while old routes still exist temporarily.
6. Add `frontend/src/api/aiBook.ts` action APIs for new endpoints.
7. Rewrite `frontend/src/stores/aiBook.ts` into a thin view-model/action store.
8. Update `AiBookView.vue` page load and actions to use backend view models only.
9. Add generation service with patch-only text contract.
10. Wire current chapter generation endpoint.
11. Wire backend map generation endpoint.
12. Rewrite reader auto-update trigger to send book/chapter identity only; backend fetches chapter text.
13. Rewrite catchup service around digest/patch/working-context and canceling/canceled state.
14. Add catchup stats to memory and task view.
15. Remove old raw memory routes from router.
16. Remove default-path frontend imports of `aiBookGeneration.ts` and `aiBookV2.ts`.
17. Delete or quarantine old frontend semantic files after build is green.
18. Add fixture tests.
19. Run sanity commands.

---

## 23. Success Criteria

- Fresh/reset local data uses only V3 AI Book memory.
- Non-V3 AI Book memory is not backed up, migrated, or displayed.
- No frontend default-path code constructs prompts for AI Book memory.
- No frontend default-path code calls text/image provider APIs for AI Book generation.
- No frontend code merges AI Book semantic memory.
- No frontend API can save raw memory.
- No raw memory save route is registered.
- Model output never returns full memory; semantic changes are patch-only.
- Relationships page only reads backend-projected relationship view data from `characterRelations`.
- Skills, locations, affiliations, school membership, items, and transient appearances never show as relationship cards.
- Catch-up does not send full memory per chapter.
- Catch-up status exposes current stage, model calls, skipped patch count, average latency, and current chapter.
- AI Book map image generation runs through backend API and persists artifacts in V3.
- Focused Rust and frontend tests pass.
