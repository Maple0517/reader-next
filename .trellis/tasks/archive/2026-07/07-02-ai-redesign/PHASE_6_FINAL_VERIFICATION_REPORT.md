# AI Book Memory V4 — Phase 6 Final Verification Report

Report date: 2026-07-04. Scope: Phase 6 verification/freeze only. No new phase, no new feature work, no archive.

## 1. Implementation Summary

| Module | Status | Notes |
|---|---|---|
| quarantined_claims workflow index | Done | Additive workflow index with claim FK and unique book/claim. |
| user_corrections correction command / audit ledger | Done | Command ledger with validation and event lifecycle. |
| correction_events append-only history | Done | Append-only event records; tests cover ordered event listing. |
| quality_audit_runs | Done | Audit lifecycle persisted with completed/failed status. |
| quality_audit_findings | Done | Findings persisted as review suggestions, not facts. |
| quality_metrics | Done | Latest metric snapshots and overview aggregation. |
| reprocess_jobs | Done | Safe MVP job ledger and runner. |
| prompt_regression_runs | Done | Fixture run metadata persisted. |
| prompt_regression_results | Done | Result/diff rows persisted with unique run/case. |
| Quality repository | Done | `QualityRepo` owns Phase 6 table access and transaction helpers. |
| Quarantine workflow service | Done | List/filter/context/actions for review flow. |
| User correction validation | Done | Target/type/source/manual override rules enforced. |
| Correction Applier | Partial | Safe MVP routes implemented for reject claim, relationship deactivate, projection/cache rebuild; broader domain-specific positive corrections remain guarded/deferred. |
| Batch audit framework | Done | Run lifecycle, findings-only writes, failed-run recording, finding-to-correction. |
| Duplicate entity audit | Done | Alias/name overlap, not-same guard, org/place link suggestion. |
| Relationship pollution audit | Done | Non-character endpoint and property-like label detection; valid relationship regression. |
| Knowledge quality audit | Done | Duplicate topic, rumor leakage, contradicted summary signals. |
| Map quality audit | Done | Open conflict, parent mismatch, stale layout signals. |
| Optional Quality Audit Judge | Not done | Optional judge path is not implemented; deterministic audit framework is used. |
| Selective reprocess orchestration | Partial | Safe MVP modes implemented; destructive/full canonical rebuild modes deferred/guarded. |
| Safe MVP reprocess modes | Done | `dry_run_compare`, `rebuild_projection`, claim/chapter validation skeletons. |
| Prompt regression fixture framework | Done | 15 deterministic cases across entity/property/relationship/identity/knowledge/map/quality. |
| Quality metrics aggregation | Done | Required quality counts and prompt failure metrics covered. |
| V4 Quality APIs | Done | Quality overview, quarantine, audits, corrections, reprocess, prompt regression. |
| Frontend V4QualityPanel.vue | Done | Lightweight quality panel with sections/actions and build coverage. |
| Existing pipeline / projection integration | Done | Existing projection/cache boundaries reused; Phase 1-5 tests pass. |
| Mock E2E tests | Done | Deterministic Rust and frontend tests cover Phase 6 workflows. |
| Real AI / prompt regression smoke test | Done with caveat | Local 8090 AI connectivity PASS; Phase 6 gated smoke PASS but uses deterministic fixture, not a QualityAuditJudge call. |

## 2. Files Changed

Backend:

- `src/storage/db/migrations/0009_v4_quality_audit.sql`
- `src/storage/db/v4/quality_repo.rs`
- `src/storage/db/v4/mod.rs`
- `src/service/v4/correction.rs`
- `src/service/v4/correction_applier.rs`
- `src/service/v4/quarantine_workflow.rs`
- `src/service/v4/quality_audit.rs`
- `src/service/v4/reprocess.rs`
- `src/service/v4/prompt_regression.rs`
- `src/service/v4/quality_metrics.rs`
- `src/service/v4/mod.rs`
- `src/api/handlers/v4.rs`
- `src/api/router.rs`

Frontend:

- `frontend/src/types/v4.ts`
- `frontend/src/api/v4/book.ts`
- `frontend/src/components/reader/V4QualityPanel.vue`
- `frontend/src/views/AiBookView.vue`

Tests:

- `frontend/src/api/v4/book.quality.test.ts`
- `frontend/src/components/reader/V4QualityPanel.test.ts`
- `frontend/src/views/AiBookView.test.ts`
- Rust tests are inline in `quality_repo`, `correction`, `correction_applier`, `quarantine_workflow`, `quality_audit`, `reprocess`, `prompt_regression`, `quality_metrics`, API/router modules.

Docs / task artifacts:

- `.trellis/tasks/07-02-ai-redesign/PHASE_6_FINAL_VERIFICATION_REPORT.md`

Frozen foundation impact:

- Phase 6 changes are additive branch work: new migration/table/repo/service/API/UI surface for quality workflow.
- Phase 1 character/property foundation: PASS, no semantic rewrite.
- Phase 2 relationship graph: PASS, relationship deactivation preserves rows/events and rebuilds graph/list cache.
- Phase 3 identity merge/redirect: PASS, duplicate audit respects active `not_same_identity`; no auto-merge.
- Phase 4 knowledge system: PASS, audit writes findings only and does not edit `current_summary`.
- Phase 5 map/place system: PASS, map audit writes findings only and does not mutate active graph/parent facts.
- Correction Applier writes Phase 6 workflow tables directly and only uses guarded/domain-safe operations for existing domains.
- No ad-hoc raw SQL editor for arbitrary Phase 1-5 canonical tables was added.
- No arbitrary raw DB editor, AI auto-fix-all, vector search, or cross-book ontology was added.

## 3. Database Verification

Tables verified by migration/schema tests:

- `0009_v4_quality_audit.sql`: Done.
- `quarantined_claims`: Done.
- `user_corrections`: Done.
- `correction_events`: Done.
- `quality_audit_runs`: Done.
- `quality_audit_findings`: Done.
- `quality_metrics`: Done.
- `reprocess_jobs`: Done.
- `prompt_regression_runs`: Done.
- `prompt_regression_results`: Done.

Key constraints verified or enforced:

- `quarantined_claims.claim_id` FK -> `claims(id)`: Done.
- `UNIQUE(book_id, claim_id)`: Done.
- `correction_events.correction_id` FK -> `user_corrections(id)`: Done.
- `correction_events` append-only usage: Done.
- `prompt_regression_results.run_id` FK -> `prompt_regression_runs(id)`: Done.
- `UNIQUE(run_id, case_id)`: Done.
- Enum `CHECK` constraints where practical: Done for migration-level enums; service layer validates compatibility.
- Correction target / correction_type compatibility: Done in `CorrectionValidationService`.
- Migration is additive only: Done.
- No alteration of Phase 1-5 table semantics: Done.
- No trigger mutates canonical state: Done.
- No hard delete of canonical / claims history outside reset semantics: Done.

`reset_v4` FK-safe order verified by `reset_v4_clears_quality_audit_tables` and broader reset tests:

`prompt_regression_results` → `prompt_regression_runs` → `correction_events` → `user_corrections` → `quality_audit_findings` → `quality_audit_runs` → `quality_metrics` → `reprocess_jobs` → `quarantined_claims` → `map_layout_snapshots` → `place_edge_conflicts` → `place_edge_sources` → `place_edges` → `place_details` → `entity_links` → `knowledge_assertion_entities` → `knowledge_assertion_links` → `knowledge_assertions` → `knowledge_cards` → `entity_merge_conflicts` → `entity_merge_operations` → `entity_identity_links` → `relationship_events` → `relationships` → `claim_source_spans` → `entity_current_properties` → `entity_properties` → `entity_aliases` → `claims` → `entities`.

Existing cleanup for `view_model_cache`, `chapter_summaries`, `chapter_processing_runs`, `processing_progress`, `ai_runs`, and `source_spans` remains present and was not removed.

## 4. Core Invariant Verification

- AI does not directly write canonical tables: PASS.
- Audit does not directly change canonical tables: PASS.
- User correction command creation does not directly change canonical tables: PASS.
- Correction Applier does not bypass reducer/domain-safe operation for implemented routes: PASS.
- Accepted claim can only become accepted after reducer/domain transaction succeeds: PASS for existing reducers; `accept_quarantined_claim` remains intent, not direct status flip.
- Correction action has audit trail: PASS.
- Batch audit only generates findings and does not auto merge/delete/deactivate: PASS.
- Duplicate entity audit does not auto merge: PASS.
- Relationship pollution audit does not directly delete relationship: PASS.
- Knowledge audit does not directly edit `current_summary`: PASS.
- Map audit does not directly edit `place_edges` / `parent_place_id`: PASS.
- Phase 6 MVP does not create source-less positive canonical facts: PASS.
- Manual override does not fake `source_span` evidence: PASS.
- Prompt regression failure does not affect canonical state: PASS.
- Reprocess dry-run does not mutate canonical state: PASS.
- Phase 1-5 frozen foundation was not refactored: PASS.

