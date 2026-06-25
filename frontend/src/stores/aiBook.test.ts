import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAiBookStore } from './aiBook'
import { getAiModelConfig } from '../api/aiModel'
import {
  cancelAiBookCatchup,
  generateAiBookChapterMemory,
  getAiBookCatchupStatus,
  getAiBookChapterMemory,
  getAiBookMemory,
  resetAiBookMemory,
  setAiBookEnabled,
  startAiBookCatchup,
} from '../api/aiBook'
import type {
  AiBookCatchupStatus,
  AiBookChapterMemoryViewResponse,
  AiBookMemoryViewResponse,
  AiServerModelConfigResponse,
  Book,
} from '../types'

vi.mock('../api/aiModel', () => ({
  getAiModelConfig: vi.fn(),
}))

vi.mock('../api/aiBook', () => ({
  getAiBookMemory: vi.fn(),
  getAiBookChapterMemory: vi.fn(),
  resetAiBookMemory: vi.fn(),
  setAiBookEnabled: vi.fn(),
  generateAiBookChapterMemory: vi.fn(),
  startAiBookCatchup: vi.fn(),
  getAiBookCatchupStatus: vi.fn(),
  cancelAiBookCatchup: vi.fn(),
}))

const getAiModelConfigMock = vi.mocked(getAiModelConfig)
const getAiBookMemoryMock = vi.mocked(getAiBookMemory)
const getAiBookChapterMemoryMock = vi.mocked(getAiBookChapterMemory)
const resetAiBookMemoryMock = vi.mocked(resetAiBookMemory)
const setAiBookEnabledMock = vi.mocked(setAiBookEnabled)
const generateAiBookChapterMemoryMock = vi.mocked(generateAiBookChapterMemory)
const startAiBookCatchupMock = vi.mocked(startAiBookCatchup)
const getAiBookCatchupStatusMock = vi.mocked(getAiBookCatchupStatus)
const cancelAiBookCatchupMock = vi.mocked(cancelAiBookCatchup)

