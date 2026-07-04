-- V4 Identity Schema
-- Phase 3: identity links, merge operations, and merge conflicts

CREATE TABLE IF NOT EXISTS entity_identity_links (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  entity_a_id TEXT NOT NULL,
  entity_b_id TEXT NOT NULL,
  link_type TEXT NOT NULL,
  confidence REAL NOT NULL,
  source_claim_id TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (entity_a_id) REFERENCES entities(id),
  FOREIGN KEY (entity_b_id) REFERENCES entities(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  CHECK (entity_a_id != entity_b_id),
  CHECK (link_type IN (
    'same_identity',
    'possible_same_identity',
    'disguise',
    'alias_reveal',
    'true_name_reveal',
    'mistaken_identity',
    'not_same_identity',
    'redirect'
  )),
  CHECK (status IN ('active', 'inactive', 'superseded'))
);

CREATE TABLE IF NOT EXISTS entity_merge_operations (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  survivor_entity_id TEXT NOT NULL,
  victim_entity_id TEXT NOT NULL,
  source_identity_link_id TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  confidence REAL NOT NULL,
  status TEXT NOT NULL,
  property_conflict_count INTEGER NOT NULL DEFAULT 0,
  relationship_merge_count INTEGER NOT NULL DEFAULT 0,
  result_json TEXT,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  FOREIGN KEY (survivor_entity_id) REFERENCES entities(id),
  FOREIGN KEY (victim_entity_id) REFERENCES entities(id),
  FOREIGN KEY (source_identity_link_id) REFERENCES entity_identity_links(id),
  CHECK (survivor_entity_id != victim_entity_id),
  CHECK (status IN ('pending', 'completed', 'failed', 'rolled_back'))
);

CREATE TABLE IF NOT EXISTS entity_merge_conflicts (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  merge_operation_id TEXT NOT NULL,
  survivor_entity_id TEXT NOT NULL,
  victim_entity_id TEXT NOT NULL,
  conflict_type TEXT NOT NULL,
  dimension_key TEXT,
  survivor_value TEXT,
  victim_value TEXT,
  resolution TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (merge_operation_id) REFERENCES entity_merge_operations(id),
  FOREIGN KEY (survivor_entity_id) REFERENCES entities(id),
  FOREIGN KEY (victim_entity_id) REFERENCES entities(id),
  CHECK (survivor_entity_id != victim_entity_id),
  CHECK (resolution IN (
    'keep_survivor',
    'keep_latest',
    'keep_higher_confidence',
    'keep_both_history',
    'needs_audit'
  ))
);

CREATE UNIQUE INDEX idx_identity_links_book_pair
ON entity_identity_links(book_id, entity_a_id, entity_b_id, link_type, status)
WHERE link_type != 'redirect';

CREATE INDEX idx_identity_links_source_claim
ON entity_identity_links(source_claim_id);

CREATE INDEX idx_identity_links_status
ON entity_identity_links(book_id, status, link_type);

CREATE UNIQUE INDEX idx_identity_links_active_redirect_unique
ON entity_identity_links(book_id, entity_a_id, link_type, status)
WHERE link_type = 'redirect';

CREATE INDEX idx_merge_ops_book_survivor
ON entity_merge_operations(book_id, survivor_entity_id, status);

CREATE INDEX idx_merge_ops_book_victim
ON entity_merge_operations(book_id, victim_entity_id, status);

CREATE INDEX idx_merge_conflicts_op
ON entity_merge_conflicts(merge_operation_id, created_at);
