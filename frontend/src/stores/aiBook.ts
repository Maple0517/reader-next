import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { getAiModelConfig } from '../api/aiModel'
import { deleteAiBookMemory, getAiBookMemory, saveAiBookMemory } from '../api/aiBook'
import type { AiBookAnyMemory, AiServerModelConfigResponse, Book, BookChapter } from '../types'
import { useAppStore } from './app'
import { getAiBookConfig, saveAiBookConfig } from '../utils/aiBookConfig'
import type { AiBookConfig } from '../types'
import {
  applyMapFallbackToMemory,
  applyMapToMemory,
  requestAiBookMapImage,
  requestAiBookMemoryUpdate,
  shouldRunAiBookAutoUpdate,
  uploadGeneratedMap,
} from '../utils/aiBookGeneration'
import { shouldSkipAiBookChapter } from '../utils/aiBookChapterFilter'
import {
  applyMapArtifactToMemoryV2,
  applyMapFallbackToMemoryV2,
  createEmptyAiBookMemoryV2,
  isAiBookMemoryV2,
  toAiBookDisplayMemory,
} from '../utils/aiBookV2'

type GenerationPhase = 'idle' | 'loading' | 'text' | 'map' | 'saving' | 'error'
interface LoadServerModelConfigOptions {
  force?: boolean
}