describe('aiBook store v3', () => {
  beforeEach(() => {
    installLocalStorage()
    setActivePinia(createPinia())
    getAiModelConfigMock.mockReset()
    getAiBookMemoryMock.mockReset()
    getAiBookChapterMemoryMock.mockReset()
    resetAiBookMemoryMock.mockReset()
    setAiBookEnabledMock.mockReset()
    generateAiBookChapterMemoryMock.mockReset()
    startAiBookCatchupMock.mockReset()
    getAiBookCatchupStatusMock.mockReset()
    cancelAiBookCatchupMock.mockReset()
  })

  it('reuses the loaded server model config for repeated checks', async () => {
    const response = createServerModelConfigResponse()
    getAiModelConfigMock.mockResolvedValue(response)
    const store = useAiBookStore()

    await expect(store.loadServerModelConfig()).resolves.toEqual(response)
    await expect(store.loadServerModelConfig()).resolves.toEqual(response)

    expect(getAiModelConfigMock).toHaveBeenCalledTimes(1)
  })

  it('aiBook_store_loads_view_model_and_calls_generate_action', async () => {
    const book = createBook()
    const memoryResponse = createMemoryResponse(book)
    const chapterResponse = createChapterResponse(book)
    const generatedResponse = {
      ...chapterResponse,
      chapter: {
        ...chapterResponse.chapter,
        generationStatus: 'generated',
      },
    }
    getAiBookMemoryMock.mockResolvedValue(memoryResponse)
    getAiBookChapterMemoryMock.mockResolvedValue(chapterResponse)
    generateAiBookChapterMemoryMock.mockResolvedValue(generatedResponse)
    const store = useAiBookStore()

    const loaded = await store.load(book)

    expect(loaded).toEqual(memoryResponse.memory)
    expect(store.memoryView).toEqual(memoryResponse.memory)

    const chapter = await store.loadChapterMemory(book.bookUrl, 3)

    expect(chapter).toEqual(chapterResponse.chapter)
    expect(store.chapterMemory).toEqual(chapterResponse.chapter)

    const generated = await store.generateChapterMemory({ bookUrl: book.bookUrl, chapterIndex: 3, mode: 'auto' })

    expect(generated).toEqual(generatedResponse.chapter)
    expect(store.memoryView).toEqual(generatedResponse.memory)
    expect(store.chapterMemory).toEqual(generatedResponse.chapter)
    expect(getAiBookMemoryMock).toHaveBeenCalledWith(book.bookUrl)
    expect(getAiBookChapterMemoryMock).toHaveBeenCalledWith({ bookUrl: book.bookUrl, chapterIndex: 3 })
    expect(generateAiBookChapterMemoryMock).toHaveBeenCalledWith({ bookUrl: book.bookUrl, chapterIndex: 3, mode: 'auto' })
  })

  it('updates enabled reset and catchup state via v3 actions', async () => {
    const book = createBook()
    const memoryResponse = createMemoryResponse(book)
    const enabledResponse = {
      memory: {
        ...memoryResponse.memory,
        enabled: true,
      },
    }
    const resetResponse = createMemoryResponse(book)
    const catchupStatus = createCatchupStatus()
    const cancelStatus = { ...catchupStatus, status: 'canceled' as const }
    setAiBookEnabledMock.mockResolvedValue(enabledResponse)
    resetAiBookMemoryMock.mockResolvedValue(resetResponse)
    startAiBookCatchupMock.mockResolvedValue(catchupStatus)
    getAiBookCatchupStatusMock.mockResolvedValue(catchupStatus)
    cancelAiBookCatchupMock.mockResolvedValue(cancelStatus)
    const store = useAiBookStore()

    await store.setEnabled(book, true)
    await store.reset(book)
    await store.startCatchup({ bookUrl: book.bookUrl, targetChapterIndex: 9 })
    await store.loadCatchupStatus(book.bookUrl)
    await store.cancelCatchup(book.bookUrl)

    expect(setAiBookEnabledMock).toHaveBeenCalledWith({ bookUrl: book.bookUrl, enabled: true })
    expect(resetAiBookMemoryMock).toHaveBeenCalledWith(book.bookUrl)
    expect(startAiBookCatchupMock).toHaveBeenCalledWith({ bookUrl: book.bookUrl, targetChapterIndex: 9 })
    expect(getAiBookCatchupStatusMock).toHaveBeenCalledWith(book.bookUrl)
    expect(cancelAiBookCatchupMock).toHaveBeenCalledWith(book.bookUrl)
    expect(store.catchupStatus).toEqual(cancelStatus)
  })

  it('preserves legacy non-null runChapterUpdate contract before Task 7', async () => {
    const book = createBook()
    const chapter = { index: 3, title: '第四章', url: 'chapter-3' }
    const generatedResponse = createChapterResponse(book)
    generateAiBookChapterMemoryMock.mockResolvedValue(generatedResponse)
    const store = useAiBookStore()

    const result = await store.runChapterUpdate({
      book,
      chapter,
      chapterContent: '正文',
      throwOnError: true,
    })

    expect(result).not.toBeNull()
    expect(result).toMatchObject({
      bookUrl: book.bookUrl,
      summary: '摘要',
      worldview: [],
    })
  })


  it('allows retry after chapter generation failure', async () => {
    const book = createBook()
    const generatedResponse = createChapterResponse(book)
    const failure = new Error('生成失败')
    generateAiBookChapterMemoryMock
      .mockRejectedValueOnce(failure)
      .mockResolvedValueOnce(generatedResponse)
    const store = useAiBookStore()

    await expect(store.generateChapterMemory({ bookUrl: book.bookUrl, chapterIndex: 3, mode: 'manual' }))
      .rejects
      .toThrow('生成失败')

    expect(store.phase).toBe('error')
    expect(store.statusText).toBe('生成失败')
    expect(store.isBusy).toBe(false)

    await expect(store.generateChapterMemory({ bookUrl: book.bookUrl, chapterIndex: 3, mode: 'manual' }))
      .resolves
      .toEqual(generatedResponse.chapter)

    expect(generateAiBookChapterMemoryMock).toHaveBeenCalledTimes(2)
    expect(store.phase).toBe('idle')
    expect(store.statusText).toBe('')
  })
})

