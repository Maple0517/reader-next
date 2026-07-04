-- V4 Places and Map Schema
-- Phase 5: canonical place details, topology edges, layout snapshots, conflicts, and cross-type entity links

CREATE TABLE IF NOT EXISTS place_details (
  entity_id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  place_type TEXT NOT NULL,
  parent_place_id TEXT,
  scale_level INTEGER NOT NULL DEFAULT 0,
  importance_score REAL NOT NULL DEFAULT 0.5,
  map_visible INTEGER NOT NULL DEFAULT 1,
  first_seen_chapter INTEGER NOT NULL,
  last_seen_chapter INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (entity_id) REFERENCES entities(id),
  FOREIGN KEY (parent_place_id) REFERENCES entities(id),
  CHECK (parent_place_id IS NULL OR parent_place_id != entity_id),
  CHECK (map_visible IN (0, 1)),
  CHECK (place_type IN (
    'world',
    'continent',
    'region',
    'country',
    'city',
    'sect_site',
    'building',
    'room',
    'mountain',
    'river',
    'forest',
    'road',
    'route',
    'secret_realm',
    'battlefield',
    'dungeon',
    'unknown'
  )),
  CHECK (status IN ('active', 'inactive', 'uncertain', 'deprecated'))
);

CREATE TABLE IF NOT EXISTS place_edges (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  from_place_id TEXT NOT NULL,
  to_place_id TEXT NOT NULL,
  edge_type TEXT NOT NULL,
  direction_hint TEXT,
  distance_hint TEXT,
  confidence REAL NOT NULL DEFAULT 0.5,
  source_claim_id TEXT NOT NULL,
  latest_source_claim_id TEXT,
  first_seen_chapter INTEGER NOT NULL,
  last_seen_chapter INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (from_place_id) REFERENCES entities(id),
  FOREIGN KEY (to_place_id) REFERENCES entities(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  FOREIGN KEY (latest_source_claim_id) REFERENCES claims(id),
  CHECK (from_place_id != to_place_id),
  CHECK (edge_type IN (
    'contains',
    'part_of',
    'near',
    'adjacent_to',
    'route_to',
    'north_of',
    'south_of',
    'east_of',
    'west_of',
    'northeast_of',
    'northwest_of',
    'southeast_of',
    'southwest_of',
    'upstream_of',
    'downstream_of',
    'inside',
    'entrance_to',
    'connects_to',
    'unknown_spatial'
  )),
  CHECK (status IN ('active', 'deprecated'))
);

CREATE TABLE IF NOT EXISTS place_edge_sources (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  edge_id TEXT NOT NULL,
  source_claim_id TEXT NOT NULL,
  chapter_index INTEGER NOT NULL,
  confidence REAL NOT NULL DEFAULT 0.5,
  created_at TEXT NOT NULL,
  FOREIGN KEY (edge_id) REFERENCES place_edges(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  UNIQUE(edge_id, source_claim_id)
);

CREATE TABLE IF NOT EXISTS map_layout_snapshots (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  max_chapter INTEGER NOT NULL,
  layout_version TEXT NOT NULL,
  layout_json TEXT NOT NULL,
  source_edge_hash TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS place_edge_conflicts (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  new_edge_claim_id TEXT NOT NULL,
  existing_edge_id TEXT,
  conflict_type TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  judge_output_json TEXT,
  status TEXT NOT NULL DEFAULT 'open',
  created_at TEXT NOT NULL,
  FOREIGN KEY (new_edge_claim_id) REFERENCES claims(id),
  FOREIGN KEY (existing_edge_id) REFERENCES place_edges(id),
  CHECK (conflict_type IN (
    'opposite_direction',
    'hierarchy_cycle',
    'duplicate_conflicting_direction',
    'impossible_containment',
    'unclear_reference',
    'low_evidence',
    'none'
  )),
  CHECK (status IN ('open', 'resolved', 'dismissed'))
);

CREATE TABLE IF NOT EXISTS entity_links (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  entity_a_id TEXT NOT NULL,
  entity_b_id TEXT NOT NULL,
  link_type TEXT NOT NULL,
  source_claim_id TEXT NOT NULL,
  confidence REAL NOT NULL DEFAULT 0.5,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (entity_a_id) REFERENCES entities(id),
  FOREIGN KEY (entity_b_id) REFERENCES entities(id),
  FOREIGN KEY (source_claim_id) REFERENCES claims(id),
  CHECK (entity_a_id != entity_b_id),
  CHECK (link_type IN (
    'organization_place_pair',
    'based_at',
    'headquarters_of',
    'related_entity'
  )),
  CHECK (status IN ('active', 'inactive', 'deprecated'))
);

CREATE INDEX idx_place_details_book_status
ON place_details(book_id, status);

CREATE INDEX idx_place_details_book_type
ON place_details(book_id, place_type, status);

CREATE INDEX idx_place_details_parent
ON place_details(book_id, parent_place_id);

CREATE INDEX idx_place_edges_book_status
ON place_edges(book_id, status);

CREATE INDEX idx_place_edges_from
ON place_edges(book_id, from_place_id, edge_type, status);

CREATE INDEX idx_place_edges_to
ON place_edges(book_id, to_place_id, edge_type, status);

CREATE INDEX idx_place_edges_source_claim
ON place_edges(source_claim_id);

CREATE UNIQUE INDEX idx_place_edges_active_unique
ON place_edges(book_id, from_place_id, to_place_id, edge_type, COALESCE(direction_hint, ''), status)
WHERE status = 'active';

CREATE INDEX idx_place_edge_sources_edge
ON place_edge_sources(book_id, edge_id);

CREATE INDEX idx_place_edge_sources_source_claim
ON place_edge_sources(source_claim_id);

CREATE INDEX idx_map_layout_snapshots_book_chapter
ON map_layout_snapshots(book_id, max_chapter, layout_version);

CREATE INDEX idx_map_layout_snapshots_edge_hash
ON map_layout_snapshots(book_id, source_edge_hash);

CREATE INDEX idx_place_edge_conflicts_book_status
ON place_edge_conflicts(book_id, status, conflict_type);

CREATE INDEX idx_place_edge_conflicts_claim
ON place_edge_conflicts(new_edge_claim_id);

CREATE INDEX idx_entity_links_book_pair
ON entity_links(book_id, entity_a_id, entity_b_id, link_type, status);

CREATE UNIQUE INDEX idx_entity_links_active_unique
ON entity_links(book_id, entity_a_id, entity_b_id, link_type, status)
WHERE status = 'active';

CREATE INDEX idx_entity_links_source_claim
ON entity_links(source_claim_id);
