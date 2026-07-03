-- V4 Relationship Schema
-- Phase 2: Character-character relationships and relationship events

CREATE TABLE IF NOT EXISTS relationships (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  subject_character_id TEXT NOT NULL,
  object_character_id TEXT NOT NULL,
  relation_group TEXT NOT NULL,
  relation_label TEXT NOT NULL,
  directionality TEXT NOT NULL DEFAULT 'undirected',
  current_state TEXT,
  strength REAL NOT NULL DEFAULT 0.5,
  polarity TEXT NOT NULL DEFAULT 'neutral',
  confidence REAL NOT NULL DEFAULT 0.5,
  importance_score REAL NOT NULL DEFAULT 0.5,
  first_seen_chapter INTEGER NOT NULL,
  last_changed_chapter INTEGER NOT NULL,
  last_seen_chapter INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (subject_character_id) REFERENCES entities(id),
  FOREIGN KEY (object_character_id) REFERENCES entities(id),
  CHECK (subject_character_id != object_character_id),
  UNIQUE(book_id, subject_character_id, object_character_id, relation_group, directionality)
);

CREATE TABLE IF NOT EXISTS relationship_events (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  relationship_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  relation_group TEXT NOT NULL,
  relation_label TEXT NOT NULL,
  state_after TEXT,
  strength_after REAL,
  polarity_after TEXT,
  chapter_index INTEGER NOT NULL,
  source_claim_id TEXT NOT NULL,
  confidence REAL NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (relationship_id) REFERENCES relationships(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  UNIQUE(source_claim_id)
);

CREATE INDEX idx_relationships_subject ON relationships(book_id, subject_character_id, status);
CREATE INDEX idx_relationships_object ON relationships(book_id, object_character_id, status);
CREATE INDEX idx_relationships_group ON relationships(book_id, relation_group, status);
CREATE INDEX idx_relationship_events_rel ON relationship_events(relationship_id, chapter_index);
