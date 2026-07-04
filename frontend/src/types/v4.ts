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

// ─── Map Types (Phase 5) ───

export interface V4PlaceSummaryView {
  id: string
  name: string
  placeType: string
}

export interface V4LinkedOrganizationView {
  id: string
  name: string
  linkType: string
}

export interface V4PlaceDetailView {
  id: string
  name: string
  placeType: string
  linkedOrganizations: V4LinkedOrganizationView[]
}

export interface V4PlaceHierarchyNode {
  placeId: string
  name: string
  placeType: string
  children: V4PlaceHierarchyNode[]
}

export interface V4MapOverviewView {
  placeCount: number
  activeEdgeCount: number
  conflictCount: number
  topPlaces: V4PlaceSummaryView[]
}

export interface V4MapPlacesResponse {
  places: V4PlaceSummaryView[]
  hierarchy: V4PlaceHierarchyNode[]
  total: number
}

export interface V4MapGraphNode {
  placeId: string
  label: string
  placeType: string
}

export interface V4MapGraphEdge {
  edgeId: string
  fromPlaceId: string
  toPlaceId: string
  edgeType: string
}

export interface V4MapLayoutNode {
  placeId: string
  x: number
  y: number
  label: string
  placeType: string
}

export interface V4MapLayoutView {
  nodes: V4MapLayoutNode[]
  edges: V4MapGraphEdge[]
  warnings: string[]
}

export interface V4MapGraphView {
  nodes: V4MapGraphNode[]
  edges: V4MapGraphEdge[]
  layout: V4MapLayoutView | null
  warnings: string[]
}

export interface V4MapConflictView {
  id: string
  newEdgeClaimId: string
  existingEdgeId: string | null
  conflictType: string
  reasonCode: string
  status: string
  createdAt: string
}

export interface V4MapConflictsResponse {
  conflicts: V4MapConflictView[]
  total: number
}

// ─── Quality Types (Phase 6) ───

export type V4QualityStatus = 'open' | 'accepted' | 'rejected' | 'converted_to_correction' | 'resolved' | string

export interface V4QualityQueryParams {
  status?: string
  reasonCode?: string
  claimType?: string
  auditType?: string
  findingType?: string
  severity?: string
  targetType?: string
  correctionType?: string
  targetId?: string
  mode?: string
  fixtureSet?: string
  limit?: number
}

export interface V4QualityMetricView {
  value?: number
  measuredAt?: string
  details?: unknown
}

export interface V4QualityClaimView {
  id: string
  claimType: string
  status: string
  chapterIndex: number
  valueJson?: unknown
}

export interface V4QualitySourceSpanView {
  id: string
  chapterIndex: number
  textExcerpt: string
}

export interface V4QualityAiRunView {
  id: string
  runType: string
  promptVersion?: string
}

export interface V4QualityQuarantineView {
  id: string
  claimId: string
  reasonCode: string
  reasonText?: string | null
  suggestedAction: string
  status: V4QualityStatus
  priority: number
  claim?: V4QualityClaimView | null
  sourceSpans?: V4QualitySourceSpanView[]
  aiRun?: V4QualityAiRunView | null
}

export interface V4QualityAuditRunView {
  id: string
  auditType: string
  status: string
  summaryJson?: unknown
  error?: string | null
}

export interface V4QualityAuditFindingView {
  id: string
  findingType: string
  severity: string
  targetType: string
  targetId: string
  relatedTargetType?: string | null
  relatedTargetId?: string | null
  reasonCode: string
  reasonText?: string | null
  evidenceJson?: unknown
  suggestedAction: string
  status: V4QualityStatus
}

export interface V4QualityCorrectionView {
  id: string
  targetType: string
  targetId: string
  correctionType: string
  status: string
  source: string
  error?: string | null
}

export interface V4QualityReprocessJobView {
  id: string
  scopeType: string
  mode: string
  status: string
  dryRun: boolean
  reason?: string | null
  error?: string | null
}

export interface V4QualityPromptRegressionRunView {
  id: string
  fixtureSet: string
  status: string
  summaryJson?: unknown
  error?: string | null
}

export interface V4QualityPromptRegressionResultView {
  id: string
  caseId: string
  caseName: string
  domain: string
  expectedJson: unknown
  actualJson: unknown
  pass: boolean
  diffJson: unknown
}

export interface V4QualityOverviewView {
  bookUrl: string
  qualityMetrics: Record<string, number | V4QualityMetricView>
  quarantine?: V4QualityQuarantineView[]
  quarantinedClaims?: V4QualityQuarantineView[]
  auditRuns: V4QualityAuditRunView[]
  findings: V4QualityAuditFindingView[]
  corrections: V4QualityCorrectionView[]
  reprocessJobs: V4QualityReprocessJobView[]
  promptRegressionRuns: V4QualityPromptRegressionRunView[]
}

export interface V4QualityQuarantineResponse {
  quarantine: V4QualityQuarantineView[]
  total: number
}

export interface V4QualityAuditRunsResponse {
  auditRuns: V4QualityAuditRunView[]
  total: number
}

export interface V4QualityAuditFindingsResponse {
  findings: V4QualityAuditFindingView[]
  total: number
}

export interface V4QualityCorrectionsResponse {
  corrections: V4QualityCorrectionView[]
  total: number
}

export interface V4QualityReprocessJobsResponse {
  reprocessJobs: V4QualityReprocessJobView[]
  total: number
}

export interface V4QualityPromptRegressionRunsResponse {
  promptRegressionRuns: V4QualityPromptRegressionRunView[]
  total: number
}

export interface V4QualityPromptRegressionResultsResponse {
  results: V4QualityPromptRegressionResultView[]
  total: number
}

export interface V4QualityActionRequest {
  action: string
  actor?: string
  note?: string
  reasonCode?: string
  suggestedAction?: string
}

export interface V4QualityAuditRunRequest {
  auditType: string
  scopeJson?: unknown
}

export interface V4QualityCorrectionRequest {
  targetType: string
  targetId: string
  correctionType: string
  correctionJson: unknown
  source: string
  sourceClaimId?: string
  sourceSpanId?: string
  createdBy?: string
}

export interface V4QualityReprocessJobRequest {
  scopeType: string
  scopeJson: unknown
  mode: string
  requestedBy?: string
  reason?: string
  dryRun: boolean
  promptVersion?: string
  schemaVersion?: string
}

export interface V4QualityPromptRegressionRunRequest {
  promptVersion: string
  schemaVersion: string
  model: string
  fixtureSet: string
}
