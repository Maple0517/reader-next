import v4Http from './http'
import type {
  V4CharacterListView,
  V4CharacterResponse,
  V4ChapterMemoryResponse,
  V4CatchupStatusResponse,
  V4MemoryResponse,
  V4MemoryStatusResponse,
  V4RelationshipGraphView,
  V4CharacterRelationshipsResponse,
  V4IdentityLinksResponse,
  V4CharacterIdentityResponse,
  V4MergeOperationsResponse,
  V4KnowledgeOverviewView,
  V4KnowledgeCategoryView,
  V4KnowledgeCardDetailView,
  V4MapConflictsResponse,
  V4MapGraphView,
  V4MapLayoutView,
  V4MapOverviewView,
  V4MapPlacesResponse,
  V4PlaceDetailView,
  V4QualityActionRequest,
  V4QualityAuditFindingsResponse,
  V4QualityAuditRunRequest,
  V4QualityAuditRunsResponse,
  V4QualityCorrectionRequest,
  V4QualityCorrectionsResponse,
  V4QualityOverviewView,
  V4QualityPromptRegressionResultsResponse,
  V4QualityPromptRegressionRunRequest,
  V4QualityPromptRegressionRunsResponse,
  V4QualityQueryParams,
  V4QualityQuarantineResponse,
  V4QualityReprocessJobRequest,
  V4QualityReprocessJobsResponse,
} from '../../types/v4'