## 5. Quarantine Workflow Verification

Supported:

- List quarantined / uncertain / rejected claims: Done.
- Filter by `claim_type`, `risk_level`, `reason_code`, `chapter`, `ai_run`, `prompt_version`: Partial; query/service support the main fields and API query shape, but not every optional filter has deep UI exposure.
- View source spans: Done.
- View `ai_run.output_json`: Done.
- View candidate canonical impact: Partial; surfaced through claim/context JSON, not a rich impact diff UI.
- `accept`: Done as correction intent; it does not directly accept the claim.
- `reject`: Done.
- `retry`: Done via dry-run/reprocess job path.
- `reclassify`: Done as workflow metadata update.
- `convert_to_correction`: Done through audit/finding and correction creation routes.

Source-backed original claim acceptance:

- Original source spans preserved: PASS.
- Direct claim acceptance is intentionally guarded: PASS.
- Structural gate / domain judge / reducer path is required before any positive canonical acceptance: PASS by invariant; direct accept route does not bypass it.
- Claim status is not flipped to `accepted` by quarantine accept alone: PASS.

Correction-backed derived operation:

- User action does not mutate original claim into a different fact: PASS.
- Creates `user_correction`: PASS.
- Links workflow through `correction_events`: PASS.
- Original workflow can be resolved/rejected/retried without canonical mutation: PASS.
- Derived positive operation must go through reducer before canonical write: PASS; source-less positive MVP path rejected.

## 6. User Correction Verification

Minimal corrections:

- `reject bad claim`: Done.
- `accept quarantined claim`: Done as validated correction intent; canonical apply is guarded.
- `retry / reprocess claim`: Done via reprocess job path.
- `mark duplicate candidate`: Done as audit finding/correction suggestion; full merge apply deferred to Phase 3 path.
- `mark not same`: Partial; duplicate audit respects existing guard and correction type exists, but broad UI apply is not a rich workflow.
- `create_entity_link`: Partial; supported as finding/correction type path, full domain operation remains guarded.
- `flag relationship pollution`: Done.
- `deactivate relationship`: Done.
- `flag knowledge assertion false / rumor / contradicted`: Partial; audit flags, full correction reducer route deferred.
- `deactivate bad place edge`: Partial; finding/correction type supported, full map correction apply deferred.
- `mark place edge conflict`: Done as finding/correction suggestion.
- `correct place parent through map validation`: Partial; guarded/deferred beyond MVP.
- `rebuild projection`: Done.

Validation:

- Target exists validation: PASS.
- Target type / correction type compatibility: PASS.
- Invalid `correction_json` rejected: PASS.
- Duplicate apply no-op or existing applied result: PASS.
- Events written for proposed / validated / applied / failed: PASS for implemented paths.
- Source-backed correction requires source claim/span: PASS.
- Source-less manual correction cannot create new positive canonical fact in Phase 6 MVP: PASS.
- Manual correction never fakes source span evidence: PASS.

## 7. Correction Applier Verification

Identity:

- `merge_entities` routes through Phase 3 identity/merge reducer: Partial/deferred; audit suggests merge, no unsafe auto-merge.
- Active `not_same_identity` blocks merge suggestion: Done in duplicate audit guard.
- `mark_not_same_entity` creates active guard: Partial/deferred for full apply path.
- Already merged pair creates `split_required` finding, not unsafe auto-split: Partial/deferred; no unsafe split implemented.

Relationship:

- `deactivate_relationship` uses safe relationship deactivation path: Done.
- Does not delete relationship row: Done.
- Preserves `relationship_events`: Done.
- Rebuilds relationship graph/list cache: Done.

Property:

- `correct_property` source-backed reducer path: Partial/deferred.
- Preserves property history / merge strategy: Preserved by not implementing unsafe direct writes.

Knowledge:

- `mark_knowledge_assertion_false` / `revise_knowledge_assertion` route through knowledge path: Partial/deferred.
- Does not directly edit `current_summary`: Done.
- Summary rebuild from active factual assertions remains Phase 4 reducer/projection behavior: Preserved.