export const useAiBookStore = defineStore('aiBook', () => {
  const appStore = useAppStore()
  const memory = ref<AiBookAnyMemory | null>(null)
  const loading = ref(false)
  const phase = ref<GenerationPhase>('idle')
  const statusText = ref('')
  const updatingChapterKeys = new Set<string>()

  const username = computed(() => appStore.userInfo?.username || 'default')
  const config = ref<AiBookConfig>(getAiBookConfig(username.value))
  const serverModelConfig = ref<AiServerModelConfigResponse | null>(null)
  const isBusy = computed(() => loading.value || phase.value !== 'idle')
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
      .then((config) => {
        serverModelConfig.value = config
        return config
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
    try {
      const saved = await getAiBookMemory(book.bookUrl)
      memory.value = saved || createEmptyAiBookMemoryV2(book)
      return memory.value
    } finally {
      loading.value = false
    }
  }

  async function save(next: AiBookAnyMemory) {
    phase.value = 'saving'
    statusText.value = '保存 AI 资料...'
    try {
      memory.value = await saveAiBookMemory(next)
      return memory.value
    } finally {
      phase.value = 'idle'
      statusText.value = ''
    }
  }

  async function setEnabled(book: Book, enabled: boolean) {
    const current = memory.value?.bookUrl === book.bookUrl ? memory.value : await load(book)
    return save({
      ...current,
      bookUrl: book.bookUrl,
      bookName: book.name,
      author: book.author,
      enabled,
      updatedAt: Date.now(),
    })
  }

  async function reset(book: Book) {
    await deleteAiBookMemory(book.bookUrl)
    memory.value = createEmptyAiBookMemoryV2(book)
    phase.value = 'idle'
    statusText.value = ''
    return memory.value
  }

  async function autoUpdateCompletedChapter(params: {
    book: Book
    chapter: BookChapter
    chapterContent: string
    chapters?: BookChapter[]
  }) {
    const current = memory.value?.bookUrl === params.book.bookUrl
      ? memory.value
      : await getAiBookMemory(params.book.bookUrl).catch(() => null)
    if (!current?.enabled) return null

    const currentConfig = refreshConfig()
    if (!shouldRunAiBookAutoUpdate(current, params.chapter.index, currentConfig)) {
      return current
    }
    return runChapterUpdate({ ...params, current, allowSkip: true })
  }

  async function runChapterUpdate(params: {
    book: Book
    chapter: BookChapter
    chapterContent: string
    current?: AiBookAnyMemory
    allowSkip?: boolean
    throwOnError?: boolean
    chapters?: BookChapter[]
  }) {
    const currentConfig = refreshConfig()
    const current = params.current || (memory.value?.bookUrl === params.book.bookUrl ? memory.value : await load(params.book))
    if (params.allowSkip && !shouldRunAiBookAutoUpdate(current, params.chapter.index, currentConfig)) {
      return current
    }

    if (shouldSkipAiBookChapter(params.chapter, params.chapters || [])) {
      return saveSkippedChapterMemory(params.book, params.chapter, current)
    }

    const key = `${params.book.bookUrl}::${params.chapter.index}`
    if (updatingChapterKeys.has(key)) return current
    updatingChapterKeys.add(key)
    phase.value = 'text'
    statusText.value = `更新 ${params.chapter.title} 的 AI 资料...`

    try {
      const update = await requestAiBookMemoryUpdate({
        config: currentConfig,
        book: params.book,
        chapter: params.chapter,
        chapterContent: params.chapterContent,
        memory: current,
      })
      const next = await saveAiBookMemory(update.memory)
      memory.value = next

      phase.value = 'idle'
      statusText.value = ''
      return next
    } catch (error) {
      const message = (error as Error).message || 'AI 资料更新失败'
      phase.value = 'error'
      statusText.value = message
      const failed: AiBookAnyMemory = {
        ...current,
        bookUrl: params.book.bookUrl,
        bookName: params.book.name,
        author: params.book.author,
        lastError: message,
        lastErrorChapterIndex: params.chapter.index,
        lastErrorChapterTitle: params.chapter.title,
        updatedAt: Date.now(),
      }
      memory.value = await saveAiBookMemory(failed).catch(() => failed)
      if (params.throwOnError) throw error
      return memory.value
    } finally {
      updatingChapterKeys.delete(key)
      if (phase.value === 'error') {
        window.setTimeout(() => {
          if (phase.value === 'error') {
            phase.value = 'idle'
            statusText.value = ''
          }
        }, 3000)
      }
    }
  }

  async function saveSkippedChapterMemory(book: Book, chapter: BookChapter, current: AiBookAnyMemory) {
    const next: AiBookAnyMemory = {
      ...current,
      bookUrl: book.bookUrl,
      bookName: book.name,
      author: book.author,
      processedChapterIndex: Math.max(current.processedChapterIndex ?? -1, chapter.index),
      processedChapterTitle: chapter.title,
      lastError: undefined,
      updatedAt: Date.now(),
    }
    memory.value = await saveAiBookMemory(next).catch(() => next)
    return memory.value
  }

  async function redrawMap(book: Book, prompt?: string, sourceChapterIndex?: number, currentMemory?: AiBookAnyMemory) {
    const currentConfig = refreshConfig()
    const current = currentMemory || memory.value || await load(book)
    const displayMemory = toAiBookDisplayMemory(current)
    const resolvedPrompt = prompt
      || (isAiBookMemoryV2(current)
        ? current.mapState.mapPrompt || current.renderArtifacts.mapImagePrompt
        : current.map?.prompt)
      || buildFallbackMapPrompt(current, book)
    phase.value = 'map'
    statusText.value = '生成世界地图...'
    try {
      const image = await requestAiBookMapImage({
        config: currentConfig,
        prompt: resolvedPrompt,
      })
      const imageUrl = await uploadGeneratedMap({
        b64Json: image.b64Json,
        imageUrl: image.imageUrl,
        filename: `${Date.now()}-${slugify(book.name || 'map')}.png`,
        useBackendProxy: currentConfig.useBackendProxy || currentConfig.modelSource === 'server',
      })
      const map = {
        imageUrl,
        prompt: resolvedPrompt,
        updatedAt: Date.now(),
        sourceChapterIndex,
      }
      const next = isAiBookMemoryV2(current)
        ? applyMapArtifactToMemoryV2(current, map)
        : applyMapToMemory(displayMemory, map)
      memory.value = await saveAiBookMemory(next)
      phase.value = 'idle'
      statusText.value = ''
      return memory.value
    } catch (error) {
      const message = (error as Error).message || '地图生成失败'
      const fallback = isAiBookMemoryV2(current)
        ? applyMapFallbackToMemoryV2(current, resolvedPrompt, `${message}，已显示关系图`, sourceChapterIndex)
        : applyMapFallbackToMemory(displayMemory, {
            prompt: resolvedPrompt,
            reason: `${message}，已显示关系图`,
            sourceChapterIndex,
          })
      memory.value = await saveAiBookMemory(fallback).catch(() => fallback)
      phase.value = 'idle'
      statusText.value = '图片地图不可用，已显示关系图'
      window.setTimeout(() => {
        if (statusText.value === '图片地图不可用，已显示关系图') {
          statusText.value = ''
        }
      }, 3000)
      return memory.value
    }
  }

  return {
    memory,
    loading,
    phase,
    statusText,
    isBusy,
    config,
    serverModelConfig,
    canUseServerModel,
    isServerModelAdmin,
    loadServerModelConfig,
    refreshConfig,
    persistConfig,
    load,
    save,
    setEnabled,
    reset,
    autoUpdateCompletedChapter,
    runChapterUpdate,
    redrawMap,
  }
})

function buildFallbackMapPrompt(memory: AiBookAnyMemory, book: Book) {
  const displayMemory = toAiBookDisplayMemory(memory)
  const locations = displayMemory.locations
    .map((item) => `${item.parentName ? `${item.parentName} > ` : ''}${item.name}${item.kind ? `（${item.kind}）` : ''}: ${item.description}`)
    .join('\n')
  return [
    `为小说《${book.name}》绘制一张不剧透的世界地图。`,
    '只包含已读进度中出现的地点和势力范围。',
    '优先表现地点层级、区域边界、路线连接、图例和地点标签，避免画成建筑外观或场景照片。',
    locations || displayMemory.summary || '保留未知区域，以卷轴地图风格呈现。',
  ].join('\n')
}

function slugify(value: string) {
  const slug = value
    .toLowerCase()
    .replace(/[^a-z0-9\u4e00-\u9fa5]+/gi, '-')
    .replace(/^-+|-+$/g, '')
  return slug || 'map'
}
