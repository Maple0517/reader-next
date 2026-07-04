/**
 * V4 AI Book Memory types.
 * These map directly to the backend V4 API response shapes.
 */

// ─── Dimension State (from CharacterCardView.current_states) ───

export interface V4DimensionState {
  label: string
  value: string
  updatedChapter: number
  confidence: number
}

// ─── Character Card (entity-level) ───

export interface V4CharacterCardView {
  id: string
  name: string
  aliases: string[]
  summary: string | null
  importance: number
  firstSeenChapter: number
  lastSeenChapter: number
  currentStates: Record<string, V4DimensionState>
}

// ─── Character List Item (book-level) ───

export interface V4CharacterListItem {
  id: string
  name: string
  aliases: string[]
  summary: string | null
  importance: number
  firstSeenChapter: number
  lastSeenChapter: number
  visibilityScore: number
}

// ─── Memory Overview Response ───

export interface V4MemoryResponse {
  bookUrl: string
  bookName: string
  author: string
  maxReadChapter: number
  maxProcessedChapter: number
  processing: boolean
  characterCount: number
  relationshipCount: number
  knowledgeCount: number
}

// ─── Character List Response ───

export interface V4CharacterListView {
  characters: V4CharacterListItem[]
  total: number
}

// ─── Character Card Response ───

export interface V4CharacterResponse {
  character: V4CharacterCardView
  relationshipCount: number
  recentChanges: unknown[]
  evidenceAvailable: boolean
}

// ─── Chapter Memory Response ───

export interface V4ChapterMemoryResponse {
  chapterIndex: number
  chapterTitle: string | null
  summary: string | null
  keyPoints: string[]
  charactersInChapter: V4CharacterListItem[]
  relationshipCount: number
  knowledgeCount: number
}

// ─── Memory Status Response ───

export interface V4MemoryStatusResponse {
  maxReadChapter: number
  maxProcessedChapter: number
  processing: boolean
  lastError: string | null
}

// ─── Catchup Status Response ───

export type V4CatchupStatus = 'idle' | 'running' | 'cancel_requested' | 'cancelled' | 'completed' | 'failed'

export interface V4CatchupStatusResponse {
  status: V4CatchupStatus
  targetChapter: number | null
  currentChapter: number | null
  maxProcessedChapter: number
  lastError: string | null
}

// ─── Relationship Types (Phase 2) ───

export interface V4RelationshipNode {
  id: string
  name: string
  aliases: string[]
  importance: number
  firstSeenChapter: number
  lastSeenChapter: number
}

export interface V4RelationshipEdge {
  id: string
  sourceId: string
  targetId: string
  group: string
  label: string
  directionality: string
  currentState?: string
  strength: number
  polarity: string
  confidence: number
  importanceScore: number
  firstSeenChapter: number
  lastChangedChapter: number
  lastSeenChapter: number
  eventCount: number
  latestSourceClaimId: string
  evidenceAvailable: boolean
}

export interface V4RelationshipGraphView {
  nodes: V4RelationshipNode[]
  edges: V4RelationshipEdge[]
  groups: string[]
  total: number
}

export interface V4CharacterRelationshipsResponse {
  relationships: V4RelationshipEdge[]
  total: number
}

// ─── Identity Debug Types (Phase 3) ───

export interface V4IdentityLink {
  id: string
  entityAId: string
  entityBId: string
  linkType: string
  status: string
  confidence: number
  sourceClaimId: string
  redirectTargetId: string | null
}

export interface V4IdentityLinksResponse {
  identityLinks: V4IdentityLink[]
  total: number
}

export interface V4CharacterIdentityResponse {
  characterId: string
  redirectTargetId: string | null
  identityLinks: V4IdentityLink[]
  total: number
}

export interface V4MergeOperation {
  id: string
  survivorEntityId: string
  victimEntityId: string
  sourceIdentityLinkId: string
  reasonCode: string
  confidence: number
  status: string
  propertyConflictCount: number
  relationshipMergeCount: number
  resultJson: string | null
  createdAt: string
  completedAt: string | null
}

export interface V4MergeOperationsResponse {
  mergeOperations: V4MergeOperation[]
  total: number
}

// ─── Knowledge Types (Phase 4) ───

export type V4KnowledgeAssertionStatus = 'active' | 'rumor' | 'uncertain' | 'revised' | 'contradicted' | 'false_in_world'

export interface V4KnowledgeCategorySummary {
  category: string
  count: number
}

export interface V4KnowledgeCardListItem {
  id: string
  category: string
  topicKey: string
  topicDisplay: string
  currentSummary: string | null
  confidence: number
  importanceScore: number
  firstSeenChapter: number
  lastUpdatedChapter: number
  assertionCount: number
}

export interface V4KnowledgeOverviewView {
  cards: V4KnowledgeCardListItem[]
  categories: V4KnowledgeCategorySummary[]
  total: number
}

export interface V4KnowledgeCategoryView {
  category: string
  cards: V4KnowledgeCardListItem[]
  total: number
}

export interface V4KnowledgeReferencedEntityView {
  entityId: string
  displayName: string
  entityType: string
  role: string
}

export interface V4KnowledgeAssertionView {
  id: string
  assertionText: string
  status: V4KnowledgeAssertionStatus
  confidence: number
  importanceScore: number
  chapterIndex: number
  referencedEntities: V4KnowledgeReferencedEntityView[]
}

export interface V4KnowledgeCardDetailView {
  card: V4KnowledgeCardListItem
  assertionsByStatus: Record<string, V4KnowledgeAssertionView[]>
}