Map:

- `deactivate_place_edge`, `mark_place_edge_conflict`, `correct_place_parent`: Partial/deferred for full apply; audit produces findings only.
- Parent cycle prevention remains Phase 5 reducer/gate behavior: Preserved.
- Map graph/layout rebuild remains projection-only: Preserved.

Projection:

- `rebuild_projection` invalidates relevant cache: Done.
- Does not mutate factual canonical state: Done.

## 8. Batch Audit Verification

- Audit creates `quality_audit_runs`: PASS.
- Audit writes `quality_audit_findings`: PASS.
- Audit updates `summary_json`: PASS.
- Audit failure records failed status: PASS.
- Audit cancellation records cancelled status: Partial; status enum supports cancellation, explicit cancellation runner is minimal.
- Audit never directly mutates canonical state: PASS.
- High severity finding still does not auto-fix: PASS.
- Converting finding to correction creates `user_correction`: PASS.

Audit types:

- `duplicate_entities`: Done.
- `relationship_pollution`: Done.
- `low_confidence_facts`: Partial/minimal via metrics/findings framework, not a full dedicated scanner.
- `knowledge_topic_drift`: Done.
- `knowledge_contradictions`: Done via contradiction/stale summary signals.
- `map_conflicts`: Done.
- `orphaned_entities`: Not done/deferred.
- `stale_projections`: Partial via stale map layout finding and projection rebuild path.
- `full_book_quality`: Done as aggregate finding runner path.

## 9. Duplicate Entity Audit Verification

Signals:

- Same normalized canonical_name: Done for same-name/name overlap candidates.
- Alias overlap: Done.
- Canonical_name matches another alias: Done through overlap logic.
- Same type + high property overlap: Not done/deferred.
- Same place name under same parent: Partial; org/place same-name is covered, parent-specific duplicate place heuristic deferred.
- Organization/place same-name special case: Done.

Guardrails:

- No automatic merge: PASS.
- Active `not_same_identity` blocks/downgrades merge suggestion: PASS.
- Organization/place same-name suggests `create_entity_link`, not merge: PASS.
- Applying merge goes through Phase 3 identity path: PASS by not providing unsafe direct merge.
- `create_entity_link` uses generic `entity_links`, not `entity_identity_links`: PASS for intended route/suggestion.

## 10. Relationship Pollution Audit Verification

Signals:

- Endpoint not character: Done.
- Label/reason resembles location / ability / equipment / faction affiliation: Done for property-like labels.
- Temporary co-location: Partial/deferred heuristic.
- Low confidence with no sustained events: Partial/deferred heuristic.
- One low-value event only: Partial/deferred heuristic.
- Stale relationship with no later support: Partial/deferred heuristic.

Verification:

- Pollution audit only creates finding: PASS.
- Valid long-term relationship is not overflagged: PASS.
- Deactivation correction does not delete row: PASS.
- Relationship projection rebuild works: PASS.

## 11. Knowledge Quality Audit Verification

Signals:

- Duplicate topic cards: Done.
- Category drift: Partial via duplicate/topic drift candidate, advanced category drift deferred.
- Custom category overuse: Not done/deferred.
- Assertion contradiction not linked: Done at MVP signal level through contradicted summary checks.
- Rumor / uncertain assertion leaking into factual summary: Done.
- Same assertion under multiple cards: Partial/deferred.
- Stale summary after assertion status changes: Done via summary/assertion mismatch checks.
- Merged victim exposed in projection: Deferred.

Verification:

- Audit does not edit `knowledge_cards.current_summary`: PASS.
- False/revision correction routes through knowledge path: Partial/deferred for full apply.
- Summary rebuild excludes rumor / uncertain / false_in_world from factual summary: PASS from Phase 4 reducer/projection regression coverage.

## 12. Map Quality Audit Verification

Signals:

- Inverse / opposite active direction conflict: Done via open conflict finding path.
- Hierarchy cycle / self-parent: Preserved by Phase 5 reducer/gate; Phase 6 audit does not add a full scanner.
- `place_edges.contains` contradicts `place_details.parent_place_id`: Done.
- Place edge endpoint missing `place_details`: Partial/deferred.
- Same-name organization/place missing `entity_link`: Covered in duplicate/org-place audit, not map audit proper.
- Stale open conflict: Done through open conflict finding.
- Low-confidence active edge: Partial/deferred.
- Stale layout snapshot: Done.

