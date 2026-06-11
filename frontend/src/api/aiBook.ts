import http from './http'
import type { AiBookAnyMemory } from '../types'

export function getAiBookMemory(bookUrl: string) {
  return http.post<AiBookAnyMemory | null>('/getAiBookMemory', { bookUrl }).then((r) => r.data)
}

export function saveAiBookMemory(memory: AiBookAnyMemory) {
  return http.post<AiBookAnyMemory>('/saveAiBookMemory', memory).then((r) => r.data)
}

export function deleteAiBookMemory(bookUrl: string) {
  return http.post<{ deleted: boolean }>('/deleteAiBookMemory', { bookUrl }).then((r) => r.data)
}
