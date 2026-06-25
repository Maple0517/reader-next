import { nextTick, ref } from 'vue'
import type { ComputedRef, Ref } from 'vue'
import type { useReaderStore } from '../stores/reader'

type ReaderStore = ReturnType<typeof useReaderStore>

export interface ContinuousChapterItem {
  index: number
  title: string
  content: string
  html: string
}

export function useContinuousReading(
  store: ReaderStore,
  renderChapterHtml: (rawText: string) => string,
  isContinuousMode: ComputedRef<boolean>,
  hideReadChaptersMode: ComputedRef<boolean>,
  scrollContainerRef: Ref<HTMLElement | undefined>,
) {
  const continuousChapters = ref<ContinuousChapterItem[]>([])
  const continuousLoadingNext = ref(false)
  const continuousLoadingPrev = ref(false)
  const suppressContinuousSync = ref(false)
  let continuousStateSyncTimer: number | null = null

  function shouldHideChapter(index: number, keepIndex?: number) {
    if (!hideReadChaptersMode.value) return false
    if (typeof keepIndex === 'number' && index === keepIndex) return false
    return store.isChapterRead(index)
  }

  function findNextVisibleIndex(startIndex: number, keepIndex?: number) {
    for (let index = startIndex; index < store.chapters.length; index += 1) {
      if (!shouldHideChapter(index, keepIndex)) {
        return index
      }
    }
    return -1
  }

  function pruneReadChapters(targetIndex = store.currentIndex) {
    if (!hideReadChaptersMode.value) return
    continuousChapters.value = continuousChapters.value.filter((chapter) => chapter.index >= targetIndex)
  }

  async function buildContinuousChapter(index: number, forceRefresh = false) {
    const chapter = store.chapters[index]
    if (!chapter) return null
    const chapterContent = await store.fetchChapterContent(index, forceRefresh)
    if (chapterContent == null) return null
    return {
      index,
      title: chapter.title,
      content: chapterContent,
      html: renderChapterHtml(chapterContent),
    } satisfies ContinuousChapterItem
  }

  function syncContinuousChapterHtml() {
    continuousChapters.value = continuousChapters.value.map((chapter) => ({
      ...chapter,
      html: renderChapterHtml(chapter.content),
    }))
  }

  function getContinuousChapter(index: number) {
    return continuousChapters.value.find((chapter) => chapter.index === index) || null
  }

  function setContinuousActiveChapter(index: number, chapterContent: string, progress: number) {
    suppressContinuousSync.value = true
    store.setActiveChapterState(index, chapterContent, progress)
    store.markChapterAsRead(index)
    void store.persistProgress(index)
    if (continuousStateSyncTimer) {
      clearTimeout(continuousStateSyncTimer)
    }
    continuousStateSyncTimer = window.setTimeout(() => {
      suppressContinuousSync.value = false
    }, 0)
  }

  async function initializeContinuousChapters(targetIndex = store.currentIndex, smooth = false) {
    if (!isContinuousMode.value || !store.chapters[targetIndex]) return

    const current = await buildContinuousChapter(targetIndex)
    if (!current) return

    continuousChapters.value = [current]
    const initialProgress = targetIndex === store.currentIndex ? store.chapterScrollProgress : 0
    setContinuousActiveChapter(targetIndex, current.content, initialProgress)

    await nextTick()
    if (initialProgress <= 0) {
      scrollToContinuousChapter(targetIndex, smooth)
    }

    const nextIndex = hideReadChaptersMode.value
      ? findNextVisibleIndex(targetIndex + 1, targetIndex)
      : targetIndex + 1
    if (nextIndex < 0) return

    void (async () => {
      const next = await buildContinuousChapter(nextIndex)
      if (!next) return
      if (continuousChapters.value.some((chapter) => chapter.index === next.index)) return
      continuousChapters.value = [...continuousChapters.value, next]
    })()
  }

  async function syncContinuousToStoreState() {
    if (!isContinuousMode.value || suppressContinuousSync.value || store.loading || !store.chapters[store.currentIndex]) return

    pruneReadChapters(store.currentIndex)
    const current = getContinuousChapter(store.currentIndex)
    if (current) {
      if (current.content !== store.content) {
        current.content = store.content
        current.html = renderChapterHtml(store.content)
      }
      return
    }

    await initializeContinuousChapters(store.currentIndex, false)
  }

  async function loadContinuousNext() {
    if (continuousLoadingNext.value || !continuousChapters.value.length) return
    const last = continuousChapters.value[continuousChapters.value.length - 1]
    const nextIndex = hideReadChaptersMode.value ? findNextVisibleIndex(last.index + 1, store.currentIndex) : last.index + 1
    if (nextIndex >= store.chapters.length) return

    continuousLoadingNext.value = true
    try {
      const next = await buildContinuousChapter(nextIndex)
      if (next && !getContinuousChapter(next.index)) {
        continuousChapters.value = [...continuousChapters.value, next]
      }
    } finally {
      continuousLoadingNext.value = false
    }
  }

  async function loadContinuousPrev() {
    if (hideReadChaptersMode.value) return
    if (continuousLoadingPrev.value || !continuousChapters.value.length) return
    const first = continuousChapters.value[0]
    const prevIndex = first.index - 1
    if (prevIndex < 0) return

    const container = scrollContainerRef.value
    const previousHeight = container?.scrollHeight || 0
    const previousTop = container?.scrollTop || 0

    continuousLoadingPrev.value = true
    try {
      const prev = await buildContinuousChapter(prevIndex)
      if (prev && !getContinuousChapter(prev.index)) {
        continuousChapters.value = [prev, ...continuousChapters.value]
        await nextTick()
        if (container) {
          const heightDiff = container.scrollHeight - previousHeight
          container.scrollTop = previousTop + heightDiff
        }
      }
    } finally {
      continuousLoadingPrev.value = false
    }
  }

  async function ensureContinuousChapterLoaded(index: number) {
    if (getContinuousChapter(index)) return
    if (!continuousChapters.value.length) {
      await initializeContinuousChapters(index, false)
      return
    }

    while (continuousChapters.value[0] && index < continuousChapters.value[0].index) {
      await loadContinuousPrev()
    }

    while (
      continuousChapters.value[continuousChapters.value.length - 1]
      && index > continuousChapters.value[continuousChapters.value.length - 1].index
    ) {
      await loadContinuousNext()
    }
  }

  function getContinuousSections() {
    const container = scrollContainerRef.value
    if (!container) return [] as HTMLElement[]
    return Array.from(container.querySelectorAll('.continuous-chapter')) as HTMLElement[]
  }

  function scrollToContinuousChapter(index: number, smooth = true) {
    const container = scrollContainerRef.value
    if (!container) return
    const section = container.querySelector(`.continuous-chapter[data-chapter-index="${index}"]`) as HTMLElement | null
    if (!section) return
    container.scrollTo({
      top: Math.max(0, section.offsetTop),
      behavior: smooth ? 'smooth' : 'auto',
    })
  }

  function clearContinuousChapters() {
    continuousChapters.value = []
  }

  function disposeContinuousReading() {
    if (continuousStateSyncTimer) {
      clearTimeout(continuousStateSyncTimer)
      continuousStateSyncTimer = null
    }
  }

  return {
    continuousChapters,
    continuousLoadingNext,
    continuousLoadingPrev,
    suppressContinuousSync,
    syncContinuousChapterHtml,
    getContinuousChapter,
    setContinuousActiveChapter,
    initializeContinuousChapters,
    syncContinuousToStoreState,
    loadContinuousNext,
    loadContinuousPrev,
    ensureContinuousChapterLoaded,
    getContinuousSections,
    scrollToContinuousChapter,
    pruneReadChapters,
    clearContinuousChapters,
    disposeContinuousReading,
  }
}
