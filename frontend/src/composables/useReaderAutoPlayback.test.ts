import { computed, ref } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useReaderAutoPlayback } from './useReaderAutoPlayback'

function createStore() {
  const spoken: Array<{
    text: string
    options: { onEnd: () => void; onError: () => void }
  }> = []
  const store = {
    isSpeaking: false,
    isPaused: false,
    isAutoScrolling: false,
    hasNext: false,
    hasPrev: false,
    currentIndex: 0,
    systemTtsNativeEventsReliable: true,
    speechConfig: {
      provider: 'system',
      openaiRequestMode: 'chunked',
    },
    startTTS: vi.fn((text, options) => {
      store.isSpeaking = true
      spoken.push({ text, options })
    }),
    stopTTS: vi.fn(() => {
      store.isSpeaking = false
    }),
    preloadOpenAITTS: vi.fn(),
  }
  return { store, spoken }
}

function createPlayback(
  store: ReturnType<typeof createStore>['store'],
  container: HTMLElement,
  chapterText: HTMLElement,
  horizontal?: {
    pageIndex: { value: number }
    showPage: (pageIndex: number) => void
  },
  options?: {
    autoPageMode?: string
    nextChapter?: () => void | Promise<void>
  },
) {
  return useReaderAutoPlayback(
    store as never,
    computed(() => ({
      autoPageMode: options?.autoPageMode || 'paragraph',
      clickAction: 'auto',
      scrollPixel: 1,
      pageSpeed: 1000,
      fontSize: 18,
      lineHeight: 1.8,
    })),
    computed(() => false),
    ref(container),
    ref(chapterText),
    options?.nextChapter || vi.fn(),
    vi.fn(),
    horizontal
      ? {
          isEnabled: computed(() => true),
          getPageIndex: () => horizontal.pageIndex.value,
          showPage: horizontal.showPage,
        }
      : undefined,
  )
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('useReaderAutoPlayback reading position', () => {
  it('starts at the first paragraph on the current horizontal page', () => {
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = `
      <section class="horizontal-page"><p>第一页</p></section>
      <section class="horizontal-page"><p>当前页顶部</p><p>当前页下一段</p></section>
    `
    container.appendChild(chapterText)
    const { store, spoken } = createStore()
    const pageIndex = ref(1)
    const playback = createPlayback(store, container, chapterText, {
      pageIndex,
      showPage: (next) => {
        pageIndex.value = next
      },
    })

    playback.startSpeech()

    expect(spoken[0]?.text).toBe('当前页顶部')
  })

  it('turns the horizontal page as speech advances', () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = `
      <section class="horizontal-page"><p>当前页</p></section>
      <section class="horizontal-page"><p>下一页</p></section>
    `
    container.appendChild(chapterText)
    const { store, spoken } = createStore()
    const pageIndex = ref(0)
    const showPage = vi.fn((next: number) => {
      pageIndex.value = next
    })
    const playback = createPlayback(store, container, chapterText, { pageIndex, showPage })

    playback.startSpeech()
    spoken[0]?.options.onEnd()
    vi.runAllTimers()

    expect(showPage).toHaveBeenCalledWith(1)
    expect(spoken[1]?.text).toBe('下一页')
  })

  it('speaks one logical paragraph as a single unit when pagination splits it across pages', () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = `
      <section class="horizontal-page"><p data-reader-paragraph="0">跨页自然段前半句，</p></section>
      <section class="horizontal-page">
        <p data-reader-paragraph="0">后半句。</p>
        <p data-reader-paragraph="1">下一自然段。</p>
      </section>
    `
    container.appendChild(chapterText)
    const { store, spoken } = createStore()
    const pageIndex = ref(0)
    const showPage = vi.fn((next: number) => {
      pageIndex.value = next
    })
    const playback = createPlayback(store, container, chapterText, { pageIndex, showPage })

    playback.startSpeech()

    expect(spoken[0]?.text).toBe('跨页自然段前半句，后半句。')
    spoken[0]?.options.onEnd()
    vi.runAllTimers()
    expect(spoken[1]?.text).toBe('下一自然段。')
  })

  it('keeps merged speech within one horizontal page before turning', () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = `
      <section class="horizontal-page"><p>当前页第一段</p><p>当前页第二段</p></section>
      <section class="horizontal-page"><p>下一页第一段</p></section>
    `
    container.appendChild(chapterText)
    const { store, spoken } = createStore()
    store.speechConfig.provider = 'openai'
    store.speechConfig.openaiRequestMode = 'merged'
    const pageIndex = ref(0)
    const showPage = vi.fn((next: number) => {
      pageIndex.value = next
    })
    const playback = createPlayback(store, container, chapterText, { pageIndex, showPage })

    playback.startSpeech()

    expect(spoken[0]?.text).toBe('当前页第一段\n当前页第二段')
    spoken[0]?.options.onEnd()
    vi.runAllTimers()
    expect(showPage).toHaveBeenCalledWith(1)
    expect(spoken[1]?.text).toBe('下一页第一段')
  })

  it('continues from an explicitly started paragraph instead of the old viewport paragraph', () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = '<p>屏幕旧段</p><p>指定起点</p><p>正确下一段</p>'
    container.appendChild(chapterText)
    const paragraphs = chapterText.querySelectorAll('p')
    vi.spyOn(container, 'getBoundingClientRect').mockReturnValue({ top: 0 } as DOMRect)
    vi.spyOn(paragraphs[0]!, 'getBoundingClientRect').mockReturnValue({ bottom: 80 } as DOMRect)
    const { store, spoken } = createStore()
    store.speechConfig.provider = 'openai'
    const playback = createPlayback(store, container, chapterText)

    playback.startSpeech(paragraphs[1] as HTMLElement)
    expect(spoken[0]?.text).toBe('指定起点')
    spoken[0]?.options.onEnd()
    vi.runAllTimers()

    expect(spoken[1]?.text).toBe('正确下一段')
  })

  it('hands a preloaded remote segment to the next audio buffer without a fixed gap', () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = '<p>第一段。</p><p>第二段。</p>'
    container.appendChild(chapterText)
    const { store, spoken } = createStore()
    store.speechConfig.provider = 'openai'
    const playback = createPlayback(store, container, chapterText)

    playback.startSpeech(chapterText.querySelector('p') as HTMLElement)
    spoken[0]?.options.onEnd()
    vi.advanceTimersByTime(0)

    expect(spoken[1]?.text).toBe('第二段。')
  })

  it('lets manual next override a pending automatic transition', () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = '<p>第一段</p><p>第二段</p><p>第三段</p>'
    container.appendChild(chapterText)
    const paragraphs = chapterText.querySelectorAll('p')
    const { store, spoken } = createStore()
    store.speechConfig.provider = 'openai'
    const playback = createPlayback(store, container, chapterText)

    playback.startSpeech(paragraphs[0] as HTMLElement)
    spoken[0]?.options.onEnd()
    playback.speechNext()
    vi.runAllTimers()

    expect(spoken.map((item) => item.text)).toEqual(['第一段', '第二段'])
  })

  it('ignores an old reading marker when starting from a new vertical position', () => {
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = '<p class="reading">旧位置</p><p>当前页面顶部</p>'
    container.appendChild(chapterText)
    const paragraphs = chapterText.querySelectorAll('p')
    vi.spyOn(container, 'getBoundingClientRect').mockReturnValue({ top: 0 } as DOMRect)
    vi.spyOn(paragraphs[0]!, 'getBoundingClientRect').mockReturnValue({ bottom: 20 } as DOMRect)
    vi.spyOn(paragraphs[1]!, 'getBoundingClientRect').mockReturnValue({ bottom: 80 } as DOMRect)
    const { store, spoken } = createStore()
    const playback = createPlayback(store, container, chapterText)

    playback.startSpeech()

    expect(spoken[0]?.text).toBe('当前页面顶部')
  })
})

