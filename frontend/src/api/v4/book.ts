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