export function getV4Memory(bookUrl: string) {
  return v4Http.get<V4MemoryResponse>('/memory', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4Characters(bookUrl: string) {
  return v4Http.get<V4CharacterListView>('/characters', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4CharacterCard(bookUrl: string, characterId: string) {
  return v4Http.get<V4CharacterResponse>(`/characters/${encodeURIComponent(characterId)}`, { params: { bookUrl } }).then((r) => r.data)
}

export function getV4ChapterMemory(params: { bookUrl: string; chapterIndex: number }) {
  return v4Http.get<V4ChapterMemoryResponse>('/chapter-memory', { params }).then((r) => r.data)
}

export function getV4MemoryStatus(bookUrl: string) {
  return v4Http.get<V4MemoryStatusResponse>('/memory/status', { params: { bookUrl } }).then((r) => r.data)
}

export function resetV4Memory(bookUrl: string) {
  return v4Http.post<{ ok: boolean }>('/memory/reset', { bookUrl }).then((r) => r.data)
}

export function setV4Enabled(params: { bookUrl: string; enabled: boolean }) {
  return v4Http.post<{ enabled: boolean }>('/enabled', params).then((r) => r.data)
}

export function generateV4ChapterMemory(params: { bookUrl: string; chapterIndex: number }) {
  return v4Http.post<{ ok: boolean; chapterIndex: number; message: string }>('/chapter-memory/generate', params).then((r) => r.data)
}

export function startV4Catchup(params: { bookUrl: string; targetChapterIndex: number }) {
  return v4Http.post<{ ok: boolean; targetChapter: number }>('/catchup/start', params).then((r) => r.data)
}

export function getV4CatchupStatus(bookUrl: string) {
  return v4Http.get<V4CatchupStatusResponse>('/catchup/status', { params: { bookUrl } }).then((r) => r.data)
}

export function cancelV4Catchup(bookUrl: string) {
  return v4Http.post<{ ok: boolean }>('/catchup/cancel', { bookUrl }).then((r) => r.data)
}

export function getV4Relationships(bookUrl: string, params?: { group?: string; minImportance?: number }) {
  return v4Http.get<V4RelationshipGraphView>('/relationships', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function getV4CharacterRelationships(bookUrl: string, characterId: string) {
  return v4Http.get<V4CharacterRelationshipsResponse>(`/characters/${encodeURIComponent(characterId)}/relationships`, { params: { bookUrl } }).then((r) => r.data)
}

export function getV4IdentityLinks(bookUrl: string) {
  return v4Http.get<V4IdentityLinksResponse>('/identity-links', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4CharacterIdentity(bookUrl: string, characterId: string) {
  return v4Http.get<V4CharacterIdentityResponse>(`/characters/${encodeURIComponent(characterId)}/identity`, { params: { bookUrl } }).then((r) => r.data)
}

export function getV4MergeOperations(bookUrl: string) {
  return v4Http.get<V4MergeOperationsResponse>('/merge-operations', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4Knowledge(bookUrl: string) {
  return v4Http.get<V4KnowledgeOverviewView>('/knowledge', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4KnowledgeCategory(bookUrl: string, category: string) {
  return v4Http.get<V4KnowledgeCategoryView>(`/knowledge/categories/${encodeURIComponent(category)}`, { params: { bookUrl } }).then((r) => r.data)
}

export function getV4KnowledgeCard(bookUrl: string, cardId: string) {
  return v4Http.get<V4KnowledgeCardDetailView>(`/knowledge/cards/${encodeURIComponent(cardId)}`, { params: { bookUrl } }).then((r) => r.data)
}

export function getV4Map(bookUrl: string) {
  return v4Http.get<V4MapOverviewView>('/map', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4MapPlaces(bookUrl: string) {
  return v4Http.get<V4MapPlacesResponse>('/map/places', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4MapPlaceDetail(bookUrl: string, placeId: string) {
  return v4Http.get<V4PlaceDetailView>(`/map/places/${encodeURIComponent(placeId)}`, { params: { bookUrl } }).then((r) => r.data)
}

export function getV4MapGraph(bookUrl: string) {
  return v4Http.get<V4MapGraphView>('/map/graph', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4MapLayout(bookUrl: string) {
  return v4Http.get<V4MapLayoutView>('/map/layout', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4MapConflicts(bookUrl: string) {
  return v4Http.get<V4MapConflictsResponse>('/map/conflicts', { params: { bookUrl } }).then((r) => r.data)
}

export function getV4Quality(bookUrl: string, params?: V4QualityQueryParams) {
  return v4Http.get<V4QualityOverviewView>('/quality', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function getV4QualityQuarantine(bookUrl: string, params?: V4QualityQueryParams) {
  return v4Http.get<V4QualityQuarantineResponse>('/quality/quarantine', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function runV4QualityQuarantineAction(bookUrl: string, id: string, request: V4QualityActionRequest) {
  return v4Http.post(`/quality/quarantine/${encodeURIComponent(id)}/action`, { ...request, bookUrl }).then((r) => r.data)
}

export function getV4QualityAuditRuns(bookUrl: string, params?: V4QualityQueryParams) {
  return v4Http.get<V4QualityAuditRunsResponse>('/quality/audit-runs', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function createV4QualityAuditRun(bookUrl: string, request: V4QualityAuditRunRequest) {
  return v4Http.post('/quality/audit-runs', { ...request, bookUrl }).then((r) => r.data)
}

export function getV4QualityAuditFindings(bookUrl: string, params?: V4QualityQueryParams) {
  return v4Http.get<V4QualityAuditFindingsResponse>('/quality/audit-findings', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function runV4QualityAuditFindingAction(bookUrl: string, id: string, request: V4QualityActionRequest) {
  return v4Http.post(`/quality/audit-findings/${encodeURIComponent(id)}/action`, { ...request, bookUrl }).then((r) => r.data)
}

export function getV4QualityCorrections(bookUrl: string, params?: V4QualityQueryParams) {
  return v4Http.get<V4QualityCorrectionsResponse>('/quality/corrections', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function createV4QualityCorrection(bookUrl: string, request: V4QualityCorrectionRequest) {
  return v4Http.post('/quality/corrections', { ...request, bookUrl }).then((r) => r.data)
}

export function applyV4QualityCorrection(bookUrl: string, id: string) {
  return v4Http.post(`/quality/corrections/${encodeURIComponent(id)}/apply`, { bookUrl }).then((r) => r.data)
}

export function getV4QualityReprocessJobs(bookUrl: string, params?: V4QualityQueryParams) {
  return v4Http.get<V4QualityReprocessJobsResponse>('/quality/reprocess-jobs', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function createV4QualityReprocessJob(bookUrl: string, request: V4QualityReprocessJobRequest) {
  return v4Http.post('/quality/reprocess-jobs', { ...request, bookUrl }).then((r) => r.data)
}

export function cancelV4QualityReprocessJob(bookUrl: string, id: string) {
  return v4Http.post(`/quality/reprocess-jobs/${encodeURIComponent(id)}/cancel`, { bookUrl }).then((r) => r.data)
}

export function getV4QualityPromptRegressionRuns(bookUrl: string, params?: V4QualityQueryParams) {
  return v4Http.get<V4QualityPromptRegressionRunsResponse>('/quality/prompt-regression-runs', { params: { bookUrl, ...params } }).then((r) => r.data)
}

export function createV4QualityPromptRegressionRun(bookUrl: string, request: V4QualityPromptRegressionRunRequest) {
  return v4Http.post('/quality/prompt-regression-runs', { ...request, bookUrl }).then((r) => r.data)
}

export function getV4QualityPromptRegressionResults(bookUrl: string, runId: string) {
  return v4Http.get<V4QualityPromptRegressionResultsResponse>(`/quality/prompt-regression-runs/${encodeURIComponent(runId)}/results`, { params: { bookUrl } }).then((r) => r.data)
}