Verification:

- Audit does not mutate `place_edges` or `parent_place_id`: PASS.
- Deactivate edge correction uses map path: Partial/deferred; no unsafe direct mutation.
- Parent correction uses map conflict validation: Partial/deferred.
- Layout rebuild remains projection-only: PASS.

## 13. Selective Reprocess Verification

Safe MVP modes:

- `retry_failed`: Partial/skeleton.
- `rebuild_projection`: Done.
- `dry_run_compare`: Done.
- `reprocess_claim`: Done as validated job/scope skeleton.
- `reprocess_single_chapter` append-only mode: Partial/skeleton.

Guarded/deferred modes:

- Chapter range canonical rebuild: Deferred/guarded.
- Full book canonical rebuild: Deferred/guarded.
- Domain canonical rebuild from scratch: Deferred/guarded.

Verification:

- Do not process beyond `max_read_chapter`: PASS.
- Preserve old `ai_runs`: PASS by no destructive path.
- Preserve old claims: PASS.
- Dry-run does not mutate canonical: PASS.
- Failed reprocess leaves no partial canonical state: PASS.
- `result_json` summarizes affected scope: PASS for MVP jobs.
- Reprocess does not introduce new global `claims.status` values: PASS.
- Stale/superseded semantics stay in job/result metadata unless existing lifecycle supports it: PASS.

## 14. Prompt Regression Verification

- Prompt regression fixtures exist: PASS.
- Runs/results persist: PASS.
- At least 15 cases across Phase 1-5: PASS, exactly 15 deterministic cases.
- Expected/actual observations captured: PASS in expected/actual JSON.
- Expected/actual judge decisions captured: Partial; deterministic fixture stores expected/actual JSON, not live judge transcripts.
- Canonical side effect summary captured: PASS in smoke checks and no-mutation assertions.
- Pass/fail captured: PASS.
- `diff_json` captured: PASS.
- Regression mode does not mutate production canonical state: PASS.
- Failure only affects prompt regression results / metrics: PASS.

Fixture coverage:

- Phase 1: character introduction, alias, property replace/append: PASS.
- Phase 2: relationship valid and relationship pollution: PASS.
- Phase 3: identity merge candidate and same-name/not-same guard: PASS.
- Phase 4: knowledge card, rumor, contradiction: PASS.
- Phase 5: map place, map edge, map conflict: PASS.
- Additional quality correction case: PASS.

## 15. Quality Metrics Verification

Generated metrics:

- Open quarantined claims: Done.
- Open uncertain claims: Done.
- Rejected claim rate: Done.
- High-risk claim acceptance rate: Done.
- Duplicate entity candidates: Done.
- Relationship pollution candidates: Done.
- Knowledge topic drift candidates: Done.
- Map conflict candidates: Done.
- Open correction count: Done.
- Failed reprocess count: Done.
- Prompt regression failure count: Done.
- Projection rebuild error count: Done.

Verification:

- Metrics insert works: PASS.
- Latest metrics query works: PASS.
- Quality overview API returns metrics summary: PASS.
- Metrics failure does not block canonical state: PASS by isolation; no canonical writes in metrics path.

## 16. API Verification

V4 quality APIs preserve `bookUrl` convention and camelCase responses.

Verified routes:

- `GET /api/books/v4/quality`: Done.
- `GET /api/books/v4/quality/quarantine`: Done.
- `POST /api/books/v4/quality/quarantine/:id/action`: Done.
- `GET /api/books/v4/quality/audit-runs`: Done.
- `POST /api/books/v4/quality/audit-runs`: Done.
- `GET /api/books/v4/quality/audit-findings`: Done.
- `POST /api/books/v4/quality/audit-findings/:id/action`: Done.
- `GET /api/books/v4/quality/corrections`: Done.
- `POST /api/books/v4/quality/corrections`: Done.
- `POST /api/books/v4/quality/corrections/:id/apply`: Done.
- `GET /api/books/v4/quality/reprocess-jobs`: Done.
- `POST /api/books/v4/quality/reprocess-jobs`: Done.
- `POST /api/books/v4/quality/reprocess-jobs/:id/cancel`: Done.
- `GET /api/books/v4/quality/prompt-regression-runs`: Done.
- `POST /api/books/v4/quality/prompt-regression-runs`: Done.
- `GET /api/books/v4/quality/prompt-regression-runs/:id/results`: Done.

