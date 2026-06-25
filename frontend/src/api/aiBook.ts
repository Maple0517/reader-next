import http from './http'
import type {
  AiBookCatchupStatus,
  AiBookChapterMemoryViewResponse,
  AiBookGenerationMode,
  AiBookMemoryViewResponse,
} from '../types'

export function getAiBookMemory(bookUrl: string) {
  return http.get<AiBookMemoryViewResponse>('/aiBook/memory', { params: { bookUrl } }).then((r) => r.data)
}

export function getAiBookChapterMemory(params: { bookUrl: string; chapterIndex: number }) {
  return http.get<AiBookChapterMemoryViewResponse>('/aiBook/chapterMemory', { params }).then((r) => r.data)
}

export function resetAiBookMemory(bookUrl: string) {
  return http.post<AiBookMemoryViewResponse>('/aiBook/memory/reset', { bookUrl }).then((r) => r.data)
}

export function setAiBookEnabled(params: { bookUrl: string; enabled: boolean }) {
  return http.post<AiBookMemoryViewResponse>('/aiBook/enabled', params).then((r) => r.data)
}

export function generateAiBookChapterMemory(params: { bookUrl: string; chapterIndex: number; mode?: AiBookGenerationMode }) {
  return http.post<AiBookChapterMemoryViewResponse>('/aiBook/chapterMemory/generate', params).then((r) => r.data)
}

export function startAiBookCatchup(params: { bookUrl: string; targetChapterIndex?: number }) {
  return http.post<AiBookCatchupStatus>('/aiBook/catchup/start', params).then((r) => r.data)
}

export function getAiBookCatchupStatus(bookUrl: string) {
  return http.get<AiBookCatchupStatus>('/aiBook/catchup/status', { params: { bookUrl } }).then((r) => r.data)
}

export function cancelAiBookCatchup(bookUrl: string) {
  return http.post<AiBookCatchupStatus>('/aiBook/catchup/cancel', { bookUrl }).then((r) => r.data)
}