function createBook(): Book {
  return { name: '山海旧事', author: '佚名', bookUrl: 'book-1', origin: 'source-1' }
}

function createMemoryResponse(book: Book): AiBookMemoryViewResponse {
  return {
    memory: {
      bookUrl: book.bookUrl,
      bookName: book.name,
      author: book.author,
      enabled: false,
      processedChapterIndex: 2,
      processedChapterTitle: '第三章',
      updatedAt: 1,
      summary: {
        current: '摘要',
        recentChanges: ['变化'],
        openQuestions: ['问题'],
      },
      characters: [],
      relationships: [],
      knowledgeFacts: [],
      locations: [],
      map: null,
      cleanup: {
        droppedFactsCount: 0,
        droppedByReason: {},
        oldSchemaBackedUp: false,
      },
      catchupStats: null,
      lastError: null,
      lastErrorChapterIndex: null,
      lastErrorChapterTitle: null,
    },
  }
}

function createChapterResponse(book: Book): AiBookChapterMemoryViewResponse {
  return {
    memory: createMemoryResponse(book).memory,
    chapter: {
      bookUrl: book.bookUrl,
      chapterIndex: 3,
      chapterTitle: '第四章',
      digest: null,
      characters: [],
      relationships: [],
      knowledgeFacts: [],
      locations: [],
      generationStatus: 'cached',
      lastError: null,
    },
  }
}

function createCatchupStatus(): AiBookCatchupStatus {
  return {
    status: 'running',
    bookUrl: 'book-1',
    currentStage: 'patch',
    startChapterIndex: 3,
    targetChapterIndex: 9,
    currentChapterIndex: 4,
    currentChapterTitle: '第五章',
    processedChapterIndex: 3,
    processedChapterTitle: '第四章',
    totalChapters: 6,
    completedChapters: 1,
    updatedAt: 10,
    error: null,
    stats: {
      totalModelCalls: 1,
      digestCalls: 1,
      patchCalls: 0,
      skippedPatchChapters: 0,
      totalInputBytes: 120,
      totalOutputBytes: 45,
      lastCallLatencyMs: 1000,
      averageCallLatencyMs: 1000,
      lastChapterIndex: 3,
      updatedAt: 10,
    },
  }
}

function createServerModelConfigResponse(): AiServerModelConfigResponse {
  return {
    canUseServerModel: true,
    isAdmin: false,
    config: {
      text: {
        enabled: true,
        baseUrl: 'https://api.example.com',
        apiKey: '',
        model: 'gpt-4o-mini',
        path: '/v1/chat/completions',
        useFullUrl: false,
      },
      image: {
        enabled: true,
        baseUrl: 'https://api.example.com',
        apiKey: '',
        model: 'gpt-image-1',
        path: '/v1/images/generations',
        imageSize: '1024x1024',
        useFullUrl: false,
      },
      speech: {
        enabled: true,
        baseUrl: 'https://api.example.com',
        apiKey: '',
        model: 'gpt-4o-mini-tts',
        path: '/v1/audio/speech',
        voice: 'alloy',
        responseFormat: 'mp3',
        useFullUrl: false,
      },
    },
  }
}

function installLocalStorage() {
  const memory = new Map<string, string>()
  Object.defineProperty(globalThis, 'localStorage', {
    value: {
      getItem: (key: string) => memory.get(key) || null,
      setItem: (key: string, value: string) => memory.set(key, value),
      removeItem: (key: string) => memory.delete(key),
      clear: () => memory.clear(),
    },
    configurable: true,
  })
}