Invalid actions return clear errors through existing handler validation. Existing Phase 1-5 APIs are covered by `cargo test v4 --lib`, `cargo test pipeline --lib`, `cargo test projection --lib`, and full `cargo test`.

## 17. Frontend Verification

Verified frontend behavior:

- `V4QualityPanel` renders: PASS.
- Quality tab visible in `AiBookView`: PASS.
- Overview section works: PASS.
- Quarantine section works: PASS.
- Audit Findings section works: PASS.
- Corrections section works: PASS.
- Reprocess section works: PASS.
- Prompt Regression section works: PASS.
- Metrics section works: PASS.
- Source evidence / JSON context is visible in lightweight form: Partial; not a complex drawer UI.
- `ai_run` output summary visible: Partial; rendered as summarized JSON/context, not a rich inspector.
- Suggested action visible: PASS.
- Correction status visible: PASS.
- Reprocess job status visible: PASS.
- Prompt regression pass/fail visible: PASS.
- Quality metrics summary visible: PASS.

Dangerous actions:

- Default review / dry-run first: PASS for dry-run reprocess and lightweight review panel.
- Direct apply allowed only for guarded MVP actions: PASS.
- Canonical-touching corrections require validation before apply: PASS at API/service boundary.

Not present:

- Raw JSON DB editor: PASS.
- Destructive delete button: PASS.
- AI fix-all: PASS.
- Arbitrary canonical row editor: PASS.
- Full CMS: PASS.

Existing panels unaffected:

- Character Panel: PASS by focused AiBookView tests and build.
- Relationship Panel: PASS by V4 pipeline/projection tests.
- Identity Panel: PASS by V4 tests.
- Knowledge Panel: PASS by V4 tests/build.
- Map Panel: PASS by projection/API tests/build.

## 18. Test Results

Fresh commands and results:

- `cargo fmt --check`: PASS.
- `git diff --check`: PASS.
- `cargo test --lib`: PASS — 652 passed.
- `cargo test`: PASS on fresh rerun — lib/bin/integration/doc tests passed; one earlier transient bin-target panic in `layout_snapshot_generated_from_active_edges` did not reproduce in focused rerun or full rerun.
- `cargo test v4 --lib`: PASS — 469 passed.
- `cargo test quality --lib`: PASS — 31 passed.
- `cargo test correction --lib`: PASS — 11 passed.
- `cargo test audit --lib`: PASS — 19 passed.
- `cargo test reprocess --lib`: PASS — 5 passed.
- `cargo test prompt_regression --lib`: PASS — 7 passed.
- `cargo test pipeline --lib`: PASS — 30 passed.
- `cargo test projection --lib`: PASS — 30 passed.
- Frontend Quality API tests: PASS — included in focused Vitest run.
- Frontend `V4QualityPanel` tests: PASS — included in focused Vitest run.
- Frontend `AiBookView` tests: PASS — included in focused Vitest run.
- Focused frontend command: `cd frontend && npm test -- --run src/api/v4/book.quality.test.ts src/components/reader/V4QualityPanel.test.ts src/views/AiBookView.test.ts`: PASS — 3 files / 8 tests.
- `cd frontend && npm run build`: PASS.
- `RUN_REAL_AI_TESTS=1 AI_BASE_URL=http://127.0.0.1:8090 AI_MODEL=gpt-5.4 cargo test real_ai_smoke_test_quality_phase6_fixture --lib -- --nocapture`: PASS — 1 passed.
- Direct local AI endpoint smoke: PASS — `http://127.0.0.1:8090`, model `gpt-5.4`, output `phase6-ai-smoke-ok`.

## 19. Real AI / Prompt Regression Smoke Test

Environment:

- `AI_MODEL`: `gpt-5.4`.
- `AI_BASE_URL`: `http://127.0.0.1:8090`.
- API key: configured for the run; not written into this report.
- `prompt_version`: `phase6-smoke` for the Phase 6 smoke fixture.
- `schema_version`: `v4`.

Smoke output:

- Direct AI connectivity: PASS; local endpoint returned `phase6-ai-smoke-ok`.
- Phase 6 gated smoke command: PASS.
- `audit_runs` count: 1 deterministic findings-only run in the smoke fixture.
- Findings by type: `duplicate_entity_candidate`, `relationship_pollution_candidate`, `knowledge_topic_drift_candidate`, `map_conflict_candidate`, `prompt_regression_failure`, `stale_quarantined_claim`.
- `quality_audit_judge ai_runs` count: 0; optional `QualityAuditJudge` not implemented.
- Corrections proposed: at least 2 in fixture flow (`accept_quarantined_claim`, `reject_claim`).
- Corrections applied: 1 deterministic `reject_claim` apply path.
- Corrections failed: 0 in smoke result.
- `reprocess_jobs` count: 1 dry-run job.
- Dry-run canonical mutations count: 0.
- Prompt regression cases run: 15.
- Prompt regression pass/fail count: 15 pass / 0 fail for deterministic fixtures.
- Metrics generated: yes; quality metric rows produced.
- Canonical mutation from audit: 0.
- Canonical mutation from prompt regression: 0.
- Projection rebuild count: 1 dry-run/rebuild path exercised by reprocess/projection tests; smoke job records completed dry-run.
- Phase 1 regression: PASS.
- Phase 2 regression: PASS.
- Phase 3 regression: PASS.
- Phase 4 regression: PASS.
- Phase 5 regression: PASS.

Product Ready criteria:

- Real judge path called if implemented: PASS by condition; no optional QualityAuditJudge exists.
- `ai_runs.output_json` recorded if QualityAuditJudge is implemented: Not applicable.
- Audit creates findings only: PASS.
- Duplicate entity finding does not auto-merge: PASS.
- Relationship pollution finding does not auto-deactivate: PASS.
- Correction Applier applies through domain-safe path: PASS for implemented MVP routes.
- Correction does not directly write arbitrary canonical tables: PASS.
- Dry-run reprocess mutates nothing: PASS.
- Prompt regression stores results/diffs: PASS.
- Prompt regression mutates no production canonical state: PASS.
- Quality metrics generated: PASS.
- Frontend Quality panel builds: PASS.
- Phase 1-5 regressions pass: PASS.

## 20. Known Limitations

Deferred / intentionally not implemented:

- Arbitrary DB editor: not implemented.
- AI auto-fix-all: not implemented.
- Vector search: not implemented.
- Cross-book ontology: not implemented.
- Production-grade workflow engine: not implemented.
- Collaborative editing: not implemented.
- Source-less positive manual facts: not implemented in MVP.
- Full-book canonical rebuild: guarded/deferred.
- Domain rebuild from scratch: guarded/deferred.
- Chapter-range canonical rebuild: guarded/deferred.
- Complex diff UI: deferred.
- Advanced permission system: deferred.
- Optional QualityAuditJudge: not implemented.
- Full domain correction apply routes for positive property/knowledge/map/identity operations: partial/deferred; current implementation avoids unsafe direct writes.
- Relationship/knowledge/map audits are useful MVP scanners, not exhaustive semantic quality engines.
- One transient full `cargo test` bin-target panic was observed once and did not reproduce in focused rerun or full rerun; keep an eye on this test if it reappears.

## 21. Final Conclusion

- Phase 6 Backend: PASS.
- Phase 6 Frontend: PASS.
- Phase 6 Real AI / Prompt Regression Smoke: PASS with caveat — direct local AI connectivity passed; Phase 6 smoke fixture is deterministic and optional QualityAuditJudge is not implemented.
- Phase 1 Regression: PASS.
- Phase 2 Relationship Regression: PASS.
- Phase 3 Identity Regression: PASS.
- Phase 4 Knowledge Regression: PASS.
- Phase 5 Map Regression: PASS.
- Phase 6 Product Readiness: PASS for the implemented safe MVP quality control plane.
- Phase 6 Safe MVP Freeze: YES.
- V4 MVP Product Ready: YES.
- V4 Full Quality System Complete: NO, deferred items are tracked as limitations.
- The freeze scope is the implemented safe MVP quality control plane.
- Deferred Phase 6.1+ items include:
  - full positive correction routes for property / knowledge / map / identity
  - full-book canonical rebuild
  - chapter-range canonical rebuild
  - domain rebuild from scratch
  - optional QualityAuditJudge
  - richer evidence / ai_run inspector UI
  - orphaned_entities full audit
  - exhaustive relationship / knowledge / map semantic audits
- 进入总体验收前必须修的问题: no blocking issue found. Recommended watch item: if the transient `layout_snapshot_generated_from_active_edges` bin-target panic recurs, isolate it as a Phase 5 projection flake before release hardening.
