import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAiBookStore } from './aiBook'
import { getAiModelConfig } from '../api/aiModel'
import { getAiBookMemory, saveAiBookMemory, deleteAiBookMemory } from '../api/aiBook'
import { requestAiBookMapImage, requestAiBookMemoryUpdate, uploadGeneratedMap } from '../utils/aiBookGeneration'
import type { AiBookMemoryV2, AiServerModelConfigResponse, Book, BookChapter } from '../types'

vi.mock('../api/aiModel', () => ({
  getAiModelConfig: vi.fn(),
}))

vi.mock('../api/aiBook', () => ({
  getAiBookMemory: vi.fn(),
  saveAiBookMemory: vi.fn(),
  deleteAiBookMemory: vi.fn(),
}))

vi.mock('../utils/aiBookGeneration', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../utils/aiBookGeneration')>()
  return {
    ...actual,
    requestAiBookMemoryUpdate: vi.fn(),
    requestAiBookMapImage: vi.fn(),
    uploadGeneratedMap: vi.fn(),
  }
})

const getAiModelConfigMock = vi.mocked(getAiModelConfig)
const getAiBookMemoryMock = vi.mocked(getAiBookMemory)
const saveAiBookMemoryMock = vi.mocked(saveAiBookMemory)
const deleteAiBookMemoryMock = vi.mocked(deleteAiBookMemory)
const requestAiBookMemoryUpdateMock = vi.mocked(requestAiBookMemoryUpdate)
const requestAiBookMapImageMock = vi.mocked(requestAiBookMapImage)
const uploadGeneratedMapMock = vi.mocked(uploadGeneratedMap)

describe('aiBook store server model config', () => {
  beforeEach(() => {
    installLocalStorage()
    vi.stubGlobal('window', { setTimeout: vi.fn() })
    setActivePinia(createPinia())
    getAiModelConfigMock.mockReset()
    getAiBookMemoryMock.mockReset()
    saveAiBookMemoryMock.mockReset()
    deleteAiBookMemoryMock.mockReset()
    requestAiBookMemoryUpdateMock.mockReset()
    requestAiBookMapImageMock.mockReset()
    uploadGeneratedMapMock.mockReset()
  })

  it('reuses the loaded server model config for repeated checks', async () => {
    const response = createServerModelConfigResponse()
    getAiModelConfigMock.mockResolvedValue(response)
    const store = useAiBookStore()

    await expect(store.loadServerModelConfig()).resolves.toEqual(response)
    await expect(store.loadServerModelConfig()).resolves.toEqual(response)

    expect(getAiModelConfigMock).toHaveBeenCalledTimes(1)
  })

  it('throws chapter update errors when requested so batch updates stop at the failed chapter', async () => {
    const store = useAiBookStore()
    const book = createBook()
    const chapter = createChapter(2, '第三章')
    const current = createMemory(book)
    requestAiBookMemoryUpdateMock.mockRejectedValue(new Error('模型炸了'))
    saveAiBookMemoryMock.mockImplementation(async (memory) => memory)

    await expect(store.runChapterUpdate({
      book,
      chapter,
      chapterContent: '正文',
      current,
      throwOnError: true,
    })).rejects.toThrow('模型炸了')

    expect(saveAiBookMemoryMock).toHaveBeenCalledWith(expect.objectContaining({
      bookUrl: book.bookUrl,
      lastError: '模型炸了',
      lastErrorChapterIndex: 2,
      lastErrorChapterTitle: '第三章',
    }))
  })

  it('does not auto redraw maps during chapter text updates', async () => {
    const store = useAiBookStore()
    const book = createBook()
    const chapter = createChapter(3, '第四章')
    const current = createMemory(book)
    const updated: AiBookMemoryV2 = {
      ...current,
      processedChapterIndex: 3,
      processedChapterTitle: '第四章',
      mapState: {
        ...current.mapState,
        dirty: true,
        reason: '新增北境学院',
        sourceChapterIndex: 3,
        mapPrompt: '绘制北境学院',
      },
    }
    requestAiBookMemoryUpdateMock.mockResolvedValue({
      memory: updated,
      shouldRegenerateMap: true,
      mapPrompt: '绘制北境学院',
    })
    requestAiBookMapImageMock.mockResolvedValue({ b64Json: 'abc', imageUrl: undefined })
    uploadGeneratedMapMock.mockResolvedValue('/assets/map.png')
    saveAiBookMemoryMock.mockImplementation(async (memory) => memory)

    const result = await store.runChapterUpdate({
      book,
      chapter,
      chapterContent: '正文',
      current,
    })

    expect(result).toMatchObject({
      processedChapterIndex: 3,
      mapState: expect.objectContaining({
        dirty: true,
        mapPrompt: '绘制北境学院',
      }),
    })
    expect(requestAiBookMapImageMock).not.toHaveBeenCalled()
    expect(uploadGeneratedMapMock).not.toHaveBeenCalled()
    expect(saveAiBookMemoryMock).toHaveBeenCalledTimes(1)
  })
})

function createBook(): Book {
  return { name: '山海旧事', author: '佚名', bookUrl: 'book-1', origin: 'source-1' }
}

function createChapter(index: number, title: string): BookChapter {
  return { index, title, url: `chapter-${index}` }
}

function createMemory(book: Book): AiBookMemoryV2 {
  return {
    schemaVersion: 2,
    bookUrl: book.bookUrl,
    bookName: book.name,
    author: book.author,
    enabled: true,
    updatedAt: 0,
    summary: { current: '', recentChanges: [], openQuestions: [] },
    chapterDigests: [],
    arcs: [],
    worldFacts: [],
    characters: [],
    relationships: [],
    locations: [],
    mapState: { dirty: false, nodes: [], edges: [] },
    renderArtifacts: {},
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
