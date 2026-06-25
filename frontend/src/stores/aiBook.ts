import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
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
  AiBookChapterMemoryViewModel,
  AiBookConfig,
  AiBookGenerationMode,
  AiBookMemory,
  AiBookMemoryViewModel,
  AiServerModelConfigResponse,
  Book,
  BookChapter,
} from '../types'
import { useAppStore } from './app'
import { getAiBookConfig, saveAiBookConfig } from '../utils/aiBookConfig'

type GenerationPhase = 'idle' | 'loading' | 'text' | 'error'
interface LoadServerModelConfigOptions {
  force?: boolean
}

export const useAiBookStore = defineStore('aiBook', () => {
  const appStore = useAppStore()
  const memoryView = ref<AiBookMemoryViewModel | null>(null)
  const chapterMemory = ref<AiBookChapterMemoryViewModel | null>(null)
  const catchupStatus = ref<AiBookCatchupStatus | null>(null)
  const loading = ref(false)
  const phase = ref<GenerationPhase>('idle')
  const statusText = ref('')
  const catchupPolling = ref(false)
  const updatingChapterKeys = new Set<string>()

  const username = computed(() => appStore.userInfo?.username || 'default')
  const config = ref<AiBookConfig>(getAiBookConfig(username.value))
  const serverModelConfig = ref<AiServerModelConfigResponse | null>(null)
  const memory = computed<AiBookMemory | null>(() => toLegacyMemory(memoryView.value))
  const isBusy = computed(() => (
    loading.value
    || phase.value === 'loading'
    || phase.value === 'text'
    || catchupPolling.value
  ))
  const canUseServerModel = computed(() => Boolean(serverModelConfig.value?.canUseServerModel))
  const isServerModelAdmin = computed(() => Boolean(serverModelConfig.value?.isAdmin))
  let serverModelConfigRequest: Promise<AiServerModelConfigResponse | null> | null = null

  async function loadServerModelConfig(options: LoadServerModelConfigOptions = {}) {
    if (!options.force && serverModelConfig.value) {
      return serverModelConfig.value
    }
    if (!options.force && serverModelConfigRequest) {
      return serverModelConfigRequest
    }

    const request = getAiModelConfig()
      .then((next) => {
        serverModelConfig.value = next
        return next
      })
      .catch(() => {
        serverModelConfig.value = null
        return null
      })
      .finally(() => {
        if (serverModelConfigRequest === request) {
          serverModelConfigRequest = null
        }
      })

    serverModelConfigRequest = request
    return request
  }

  function refreshConfig() {
    config.value = getAiBookConfig(username.value)
    return config.value
  }

  function persistConfig(next: AiBookConfig) {
    config.value = saveAiBookConfig(username.value, next)
    return config.value
  }

  async function load(book: Book) {
    loading.value = true
    statusText.value = '加载 AI 资料...'
    memoryView.value = null
    chapterMemory.value = null
    try {
      const response = await getAiBookMemory(book.bookUrl)
      applyMemoryResponse(response.memory)
      return memoryView.value
    } catch (error) {
      memoryView.value = null
      chapterMemory.value = null
      throw error
    } finally {
      loading.value = false
      if (phase.value === 'idle') {
        statusText.value = ''
      }
    }
  }

  async function loadChapterMemory(bookUrl: string, chapterIndex: number) {
    loading.value = true
    statusText.value = `加载第 ${chapterIndex + 1} 章 AI 资料...`
    chapterMemory.value = null
    try {
      const response = await getAiBookChapterMemory({ bookUrl, chapterIndex })
      applyChapterResponse(response.memory, response.chapter)
      return chapterMemory.value
    } catch (error) {
      chapterMemory.value = null
      throw error
    } finally {
      loading.value = false
      if (phase.value === 'idle') {
        statusText.value = ''
      }
    }
  }

  async function setEnabled(book: Book, enabled: boolean) {
    phase.value = 'loading'
    statusText.value = enabled ? '开启 AI 资料...' : '关闭 AI 资料...'
    try {
      const response = await setAiBookEnabled({ bookUrl: book.bookUrl, enabled })
      applyMemoryResponse(response.memory)
      return memoryView.value
    } finally {
      phase.value = 'idle'
      statusText.value = ''
    }
  }

  async function reset(book: Book) {
    phase.value = 'loading'
    statusText.value = '重置 AI 资料...'
    try {
      const response = await resetAiBookMemory(book.bookUrl)
      applyMemoryResponse(response.memory)
      chapterMemory.value = null
      return memoryView.value
    } finally {
      phase.value = 'idle'
      statusText.value = ''
    }
  }

  async function generateChapterMemory(params: { bookUrl: string; chapterIndex: number; mode?: AiBookGenerationMode }) {
    phase.value = 'text'
    statusText.value = `生成第 ${params.chapterIndex + 1} 章 AI 资料...`
    try {
      const response = await generateAiBookChapterMemory(params)
      applyChapterResponse(response.memory, response.chapter)
      phase.value = 'idle'
      statusText.value = ''
      return chapterMemory.value
    } catch (error) {
      setActionError((error as Error).message || 'AI 资料更新失败')
      throw error
    }
  }

  async function startCatchup(params: { bookUrl: string; targetChapterIndex?: number }) {
    catchupPolling.value = true
    try {
      catchupStatus.value = await startAiBookCatchup(params)
      return catchupStatus.value
    } finally {
      catchupPolling.value = false
    }
  }

  async function loadCatchupStatus(bookUrl: string) {
    catchupPolling.value = true
    try {
      catchupStatus.value = await getAiBookCatchupStatus(bookUrl)
      return catchupStatus.value
    } finally {
      catchupPolling.value = false
    }
  }

  async function cancelCatchup(bookUrl: string) {
    catchupPolling.value = true
    try {
      catchupStatus.value = await cancelAiBookCatchup(bookUrl)
      return catchupStatus.value
    } finally {
      catchupPolling.value = false
    }
  }

  async function autoUpdateCompletedChapter(params: {
    book: Book
    chapter: BookChapter
    chapterContent: string
    chapters?: BookChapter[]
  }) {
    const current = memoryView.value?.bookUrl === params.book.bookUrl
      ? memoryView.value
      : await load(params.book).catch(() => null)
    if (!current?.enabled) return null
    await generateChapterMemory({
      bookUrl: params.book.bookUrl,
      chapterIndex: params.chapter.index,
      mode: 'auto',
    }).catch(() => null)
    return memory.value
  }

  async function runChapterUpdate(params: {
    book: Book
    chapter: BookChapter
    chapterContent: string
    current?: AiBookMemory | null
    allowSkip?: boolean
    throwOnError?: boolean
    chapters?: BookChapter[]
  }): Promise<AiBookMemory> {
    const key = `${params.book.bookUrl}::${params.chapter.index}`
    if (updatingChapterKeys.has(key)) {
      return resolveLegacyMemoryFallback(memory.value, params.book, params.current)
    }
    updatingChapterKeys.add(key)
    try {
      await generateChapterMemory({
        bookUrl: params.book.bookUrl,
        chapterIndex: params.chapter.index,
        mode: params.allowSkip ? 'auto' : 'manual',
      })
      return resolveLegacyMemoryFallback(memory.value, params.book, params.current)
    } catch (error) {
      applyLocalError(params.chapter.index, params.chapter.title, (error as Error).message || 'AI 资料更新失败')
      if (params.throwOnError) {
        throw error
      }
      return resolveLegacyMemoryFallback(memory.value, params.book, params.current)
    } finally {
      updatingChapterKeys.delete(key)
    }
  }

  function applyMemoryResponse(next: AiBookMemoryViewModel) {
    memoryView.value = next
  }

  function applyChapterResponse(nextMemory: AiBookMemoryViewModel, nextChapter: AiBookChapterMemoryViewModel) {
    memoryView.value = nextMemory
    chapterMemory.value = nextChapter
  }

  function applyLocalError(chapterIndex: number, chapterTitle: string, message: string) {
    if (!memoryView.value) {
      return
    }
    memoryView.value = {
      ...memoryView.value,
      lastError: message,
      lastErrorChapterIndex: chapterIndex,
      lastErrorChapterTitle: chapterTitle,
    }
  }

  function setActionError(message: string) {
    phase.value = 'error'
    statusText.value = message
  }

  return {
    memory,
    memoryView,
    chapterMemory,
    catchupStatus,
    loading,
    phase,
    statusText,
    catchupPolling,
    isBusy,
    config,
    serverModelConfig,
    canUseServerModel,
    isServerModelAdmin,
    loadServerModelConfig,
    refreshConfig,
    persistConfig,
    load,
    loadChapterMemory,
    setEnabled,
    reset,
    generateChapterMemory,
    startCatchup,
    loadCatchupStatus,
    cancelCatchup,
    // temporary wrappers until Task 7 switches AiBookView/reader.ts to V3 actions directly
    autoUpdateCompletedChapter,
    runChapterUpdate,
  }
})