describe('useReaderAutoPlayback auto reading', () => {
  function stubAnimationFrame() {
    let callback: FrameRequestCallback | null = null
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      callback = cb
      return 1
    })
    vi.stubGlobal('cancelAnimationFrame', () => {
      callback = null
    })
    return {
      runFrame: () => {
        const cb = callback
        callback = null
        cb?.(0)
      },
    }
  }

  function defineScrollGeometry(el: HTMLElement, values: { scrollHeight: number; clientHeight: number; scrollTop: number }) {
    let top = values.scrollTop
    Object.defineProperty(el, 'scrollHeight', { value: values.scrollHeight, configurable: true })
    Object.defineProperty(el, 'clientHeight', { value: values.clientHeight, configurable: true })
    Object.defineProperty(el, 'scrollTop', {
      get: () => top,
      set: (next: number) => {
        top = next
      },
      configurable: true,
    })
  }

  it('uses paragraph progression instead of jumping chapters in horizontal mode with pixel mode configured', async () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = `
      <section class="horizontal-page"><p>第一页段落</p></section>
      <section class="horizontal-page"><p>第二页段落</p></section>
    `
    container.appendChild(chapterText)
    const { store } = createStore()
    store.hasNext = true
    const nextChapter = vi.fn()
    const pageIndex = ref(0)
    const showPage = vi.fn((next: number) => {
      pageIndex.value = next
    })
    const playback = createPlayback(store, container, chapterText, { pageIndex, showPage }, { autoPageMode: 'pixel', nextChapter })

    store.isAutoScrolling = true
    playback.startAutoScroll()

    const paragraphs = chapterText.querySelectorAll('p')
    expect(nextChapter).not.toHaveBeenCalled()
    expect(paragraphs[0]?.classList.contains('reading')).toBe(true)

    await vi.advanceTimersByTimeAsync(1000)

    expect(showPage).toHaveBeenCalledWith(1)
    expect(paragraphs[1]?.classList.contains('reading')).toBe(true)
    expect(nextChapter).not.toHaveBeenCalled()
  })

  it('advances to the next chapter only after the last paragraph dwell in horizontal mode', async () => {
    vi.useFakeTimers()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = `
      <section class="horizontal-page"><p>唯一一段</p></section>
    `
    container.appendChild(chapterText)
    const { store } = createStore()
    store.hasNext = true
    const nextChapter = vi.fn()
    const pageIndex = ref(0)
    const playback = createPlayback(store, container, chapterText, {
      pageIndex,
      showPage: (next) => {
        pageIndex.value = next
      },
    }, { autoPageMode: 'pixel', nextChapter })

    store.isAutoScrolling = true
    playback.startAutoScroll()
    expect(nextChapter).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1000)

    expect(nextChapter).toHaveBeenCalledTimes(1)
  })

  it('stops pixel auto scroll without jumping chapters when content fits the viewport', () => {
    stubAnimationFrame()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = '<p>短章节</p>'
    container.appendChild(chapterText)
    defineScrollGeometry(container, { scrollHeight: 500, clientHeight: 500, scrollTop: 0 })
    const { store } = createStore()
    store.hasNext = true
    const nextChapter = vi.fn()
    const playback = createPlayback(store, container, chapterText, undefined, { autoPageMode: 'pixel', nextChapter })

    store.isAutoScrolling = true
    playback.startAutoScroll()

    expect(nextChapter).not.toHaveBeenCalled()
    expect(store.isAutoScrolling).toBe(false)
  })

  it('does not jump chapters instantly when pixel auto scroll starts at the bottom', async () => {
    vi.useFakeTimers()
    const frames = stubAnimationFrame()
    const container = document.createElement('div')
    const chapterText = document.createElement('div')
    chapterText.innerHTML = '<p>已在章末</p>'
    container.appendChild(chapterText)
    defineScrollGeometry(container, { scrollHeight: 2000, clientHeight: 500, scrollTop: 1500 })
    const { store } = createStore()
    store.hasNext = true
    const nextChapter = vi.fn()
    const playback = createPlayback(store, container, chapterText, undefined, { autoPageMode: 'pixel', nextChapter })

    store.isAutoScrolling = true
    playback.startAutoScroll()
    expect(nextChapter).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1100)
    frames.runFrame()

    expect(nextChapter).toHaveBeenCalledTimes(1)
  })
})
