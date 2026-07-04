-- V4 Knowledge Schema
-- Phase 4: knowledge cards, assertions, revision links, and entity references

CREATE TABLE IF NOT EXISTS knowledge_cards (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  category TEXT NOT NULL,
  topic_key TEXT NOT NULL,
  topic_display TEXT NOT NULL,
  current_summary TEXT,
  confidence REAL NOT NULL DEFAULT 0.5,
  importance_score REAL NOT NULL DEFAULT 0.5,
  first_seen_chapter INTEGER NOT NULL,
  last_updated_chapter INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (category IN (
    'power_system',
    'faction_structure',
    'world_rule',
    'history',
    'secret',
    'prophecy',
    'politics',
    'geography',
    'custom'
  )),
  CHECK (status IN ('active', 'inactive', 'deprecated')),
  UNIQUE(book_id, category, topic_key)
);

CREATE TABLE IF NOT EXISTS knowledge_assertions (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  card_id TEXT NOT NULL,
  source_claim_id TEXT NOT NULL,
  assertion_text TEXT NOT NULL,
  status TEXT NOT NULL,
  confidence REAL NOT NULL,
  importance_score REAL NOT NULL DEFAULT 0.5,
  chapter_index INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (card_id) REFERENCES knowledge_cards(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  CHECK (status IN (
    'active',
    'rumor',
    'uncertain',
    'revised',
    'contradicted',
    'false_in_world'
  )),
  UNIQUE(id, book_id)
);

CREATE TABLE IF NOT EXISTS knowledge_assertion_links (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  from_assertion_id TEXT NOT NULL,
  to_assertion_id TEXT NOT NULL,
  link_type TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (from_assertion_id) REFERENCES knowledge_assertions(id),
  FOREIGN KEY (to_assertion_id) REFERENCES knowledge_assertions(id),
  FOREIGN KEY (from_assertion_id, book_id) REFERENCES knowledge_assertions(id, book_id),
  FOREIGN KEY (to_assertion_id, book_id) REFERENCES knowledge_assertions(id, book_id),
  CHECK (from_assertion_id != to_assertion_id),
  CHECK (link_type IN ('supersedes', 'contradicts', 'supports', 'clarifies')),
  UNIQUE(book_id, from_assertion_id, to_assertion_id, link_type)
);

CREATE TABLE IF NOT EXISTS knowledge_assertion_entities (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  assertion_id TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (assertion_id) REFERENCES knowledge_assertions(id),
  FOREIGN KEY (entity_id) REFERENCES entities(id),
  CHECK (role IN (
    'subject',
    'related',
    'source',
    'target',
    'location',
    'faction',
    'ability',
    'realm'
  )),
  UNIQUE(assertion_id, entity_id, role)
);

CREATE INDEX idx_knowledge_cards_book_category_status
ON knowledge_cards(book_id, category, status);

CREATE INDEX idx_knowledge_cards_book_topic
ON knowledge_cards(book_id, category, topic_key);

CREATE INDEX idx_knowledge_assertions_card_status
ON knowledge_assertions(card_id, status, chapter_index);

CREATE INDEX idx_knowledge_assertions_source_claim
ON knowledge_assertions(source_claim_id);

CREATE INDEX idx_knowledge_assertions_book_chapter
ON knowledge_assertions(book_id, chapter_index);

CREATE INDEX idx_knowledge_assertion_links_from
ON knowledge_assertion_links(book_id, from_assertion_id, link_type);

CREATE INDEX idx_knowledge_assertion_links_to
ON knowledge_assertion_links(book_id, to_assertion_id, link_type);

CREATE INDEX idx_knowledge_assertion_entities_assertion
ON knowledge_assertion_entities(assertion_id);

CREATE INDEX idx_knowledge_assertion_entities_entity
ON knowledge_assertion_entities(book_id, entity_id);
