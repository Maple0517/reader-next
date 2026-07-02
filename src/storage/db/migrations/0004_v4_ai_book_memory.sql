-- V4 AI Book Memory Schema
-- Clean-slate redesign: Source Layer, Claim Layer, Entity Layer, Property Layer, Progress, Cache

-- Source Layer
CREATE TABLE IF NOT EXISTS chapters (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  chapter_index INTEGER NOT NULL,
  title TEXT,
  raw_text TEXT NOT NULL,
  text_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(book_id, chapter_index)
);

CREATE TABLE IF NOT EXISTS chapter_segments (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  chapter_hash TEXT NOT NULL,
  segment_index INTEGER NOT NULL,
  segment_type TEXT NOT NULL DEFAULT 'default',
  start_span_id TEXT,
  end_span_id TEXT,
  start_offset INTEGER,
  end_offset INTEGER,
  text_hash TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  UNIQUE(book_id, chapter_id, chapter_hash, segment_index),
  FOREIGN KEY (chapter_id) REFERENCES chapters(id)
);

CREATE TABLE IF NOT EXISTS source_spans (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  chapter_hash TEXT NOT NULL,
  segment_id TEXT NOT NULL,
  span_index INTEGER NOT NULL,
  start_offset INTEGER NOT NULL,
  end_offset INTEGER NOT NULL,
  text_excerpt TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  UNIQUE(book_id, chapter_id, chapter_hash, span_index),
  FOREIGN KEY (chapter_id) REFERENCES chapters(id),
  FOREIGN KEY (segment_id) REFERENCES chapter_segments(id)
);

CREATE TABLE IF NOT EXISTS ai_runs (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  segment_id TEXT,
  run_type TEXT NOT NULL,
  model TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  input_hash TEXT NOT NULL,
  output_json TEXT,
  status TEXT NOT NULL,
  error TEXT,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  FOREIGN KEY (chapter_id) REFERENCES chapters(id)
);

CREATE TABLE IF NOT EXISTS chapter_processing_runs (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  chapter_index INTEGER NOT NULL,
  chapter_hash TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  status TEXT NOT NULL,
  error TEXT,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  UNIQUE(book_id, chapter_index, chapter_hash, prompt_version, schema_version)
);

CREATE TABLE IF NOT EXISTS chapter_summaries (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  chapter_index INTEGER NOT NULL,
  summary TEXT NOT NULL,
  key_points_json TEXT,
  updated_at TEXT NOT NULL,
  UNIQUE(book_id, chapter_index)
);

-- Claim Layer
CREATE TABLE IF NOT EXISTS claims (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  chapter_index INTEGER NOT NULL,
  claim_type TEXT NOT NULL,
  subject_mention TEXT,
  object_mention TEXT,
  subject_entity_id TEXT,
  object_entity_id TEXT,
  predicate TEXT NOT NULL,
  value_json TEXT,
  value_text TEXT,
  primary_source_span_id TEXT NOT NULL,
  ai_run_id TEXT NOT NULL,
  confidence REAL NOT NULL,
  risk_level TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'proposed',
  supersedes_claim_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (primary_source_span_id) REFERENCES source_spans(id),
  FOREIGN KEY (ai_run_id) REFERENCES ai_runs(id)
);

CREATE TABLE IF NOT EXISTS claim_source_spans (
  claim_id TEXT NOT NULL,
  source_span_id TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'primary',
  PRIMARY KEY(claim_id, source_span_id),
  FOREIGN KEY (claim_id) REFERENCES claims(id),
  FOREIGN KEY (source_span_id) REFERENCES source_spans(id)
);

-- Entity Layer
CREATE TABLE IF NOT EXISTS entities (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  canonical_name TEXT NOT NULL,
  display_name TEXT NOT NULL,
  short_summary TEXT,
  importance_score REAL NOT NULL DEFAULT 0,
  first_seen_chapter INTEGER NOT NULL,
  last_seen_chapter INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS entity_aliases (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  alias TEXT NOT NULL,
  alias_type TEXT NOT NULL,
  first_seen_chapter INTEGER NOT NULL,
  last_seen_chapter INTEGER,
  confidence REAL NOT NULL,
  source_claim_id TEXT,
  UNIQUE(book_id, entity_id, alias),
  FOREIGN KEY (entity_id) REFERENCES entities(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id)
);

-- Property Layer
CREATE TABLE IF NOT EXISTS property_dimensions (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  dimension_key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  value_type TEXT NOT NULL,
  aliases_json TEXT,
  importance REAL NOT NULL DEFAULT 0.5,
  merge_strategy TEXT NOT NULL DEFAULT 'replace',
  created_by TEXT NOT NULL DEFAULT 'system',
  status TEXT NOT NULL DEFAULT 'active',
  first_seen_chapter INTEGER NOT NULL,
  UNIQUE(book_id, entity_type, dimension_key)
);

CREATE TABLE IF NOT EXISTS entity_properties (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  dimension_key TEXT NOT NULL,
  value_text TEXT,
  value_json TEXT,
  valid_from_chapter INTEGER NOT NULL,
  valid_to_chapter INTEGER,
  source_claim_id TEXT NOT NULL,
  confidence REAL NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  supersedes_property_id TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (entity_id) REFERENCES entities(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  FOREIGN KEY (supersedes_property_id) REFERENCES entity_properties(id)
);

CREATE TABLE IF NOT EXISTS entity_current_properties (
  book_id TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  dimension_key TEXT NOT NULL,
  property_id TEXT NOT NULL,
  value_text TEXT,
  value_json TEXT,
  updated_chapter INTEGER NOT NULL,
  confidence REAL NOT NULL,
  PRIMARY KEY(book_id, entity_id, dimension_key),
  FOREIGN KEY (entity_id) REFERENCES entities(id),
  FOREIGN KEY (property_id) REFERENCES entity_properties(id)
);

-- Progress + Job State
CREATE TABLE IF NOT EXISTS reading_progress (
  book_id TEXT PRIMARY KEY,
  max_read_chapter INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS processing_progress (
  book_id TEXT PRIMARY KEY,
  max_processed_chapter INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'idle',
  target_chapter INTEGER,
  current_chapter INTEGER,
  current_segment_id TEXT,
  last_error TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS book_memory_v4_settings (
  book_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL
);

-- View Model Cache
CREATE TABLE IF NOT EXISTS view_model_cache (
  book_id TEXT NOT NULL,
  view_type TEXT NOT NULL,
  scope_id TEXT NOT NULL,
  max_chapter INTEGER NOT NULL,
  payload_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(book_id, view_type, scope_id, max_chapter)
);

-- Indexes
CREATE INDEX idx_chapters_book_chapter ON chapters(book_id, chapter_index);
CREATE INDEX idx_segments_chapter ON chapter_segments(chapter_id, status);
CREATE INDEX idx_spans_segment ON source_spans(segment_id, status);
CREATE INDEX idx_spans_chapter_status ON source_spans(chapter_id, status);
CREATE INDEX idx_claims_book_chapter_status ON claims(book_id, chapter_index, status);
CREATE INDEX idx_claims_book_ai_run ON claims(book_id, ai_run_id);
CREATE INDEX idx_aliases_book_alias ON entity_aliases(book_id, alias);
CREATE INDEX idx_entities_book_type_name ON entities(book_id, entity_type, canonical_name);
CREATE INDEX idx_current_properties_entity ON entity_current_properties(book_id, entity_id);
CREATE INDEX idx_properties_entity_dimension ON entity_properties(book_id, entity_id, dimension_key);
CREATE INDEX idx_settings_book ON book_memory_v4_settings(book_id);