function toLegacyMemory(memory: AiBookMemoryViewModel | AiBookMemory | null | undefined): AiBookMemory | null {
  if (!memory) {
    return null
  }
  if ('worldview' in memory) {
    return memory
  }

  const charactersById = new Map(memory.characters.map((item) => [item.id, item.name]))
  const locationsById = new Map(memory.locations.map((item) => [item.id, item.name]))

  return {
    bookUrl: memory.bookUrl,
    bookName: memory.bookName || undefined,
    author: memory.author || undefined,
    enabled: memory.enabled,
    processedChapterIndex: memory.processedChapterIndex ?? undefined,
    processedChapterTitle: memory.processedChapterTitle || undefined,
    updatedAt: memory.updatedAt,
    summary: memory.summary.current,
    worldview: memory.knowledgeFacts.map((item) => ({
      title: item.title,
      content: item.content,
      category: item.category,
      confidence: item.confidence,
      importance: item.importance,
      evidence: item.evidence,
    })),
    characters: memory.characters.map((item) => ({
      name: item.name,
      aliases: item.aliases,
      status: '',
      description: item.description || undefined,
      lastSeenChapter: formatChapter(item.lastSeenChapterIndex ?? undefined),
      importance: item.importance,
      evidence: item.evidence,
    })),
    relationships: memory.relationships.map((item) => ({
      source: charactersById.get(item.sourceCharacterId) || item.sourceCharacterId,
      target: charactersById.get(item.targetCharacterId) || item.targetCharacterId,
      relation: item.label,
      status: item.status,
      description: item.summary,
      evidence: item.evidence,
    })),
    locations: memory.locations.map((item) => ({
      name: item.name,
      kind: item.kind,
      parentName: item.parentLocationId ? locationsById.get(item.parentLocationId) : undefined,
      description: item.description,
      status: item.currentStatus || undefined,
      relatedCharacters: [],
      firstSeenChapter: formatChapter(item.firstSeenChapterIndex ?? undefined),
      importance: item.importance,
      evidence: item.evidence,
    })),
    map: memory.map
      ? {
          imageUrl: memory.map.renderArtifacts?.imageUrl || undefined,
          prompt: undefined,
          updatedAt: memory.map.renderArtifacts?.updatedAt ?? undefined,
          sourceChapterIndex: memory.map.renderArtifacts?.chapterIndex ?? undefined,
          fallback: memory.map.renderArtifacts?.imageUrl ? undefined : 'relationship-graph',
          fallbackReason: memory.map.state?.dirty ? '地图待重新生成' : undefined,
        }
      : null,
    mapDirty: Boolean(memory.map?.state?.dirty),
    lastError: memory.lastError || undefined,
    lastErrorChapterIndex: memory.lastErrorChapterIndex ?? undefined,
    lastErrorChapterTitle: memory.lastErrorChapterTitle || undefined,
    cleanup: memory.cleanup,
    catchupStats: memory.catchupStats,
  }
}

function formatChapter(index?: number) {
  if (typeof index !== 'number') {
    return undefined
  }
  return `第${index + 1}章`
}

function resolveLegacyMemoryFallback(next: AiBookMemory | null, book: Book, current?: AiBookMemory | null): AiBookMemory {
  return toLegacyMemory(next)
    || toLegacyMemory(current)
    || toLegacyMemory({
      bookUrl: book.bookUrl,
      bookName: book.name,
      author: book.author,
      enabled: false,
      updatedAt: Date.now(),
      summary: {
        current: '',
        recentChanges: [],
        openQuestions: [],
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
    })!
}
