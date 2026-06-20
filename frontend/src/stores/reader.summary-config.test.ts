import { describe, expect, it, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useReaderStore } from './reader'

describe('reader summary display config', () => {
  beforeEach(() => {
    const storage = new Map<string, string>()
    vi.stubGlobal('localStorage', {
      getItem: vi.fn((key: string) => storage.get(key) ?? null),
      setItem: vi.fn((key: string, value: string) => storage.set(key, value)),
      removeItem: vi.fn((key: string) => storage.delete(key)),
      clear: vi.fn(() => storage.clear()),
    })
    localStorage.clear()
    setActivePinia(createPinia())
  })

  it('defaults key points to card style', () => {
    const store = useReaderStore()
    expect(store.config.chapterSummaryKeyPointStyle).toBe('card')
  })
})
