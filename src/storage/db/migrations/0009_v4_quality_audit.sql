-- V4 Quality / Audit / Reprocess Schema
-- Phase 6: quality workflow indexes, correction ledger, audit findings, reprocess jobs, prompt regression results

CREATE TABLE IF NOT EXISTS quarantined_claims (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  claim_id TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  reason_text TEXT,
  suggested_action TEXT NOT NULL DEFAULT 'needs_manual_review',
  status TEXT NOT NULL DEFAULT 'open',
  priority INTEGER NOT NULL DEFAULT 0,
  assigned_to TEXT,
  reviewed_at TEXT,
  review_decision TEXT,
  review_note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (claim_id) REFERENCES claims(id),
  UNIQUE(book_id, claim_id),
  CHECK (status IN ('open', 'accepted', 'rejected', 'retried', 'reclassified', 'ignored', 'resolved')),
  CHECK (suggested_action IN ('accept', 'reject', 'retry', 'reclassify', 'needs_manual_review', 'create_correction', 'no_action'))
);

CREATE TABLE IF NOT EXISTS user_corrections (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  correction_type TEXT NOT NULL,
  correction_json TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'proposed',
  source TEXT NOT NULL DEFAULT 'user',
  source_claim_id TEXT,
  source_span_id TEXT,
  created_by TEXT NOT NULL,
  applied_by TEXT,
  created_at TEXT NOT NULL,
  applied_at TEXT,
  reverted_at TEXT,
  error TEXT,
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  FOREIGN KEY (source_span_id) REFERENCES source_spans(id),
  CHECK (target_type IN (
    'claim',
    'entity',
    'entity_alias',
    'entity_property',
    'relationship',
    'relationship_event',
    'identity_link',
    'knowledge_card',
    'knowledge_assertion',
    'place',
    'place_edge',
    'entity_link',
    'projection_cache'
  )),
  CHECK (correction_type IN (
    'reject_claim',
    'accept_quarantined_claim',
    'retry_claim',
    'reclassify_claim',
    'merge_entities',
    'mark_not_same_entity',
    'split_required',
    'create_entity_link',
    'correct_property',
    'deactivate_relationship',
    'correct_relationship_label',
    'mark_knowledge_assertion_false',
    'revise_knowledge_assertion',
    'deactivate_place_edge',
    'mark_place_edge_conflict',
    'correct_place_parent',
    'rebuild_projection',
    'reprocess_scope'
  )),
  CHECK (status IN ('proposed', 'validated', 'applied', 'rejected', 'failed', 'reverted')),
  CHECK (source IN ('user', 'audit', 'system', 'test'))
);

CREATE TABLE IF NOT EXISTS correction_events (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  correction_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  before_json TEXT,
  after_json TEXT,
  actor TEXT NOT NULL,
  reason TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (correction_id) REFERENCES user_corrections(id),
  CHECK (event_type IN ('proposed', 'validated', 'applied', 'failed', 'reverted'))
);

CREATE TABLE IF NOT EXISTS quality_audit_runs (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  audit_type TEXT NOT NULL,
  scope_json TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued',
  started_at TEXT,
  finished_at TEXT,
  summary_json TEXT,
  error TEXT,
  CHECK (audit_type IN (
    'duplicate_entities',
    'relationship_pollution',
    'low_confidence_facts',
    'knowledge_topic_drift',
    'knowledge_contradictions',
    'map_conflicts',
    'orphaned_entities',
    'stale_projections',
    'prompt_regression',
    'full_book_quality'
  )),
  CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled'))
);

