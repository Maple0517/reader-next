import v4Http from './http'
import type {
  V4CharacterListView,
  V4CharacterResponse,
  V4ChapterMemoryResponse,
  V4CatchupStatusResponse,
  V4MemoryResponse,
  V4MemoryStatusResponse,
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
