# AI Book Memory V3 Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mixed frontend/backend AI Book pipeline with a backend-owned V3 memory engine and thin frontend view-model rendering.

**Architecture:** Backend owns typed memory, normalization, merge, validation, generation, catchup, and projection. Frontend only calls action APIs and renders backend view models. Cutover is destructive: local DB/book data is cleared, so there is no V1/V2 migration, backup, adapter, or recovery path.

**Tech Stack:** Rust (`axum`, `serde`, `sqlx`, `tokio`), Vue 3, TypeScript, Pinia, Vitest, existing Reader book/local-txt services.

---

## File Map

- Backend models: `/Users/maple/Documents/reader/src/model/ai_book.rs`, `/Users/maple/Documents/reader/src/model/ai_book_generation.rs`, `/Users/maple/Documents/reader/src/model/ai_book_catchup.rs`
- Backend helpers/services: `/Users/maple/Documents/reader/src/service/ai_book_memory_v3.rs`, `/Users/maple/Documents/reader/src/service/ai_book_service.rs`, `/Users/maple/Documents/reader/src/service/ai_book_generation_service.rs`, `/Users/maple/Documents/reader/src/service/ai_book_catchup_service.rs`
- Backend API/bootstrap: `/Users/maple/Documents/reader/src/api/handlers/ai_book.rs`, `/Users/maple/Documents/reader/src/api/router.rs`, `/Users/maple/Documents/reader/src/api/mod.rs`, `/Users/maple/Documents/reader/src/app/bootstrap.rs`
- Frontend: `/Users/maple/Documents/reader/frontend/src/types/index.ts`, `/Users/maple/Documents/reader/frontend/src/api/aiBook.ts`, `/Users/maple/Documents/reader/frontend/src/stores/aiBook.ts`, `/Users/maple/Documents/reader/frontend/src/stores/reader.ts`, `/Users/maple/Documents/reader/frontend/src/views/AiBookView.vue`

---

### Task 1: Define V3 backend models and catchup contracts

**Files:**
- Modify: `/Users/maple/Documents/reader/src/model/ai_book.rs`
- Modify: `/Users/maple/Documents/reader/src/model/ai_book_catchup.rs`
- Create: `/Users/maple/Documents/reader/src/model/ai_book_generation.rs`
- Modify: `/Users/maple/Documents/reader/src/model/mod.rs`

- [x] Add V3 memory, view-model, generation DTO, stats, and catchup contract structs.
- [x] Keep serde camelCase and schemaVersion=3.
- [x] Tests: `cargo test ai_book_v3_empty_memory_is_valid`, `cargo test ai_book_v3_catchup_status_serializes_new_states`, `cargo test ai_book`, `git diff --check`.

### Task 2: Build pure V3 memory helpers and projections

**Files:**
- Create: `/Users/maple/Documents/reader/src/service/ai_book_memory_v3.rs`
- Modify: `/Users/maple/Documents/reader/src/service/mod.rs`

- [x] Add validation, stable IDs, alias merge, relation classify/redirect/drop, merge, display/chapter projection, and capped working context.
- [x] Ensure evidence is required for persisted model semantic entities.
- [x] Ensure working context relation names come from character IDs, not relation labels.
- [x] Tests: `cargo test --lib ai_book_v3`, `cargo test --lib`, `git diff --check`.

### Task 3: Rewrite AiBookService as typed V3 persistence

**Files:**
- Modify: `/Users/maple/Documents/reader/src/service/ai_book_service.rs`

- [ ] Add `get_or_create_v3`, `save_v3`, `reset_v3`, and `set_enabled`.
- [ ] No row creates empty V3.
- [ ] Invalid or non-V3 stored data resets/deletes to empty V3 without migration or backup.
- [ ] Existing stored V3 is validated before mutation.
- [ ] Tests: invalid/non-V3 reset, non-V3 save rejection, renderable get, enabled toggle.

### Task 4: Add chapter loading, generation service, and write guard

**Files:**
- Create: `/Users/maple/Documents/reader/src/service/ai_book_generation_service.rs`
- Modify: `/Users/maple/Documents/reader/src/app/bootstrap.rs`
- Modify: `/Users/maple/Documents/reader/src/api/mod.rs`

- [ ] Load chapter text on the backend from `user_ns + bookUrl + chapterIndex`.
- [ ] Add per-book write guard keyed by `user_ns + bookUrl`.
- [ ] Add combined current-chapter generation and digest/patch catchup primitives.

### Task 5: Replace backend handlers/routes with V3 action API

**Files:**
- Modify: `/Users/maple/Documents/reader/src/api/handlers/ai_book.rs`
- Modify: `/Users/maple/Documents/reader/src/api/router.rs`

- [ ] Add `/reader3/aiBook/*` memory/chapter/reset/enabled/generate/map/catchup routes.
- [ ] Remove old raw memory save/delete/get routes and pause route.

### Task 6: Rewrite frontend types/API/store to thin V3 flow

**Files:**
- Modify: `/Users/maple/Documents/reader/frontend/src/types/index.ts`
- Modify: `/Users/maple/Documents/reader/frontend/src/api/aiBook.ts`
- Modify: `/Users/maple/Documents/reader/frontend/src/stores/aiBook.ts`

- [ ] Add V3 view/action types.
- [ ] Replace raw save/delete helpers with action APIs.
- [ ] Store only loads view models and calls backend actions.

### Task 7: Rewrite AiBookView and reader auto-update

**Files:**
- Modify: `/Users/maple/Documents/reader/frontend/src/views/AiBookView.vue`
- Modify: `/Users/maple/Documents/reader/frontend/src/stores/reader.ts`

- [ ] Render backend view models only.
- [ ] Current-chapter update sends identity only; frontend never sends chapter text or provider payloads.

### Task 8: Rewrite catchup around V3 generation

**Files:**
- Modify: `/Users/maple/Documents/reader/src/service/ai_book_catchup_service.rs`
- Modify: `/Users/maple/Documents/reader/src/api/handlers/ai_book.rs`

- [ ] Replace pause path with canceling/canceled.
- [ ] Use digest-first + backend guard + patch flow.
- [ ] Persist stats summary; keep active task state operational.

### Task 9: Map path, legacy cleanup, and final verification

**Files:**
- Modify as needed: backend generation service and `AiBookView.vue`
- Delete or quarantine after build is green: `frontend/src/utils/aiBookGeneration.ts`, `frontend/src/utils/aiBookV2.ts`

- [ ] Backend map generation or explicitly disabled empty map tab.
- [ ] No default-path frontend imports of legacy AI Book generation/V2 utilities.
- [ ] Run `cargo test ai_book`, `cd frontend && npm test -- aiBook`, `git diff --check`.