CREATE TABLE IF NOT EXISTS quality_audit_findings (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  audit_run_id TEXT NOT NULL,
  finding_type TEXT NOT NULL,
  severity TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  related_target_type TEXT,
  related_target_id TEXT,
  reason_code TEXT NOT NULL,
  reason_text TEXT,
  evidence_json TEXT,
  suggested_action TEXT NOT NULL DEFAULT 'needs_manual_review',
  status TEXT NOT NULL DEFAULT 'open',
  created_at TEXT NOT NULL,
  resolved_at TEXT,
  FOREIGN KEY (audit_run_id) REFERENCES quality_audit_runs(id),
  CHECK (finding_type IN (
    'duplicate_entity_candidate',
    'relationship_pollution_candidate',
    'low_confidence_accepted_fact',
    'stale_quarantined_claim',
    'knowledge_topic_drift_candidate',
    'knowledge_contradiction_candidate',
    'map_conflict_candidate',
    'orphaned_entity',
    'broken_reference',
    'stale_projection',
    'prompt_regression_failure'
  )),
  CHECK (severity IN ('info', 'low', 'medium', 'high', 'critical')),
  CHECK (suggested_action IN (
    'needs_manual_review',
    'create_correction',
    'reject_claim',
    'retry_claim',
    'merge_entities',
    'mark_not_same_entity',
    'create_entity_link',
    'deactivate_relationship',
    'correct_relationship_label',
    'mark_knowledge_assertion_false',
    'revise_knowledge_assertion',
    'deactivate_place_edge',
    'mark_place_edge_conflict',
    'correct_place_parent',
    'rebuild_projection',
    'reprocess_scope',
    'dismiss',
    'no_action'
  )),
  CHECK (status IN ('open', 'accepted', 'dismissed', 'converted_to_correction', 'resolved'))
);

CREATE TABLE IF NOT EXISTS quality_metrics (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  metric_type TEXT NOT NULL,
  metric_value REAL NOT NULL,
  metric_json TEXT,
  measured_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS reprocess_jobs (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  scope_type TEXT NOT NULL,
  scope_json TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued',
  requested_by TEXT NOT NULL,
  reason TEXT,
  dry_run INTEGER NOT NULL DEFAULT 1,
  prompt_version TEXT,
  schema_version TEXT,
  started_at TEXT,
  finished_at TEXT,
  result_json TEXT,
  error TEXT,
  CHECK (dry_run IN (0, 1)),
  CHECK (scope_type IN ('chapter', 'chapter_range', 'claim', 'claim_type', 'domain', 'prompt_version', 'full_book')),
  CHECK (mode IN ('retry_failed', 'reprocess_claims', 'reprocess_chapters', 'rebuild_projection', 'rebuild_domain', 'dry_run_compare', 'reprocess_claim', 'reprocess_single_chapter')),
  CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled'))
);

CREATE TABLE IF NOT EXISTS prompt_regression_runs (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  model TEXT NOT NULL,
  fixture_set TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued',
  started_at TEXT,
  finished_at TEXT,
  summary_json TEXT,
  error TEXT,
  CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled'))
);

CREATE TABLE IF NOT EXISTS prompt_regression_results (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  case_id TEXT NOT NULL,
  case_name TEXT NOT NULL,
  domain TEXT NOT NULL,
  expected_json TEXT NOT NULL,
  actual_json TEXT NOT NULL,
  pass INTEGER NOT NULL,
  diff_json TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES prompt_regression_runs(id),
  UNIQUE(run_id, case_id),
  CHECK (pass IN (0, 1))
);

CREATE INDEX idx_quarantined_claims_book_status
ON quarantined_claims(book_id, status, priority);

CREATE INDEX idx_quarantined_claims_claim
ON quarantined_claims(claim_id);

CREATE INDEX idx_user_corrections_book_status
ON user_corrections(book_id, status, correction_type);

CREATE INDEX idx_user_corrections_target
ON user_corrections(book_id, target_type, target_id, status);

CREATE INDEX idx_correction_events_correction
ON correction_events(correction_id, created_at);

CREATE INDEX idx_quality_audit_runs_book_type
ON quality_audit_runs(book_id, audit_type, status);

CREATE INDEX idx_quality_audit_findings_book_status
ON quality_audit_findings(book_id, status, severity);

CREATE INDEX idx_quality_audit_findings_target
ON quality_audit_findings(book_id, target_type, target_id);

CREATE INDEX idx_quality_metrics_latest
ON quality_metrics(book_id, metric_type, measured_at);

CREATE INDEX idx_reprocess_jobs_book_status
ON reprocess_jobs(book_id, status, mode);

CREATE INDEX idx_prompt_regression_runs_book_status
ON prompt_regression_runs(book_id, fixture_set, status);

CREATE INDEX idx_prompt_regression_results_run
ON prompt_regression_results(run_id, pass, domain);
