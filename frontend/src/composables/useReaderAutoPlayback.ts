import type { ComputedRef, Ref } from 'vue'
import type { useReaderStore } from '../stores/reader'

type ReaderStore = ReturnType<typeof useReaderStore>
const OPENAI_SPEECH_CHUNK_CHAR_LIMIT = 70
const OPENAI_PRELOAD_CHUNK_LIMIT = 5
const OPENAI_MERGED_SEGMENT_CHAR_LIMIT = 260

interface AutoPlaybackConfig {
  autoPageMode: string
  clickAction: string
  scrollPixel: number
  pageSpeed: number
  fontSize: number
  lineHeight: number
}

interface HorizontalPlayback {
  isEnabled: ComputedRef<boolean>
  getPageIndex: () => number
  showPage: (pageIndex: number) => void
}

export function useReaderAutoPlayback(
  store: ReaderStore,
  config: ComputedRef<AutoPlaybackConfig>,
  isContinuousMode: ComputedRef<boolean>,
  scrollContainerRef: Ref<HTMLElement | undefined>,
  chapterTextRef: Ref<HTMLElement | undefined>,
  nextChapter: () => void | Promise<void>,
  prevChapter: () => void | Promise<void>,
  horizontalPlayback?: HorizontalPlayback,
) {
  let autoScrollId: number | null = null
  let autoScrollSessionTop = 0
  let autoScrollSessionAt = 0
  let autoParagraphTimer: number | null = null
  let autoReadingParagraphIndex = -1
  let autoReadingProcessing = false
  let speechRestartTimer: number | null = null
  let isSpeechTransitioning = false
  let activeSpeechParagraph: HTMLElement | null = null
  let currentSpeechParagraph: HTMLElement | null = null
  let currentSpeechSegments: { text: string; nextParagraph: HTMLElement | null }[] = []
  let currentSpeechSegmentIndex = 0

  function isSafariSpeechDelayBrowser() {
    if (typeof navigator === 'undefined') return false
    const ua = navigator.userAgent || ''
    return /Safari/i.test(ua) && !/Chrome|Chromium|CriOS|Edg|EdgiOS|Android/i.test(ua)
  }

  function paragraphPreview(paragraph: HTMLElement | null) {
    return paragraph?.innerText.trim().slice(0, 40) || ''
  }

  function logSpeech(message: string, payload?: unknown) {
    void message
    void payload
  }

  function getFilteredParagraphs() {
    const roots = isContinuousMode.value
      ? Array.from(scrollContainerRef.value?.querySelectorAll('.chapter-text[data-role="continuous"]') || []) as HTMLElement[]
      : (chapterTextRef.value ? [chapterTextRef.value] : [])
    if (!roots.length) return [] as HTMLElement[]
    const allElements = roots.flatMap((root) => Array.from(root.querySelectorAll('p')) as HTMLElement[])
    const list: HTMLElement[] = []
    let lastText = ''
    allElements.forEach((el) => {
      const text = el.innerText.trim()
      if (text && text !== lastText) {
        list.push(el)
        lastText = text
      }
    })
    return list
  }

  function getCurrentParagraph() {
    if ((store.isSpeaking || isSpeechTransitioning) && activeSpeechParagraph) {
      return activeSpeechParagraph
    }
    const reading = scrollContainerRef.value?.querySelector('.reading') as HTMLElement | null
    if (store.isSpeaking && reading) return reading

    const container = scrollContainerRef.value
    if (!container) return null

    if (horizontalPlayback?.isEnabled.value && chapterTextRef.value) {
      const pages = Array.from(chapterTextRef.value.querySelectorAll('.horizontal-page')) as HTMLElement[]
      const pageIndex = Math.max(0, Math.min(pages.length - 1, horizontalPlayback.getPageIndex()))
      const paragraphs = Array.from(pages[pageIndex]?.querySelectorAll('p') || []) as HTMLElement[]
      return paragraphs.find((paragraph) => paragraph.innerText.trim()) || null
    }

    const list = getFilteredParagraphs()
    const containerRect = container.getBoundingClientRect()
    const anchorTop = containerRect.top + 40
    for (const paragraph of list) {
      if (paragraph.getBoundingClientRect().bottom > anchorTop) {
        return paragraph
      }
    }

    return list[0] || null
  }

  function getPrevParagraph() {
    const current = getCurrentParagraph()
    return getPrevParagraphFrom(current)
  }

  function getPrevParagraphFrom(current: HTMLElement | null) {
    const list = getFilteredParagraphs()
    const index = current ? list.indexOf(current) : -1
    if (index > 0) {
      const paragraphId = current?.dataset.readerParagraph
      for (let cursor = index - 1; cursor >= 0; cursor -= 1) {
        if (!paragraphId || list[cursor]?.dataset.readerParagraph !== paragraphId) {
          const previousId = list[cursor]?.dataset.readerParagraph
          if (!previousId) return list[cursor] || null
          while (cursor > 0 && list[cursor - 1]?.dataset.readerParagraph === previousId) {
            cursor -= 1
          }
          return list[cursor] || null
        }
      }
    }
    return null
  }

  function getNextParagraph() {
    const current = getCurrentParagraph()
    return getNextParagraphFrom(current)
  }

  function getNextParagraphFrom(current: HTMLElement | null) {
    const list = getFilteredParagraphs()
    const index = current ? list.indexOf(current) : -1
    if (index >= 0 && index < list.length - 1) {
      const paragraphId = current?.dataset.readerParagraph
      for (let cursor = index + 1; cursor < list.length; cursor += 1) {
        if (!paragraphId || list[cursor]?.dataset.readerParagraph !== paragraphId) {
          return list[cursor] || null
        }
      }
    }
    return null
  }

  function getLogicalParagraphText(paragraph: HTMLElement | null) {
    const currentText = paragraph?.innerText.trim() || ''
    const paragraphId = paragraph?.dataset.readerParagraph
    if (!paragraphId) return currentText

    return getFilteredParagraphs()
      .filter((item) => item.dataset.readerParagraph === paragraphId)
      .map((item) => item.innerText.trim())
      .filter(Boolean)
      .join('')
  }

  function splitLongSentence(sentence: string) {
    const chunks: string[] = []
    let remaining = sentence.trim()
    while (remaining.length > OPENAI_SPEECH_CHUNK_CHAR_LIMIT) {
      let splitIndex = Math.max(
        remaining.lastIndexOf('，', OPENAI_SPEECH_CHUNK_CHAR_LIMIT),
        remaining.lastIndexOf('、', OPENAI_SPEECH_CHUNK_CHAR_LIMIT),
        remaining.lastIndexOf(',', OPENAI_SPEECH_CHUNK_CHAR_LIMIT),
        remaining.lastIndexOf(' ', OPENAI_SPEECH_CHUNK_CHAR_LIMIT),
      )
      if (splitIndex <= 0) {
        splitIndex = OPENAI_SPEECH_CHUNK_CHAR_LIMIT
      }
      chunks.push(remaining.slice(0, splitIndex).trim())
      remaining = remaining.slice(splitIndex).trim()
    }
    if (remaining) chunks.push(remaining)
    return chunks
  }

  function buildParagraphSpeechChunks(paragraph: HTMLElement | null) {
    const rawText = getLogicalParagraphText(paragraph)
    if (!rawText) return [] as string[]

    const sentences = rawText
      .replace(/\n+/g, '\n')
      .split(/(?<=[。！？!?；;])/)
      .map((item) => item.trim())
      .filter(Boolean)

    const chunks: string[] = []
    let current = ''

    const pushCurrent = () => {
      const normalized = current.trim()
      if (normalized) chunks.push(normalized)
      current = ''
    }

    for (const sentence of (sentences.length ? sentences : [rawText])) {
      if (sentence.length > OPENAI_SPEECH_CHUNK_CHAR_LIMIT) {
        pushCurrent()
        chunks.push(...splitLongSentence(sentence))
        continue
      }
      const next = current ? `${current}${sentence}` : sentence
      if (next.length > OPENAI_SPEECH_CHUNK_CHAR_LIMIT) {
        pushCurrent()
        current = sentence
      } else {
        current = next
      }
    }

    pushCurrent()
    return chunks.length ? chunks : [rawText]
  }

  function buildMergedSpeechSegment(paragraph: HTMLElement | null) {
    const currentText = getLogicalParagraphText(paragraph)
    if (!currentText) {
      return {
        text: '',
        nextParagraph: getNextParagraphFrom(paragraph),
      }
    }

    const list = getFilteredParagraphs()
    const startIndex = paragraph ? list.indexOf(paragraph) : -1
    if (startIndex < 0) {
      return {
        text: currentText,
        nextParagraph: getNextParagraphFrom(paragraph),
      }
    }

    const mergedTexts: string[] = [currentText]
    let mergedLength = currentText.length
    const indexAfterLogicalParagraph = (index: number) => {
      const paragraphId = list[index]?.dataset.readerParagraph
      let nextIndex = index + 1
      while (paragraphId && list[nextIndex]?.dataset.readerParagraph === paragraphId) {
        nextIndex += 1
      }
      return nextIndex
    }
    let cursorIndex = indexAfterLogicalParagraph(startIndex)
    const currentPage = horizontalPlayback?.isEnabled.value
      ? paragraph?.closest('.horizontal-page')
      : null

    while (cursorIndex < list.length && mergedLength < OPENAI_MERGED_SEGMENT_CHAR_LIMIT) {
      if (currentPage && list[cursorIndex]?.closest('.horizontal-page') !== currentPage) {
        break
      }
      const nextText = getLogicalParagraphText(list[cursorIndex] || null)
      if (!nextText) {
        cursorIndex = indexAfterLogicalParagraph(cursorIndex)
        continue
      }
      if (mergedLength + nextText.length > OPENAI_MERGED_SEGMENT_CHAR_LIMIT) {
        break
      }
      mergedTexts.push(nextText)
      mergedLength += nextText.length
      cursorIndex = indexAfterLogicalParagraph(cursorIndex)
    }

    return {
      text: mergedTexts.join('\n'),
      nextParagraph: list[cursorIndex] || null,
    }
  }

  function resetSpeechChunkState() {
    currentSpeechParagraph = null
    currentSpeechSegments = []
    currentSpeechSegmentIndex = 0
  }

  function buildOpenAISpeechSegments(paragraph: HTMLElement) {
    if (store.speechConfig.openaiRequestMode === 'merged') {
      const merged = buildMergedSpeechSegment(paragraph)
      return merged.text ? [merged] : []
    }

    const paragraphChunks = buildParagraphSpeechChunks(paragraph)
    const nextParagraph = getNextParagraphFrom(paragraph)
    return paragraphChunks.map((text, index) => ({
      text,
      nextParagraph: index < paragraphChunks.length - 1 ? paragraph : nextParagraph,
    }))
  }

  function ensureSpeechChunkState(paragraph: HTMLElement) {
    if (store.speechConfig.provider === 'system') {
      return {
        text: getLogicalParagraphText(paragraph),
        nextParagraph: getNextParagraphFrom(paragraph),
      }
    }

    if (currentSpeechParagraph !== paragraph) {
      currentSpeechParagraph = paragraph
      currentSpeechSegments = buildOpenAISpeechSegments(paragraph)
      currentSpeechSegmentIndex = 0
    }

    return currentSpeechSegments[currentSpeechSegmentIndex] || {
      text: '',
      nextParagraph: getNextParagraphFrom(paragraph),
    }
  }

  function getUpcomingSpeechChunks(startParagraph: HTMLElement | null) {
    const chunks: string[] = []

    if (store.speechConfig.provider === 'system') {
      return chunks
    }

    if (store.speechConfig.openaiRequestMode === 'merged') {
      const merged = buildMergedSpeechSegment(startParagraph)
      return merged.text ? [merged.text] : []
    }

    if (currentSpeechParagraph && currentSpeechSegments.length) {
      for (let i = currentSpeechSegmentIndex + 1; i < currentSpeechSegments.length && chunks.length < OPENAI_PRELOAD_CHUNK_LIMIT; i += 1) {
        if (currentSpeechSegments[i]?.text) {
          chunks.push(currentSpeechSegments[i].text)
        }
      }
    }

    let cursor = startParagraph
    while (cursor && chunks.length < OPENAI_PRELOAD_CHUNK_LIMIT) {
      const paragraphChunks = buildParagraphSpeechChunks(cursor)
      for (const chunk of paragraphChunks) {
        if (chunks.length >= OPENAI_PRELOAD_CHUNK_LIMIT) break
        chunks.push(chunk)
      }
      cursor = getNextParagraphFrom(cursor)
    }

    return chunks
  }

  function clearReadingClass() {
    scrollContainerRef.value?.querySelectorAll('.reading').forEach((el) => el.classList.remove('reading'))
  }

  function showParagraph(paragraph: HTMLElement | null, smooth = true) {
    const container = scrollContainerRef.value
    if (!container || !paragraph) return

    if (horizontalPlayback?.isEnabled.value && chapterTextRef.value) {
      const page = paragraph.closest('.horizontal-page')
      const pages = Array.from(chapterTextRef.value.querySelectorAll('.horizontal-page'))
      const pageIndex = page ? pages.indexOf(page) : -1
      if (pageIndex >= 0 && pageIndex !== horizontalPlayback.getPageIndex()) {
        horizontalPlayback.showPage(pageIndex)
      }
      return
    }

    const targetTop = Math.max(0, paragraph.offsetTop - 24)
    container.scrollTo({
      top: targetTop,
      behavior: smooth ? 'smooth' : 'auto',
    })
  }

  function markReadingParagraph(paragraph: HTMLElement | null) {
    clearReadingClass()
    if (paragraph) {
      paragraph.classList.add('reading')
    }
  }

  // Horizontal page mode has no vertical overflow to pixel-scroll through,
  // so auto reading there is always driven by paragraph progression.
  function usesParagraphAutoReading() {
    return config.value.autoPageMode === 'paragraph' || !!horizontalPlayback?.isEnabled.value
  }

  function estimateParagraphLineCount(paragraph: HTMLElement) {
    const lineHeightPx = Math.max(1, config.value.fontSize * config.value.lineHeight)
    if (paragraph.offsetHeight > 0) {
      return Math.max(1, Math.ceil(paragraph.offsetHeight / lineHeightPx))
    }
    // Not laid out yet (height unknown): fall back to a rough CJK chars-per-line estimate.
    const charCount = paragraph.innerText.trim().length
    return Math.max(1, Math.ceil(charCount / 24))
  }

  function showParagraphForAutoReading(paragraph: HTMLElement | null) {
    if (!paragraph) return
    if (horizontalPlayback?.isEnabled.value) {
      showParagraph(paragraph)
      return
    }
    const container = scrollContainerRef.value
    if (!container) {
      showParagraph(paragraph)
      return
    }
    const containerRect = container.getBoundingClientRect()
    const rect = paragraph.getBoundingClientRect()
    const comfortablyVisible = rect.top >= containerRect.top + 8 && rect.bottom <= containerRect.bottom - 8
    if (!comfortablyVisible) {
      showParagraph(paragraph)
    }
  }

  function runAutoScroll() {
    if (!store.isAutoScrolling || !scrollContainerRef.value) return

    const container = scrollContainerRef.value
    const speed = Math.max(1, config.value.scrollPixel) * (config.value.pageSpeed / 1000) * 0.5

    container.scrollTop += speed

    const maxScrollTop = container.scrollHeight - container.clientHeight
    if (maxScrollTop <= 2) {
      // Content fits the viewport: there is nowhere to scroll, so stop
      // instead of treating the first frame as "reached the end".
      stopAutoScroll()
      return
    }

    if (container.scrollTop >= maxScrollTop - 2) {
      // Guard against an instant chapter jump when auto reading starts
      // already at (or restored to) the bottom of a chapter.
      const movedEnough = container.scrollTop > autoScrollSessionTop + 2
      const waitedEnough = Date.now() - autoScrollSessionAt > 1000
      if (!movedEnough && !waitedEnough) {
        autoScrollId = requestAnimationFrame(runAutoScroll)
        return
      }
      autoScrollId = null
      if (config.value.clickAction === 'auto' && store.hasNext) {
        Promise.resolve(nextChapter()).then(() => {
          window.setTimeout(() => {
            if (store.isAutoScrolling && !usesParagraphAutoReading()) {
              autoScrollSessionTop = scrollContainerRef.value?.scrollTop || 0
              autoScrollSessionAt = Date.now()
              runAutoScroll()
            }
          }, 300)
        })
      } else {
        stopAutoScroll()
      }
    } else {
      autoScrollId = requestAnimationFrame(runAutoScroll)
    }
  }

  function runAutoParagraph() {
    if (!store.isAutoScrolling) return
    if (autoReadingProcessing) return

    const list = getFilteredParagraphs()
    if (!list.length) return

    autoReadingProcessing = true

    if (autoReadingParagraphIndex < 0) {
      const current = getCurrentParagraph()
      autoReadingParagraphIndex = current ? Math.max(0, list.indexOf(current)) : 0
    }

    if (autoReadingParagraphIndex >= list.length) {
      autoReadingParagraphIndex = -1
      autoReadingProcessing = false
      if (store.hasNext) {
        Promise.resolve(nextChapter()).then(() => {
          window.setTimeout(() => {
            if (store.isAutoScrolling && usesParagraphAutoReading()) {
              runAutoParagraph()
            }
          }, 300)
        })
      } else {
        stopAutoScroll()
      }
      return
    }

    const current = list[autoReadingParagraphIndex]
    markReadingParagraph(current)
    showParagraphForAutoReading(current)

    const delayTime = Math.max(300, config.value.pageSpeed * estimateParagraphLineCount(current))

    autoReadingProcessing = false
    autoParagraphTimer = window.setTimeout(() => {
      autoReadingParagraphIndex += 1
      runAutoParagraph()
    }, delayTime)
  }

  function startAutoScroll() {
    if (usesParagraphAutoReading()) {
      if (autoParagraphTimer) return
      runAutoParagraph()
      return
    }
    if (autoScrollId) return
    autoScrollSessionTop = scrollContainerRef.value?.scrollTop || 0
    autoScrollSessionAt = Date.now()
    runAutoScroll()
  }

  function stopAutoScroll() {
    store.isAutoScrolling = false
    autoReadingParagraphIndex = -1
    autoReadingProcessing = false
    if (autoScrollId) {
      cancelAnimationFrame(autoScrollId)
      autoScrollId = null
    }
    if (autoParagraphTimer) {
      clearTimeout(autoParagraphTimer)
      autoParagraphTimer = null
    }
    if (!store.isSpeaking) {
      clearReadingClass()
    }
  }

  function restartSpeechTarget(paragraph: HTMLElement | null, interruptCurrent = true) {
    logSpeech('restartSpeechTarget', {
      interruptCurrent,
      paragraph: paragraphPreview(paragraph),
      isSpeechTransitioning,
    })
    if (!paragraph) {
      store.stopTTS()
      resetSpeechChunkState()
      return
    }
    if (isSpeechTransitioning && !interruptCurrent) return
    isSpeechTransitioning = true
    resetSpeechChunkState()
    if (interruptCurrent) {
      store.stopTTS(false)
    }
    if (speechRestartTimer) {
      clearTimeout(speechRestartTimer)
    }
    const restartDelay = !interruptCurrent && store.speechConfig.provider === 'system'
      ? ((isSafariSpeechDelayBrowser() && !store.systemTtsNativeEventsReliable) ? 160 : 40)
      : 150
    speechRestartTimer = window.setTimeout(() => {
      if (store.isPaused) {
        isSpeechTransitioning = false
        return
      }
      isSpeechTransitioning = false
      startSpeech(paragraph, interruptCurrent)
    }, restartDelay)
  }

  function continueSpeechTarget(paragraph: HTMLElement | null, resetChunks = true) {
    logSpeech('continueSpeechTarget', {
      resetChunks,
      paragraph: paragraphPreview(paragraph),
      hasNextChapter: store.hasNext,
    })
    if (speechRestartTimer) {
      clearTimeout(speechRestartTimer)
    }

    const continueDelay = store.speechConfig.provider === 'system'
      ? ((isSafariSpeechDelayBrowser() && !store.systemTtsNativeEventsReliable) ? 160 : 40)
      : 0
    const chapterContinueDelay = store.speechConfig.provider === 'system' ? continueDelay : 40

    if (paragraph) {
      isSpeechTransitioning = true
      if (resetChunks) {
        resetSpeechChunkState()
      }
      speechRestartTimer = window.setTimeout(() => {
        if (store.isPaused) {
          isSpeechTransitioning = false
          return
        }
        isSpeechTransitioning = false
        startSpeech(paragraph, false)
      }, continueDelay)
      return
    }

    if (!store.hasNext) {
      store.stopTTS()
      clearReadingClass()
      return
    }

    isSpeechTransitioning = true
    if (resetChunks) {
      resetSpeechChunkState()
    }
    Promise.resolve(nextChapter())
      .then(() => {
        speechRestartTimer = window.setTimeout(() => {
          if (store.isPaused) {
            isSpeechTransitioning = false
            return
          }
          isSpeechTransitioning = false
          startSpeech(getFilteredParagraphs()[0] || null, false)
        }, chapterContinueDelay)
      })
      .catch(() => {
        isSpeechTransitioning = false
      })
  }

  function startSpeech(paragraph?: HTMLElement | null, interruptCurrent = true) {
    const current = paragraph || getCurrentParagraph()
    logSpeech('startSpeech', {
      interruptCurrent,
      paragraph: paragraphPreview(current),
      currentIndex: store.currentIndex,
    })
    if (!current?.innerText.trim()) {
      if (interruptCurrent) {
        speechNext()
      } else {
        continueSpeechTarget(getNextParagraph())
      }
      return
    }

    activeSpeechParagraph = current
    markReadingParagraph(current)
    showParagraph(current)
    const chunk = ensureSpeechChunkState(current)
    if (!chunk.text.trim()) {
      if (interruptCurrent) {
        speechNext(chunk.nextParagraph)
      } else {
        continueSpeechTarget(chunk.nextParagraph)
      }
      return
    }
    const nextParagraph = chunk.nextParagraph
    logSpeech('speak chunk', {
      interruptCurrent,
      provider: store.speechConfig.provider,
      text: chunk.text.slice(0, 60),
      nextParagraph: paragraphPreview(nextParagraph),
      chunkIndex: currentSpeechSegmentIndex,
      chunkCount: currentSpeechSegments.length,
    })
    store.startTTS(chunk.text, {
      onEnd: () => {
        logSpeech('chunk onEnd', {
          provider: store.speechConfig.provider,
          currentParagraph: paragraphPreview(current),
          nextParagraph: paragraphPreview(nextParagraph),
          chunkIndex: currentSpeechSegmentIndex,
          chunkCount: currentSpeechSegments.length,
        })
        if (store.speechConfig.provider !== 'system' && currentSpeechParagraph === current && currentSpeechSegmentIndex < currentSpeechSegments.length - 1) {
          currentSpeechSegmentIndex += 1
          continueSpeechTarget(current, false)
          return
        }
        continueSpeechTarget(nextParagraph)
      },
      onError: () => {
        logSpeech('chunk onError', {
          currentParagraph: paragraphPreview(current),
          nextParagraph: paragraphPreview(nextParagraph),
        })
        resetSpeechChunkState()
        clearReadingClass()
      },
    }, interruptCurrent)
    const preloadTexts = getUpcomingSpeechChunks(nextParagraph)
    if (preloadTexts.length) {
      window.setTimeout(() => {
        void store.preloadOpenAITTS(preloadTexts)
      }, 0)
    }
  }

  function speechPrev() {
    logSpeech('speechPrev', {
      currentParagraph: paragraphPreview(getCurrentParagraph()),
      hasPrevChapter: store.hasPrev,
    })
    resetSpeechChunkState()
    const prev = getPrevParagraph()
    if (prev) {
      restartSpeechTarget(prev)
      return
    }
    if (!store.hasPrev) {
      store.stopTTS()
      return
    }
    store.stopTTS(false)
    Promise.resolve(prevChapter()).then(() => {
      window.setTimeout(() => {
        const list = getFilteredParagraphs()
        restartSpeechTarget(list[list.length - 1] || null)
      }, 120)
    })
  }

  function speechNext(forcedNext?: HTMLElement | null, interruptCurrent = true) {
    logSpeech('speechNext', {
      interruptCurrent,
      forcedNext: paragraphPreview(forcedNext || null),
      currentParagraph: paragraphPreview(getCurrentParagraph()),
      hasNextChapter: store.hasNext,
    })
    resetSpeechChunkState()
    const next = forcedNext ?? getNextParagraph()
    if (next) {
      restartSpeechTarget(next, interruptCurrent)
      return
    }
    if (!store.hasNext) {
      store.stopTTS()
      clearReadingClass()
      return
    }
    if (interruptCurrent) {
      store.stopTTS(false)
    }
    Promise.resolve(nextChapter()).then(() => {
      window.setTimeout(() => {
        restartSpeechTarget(getFilteredParagraphs()[0] || null)
      }, 120)
    })
  }

  function restartSpeechFromCurrentParagraph() {
    logSpeech('restartSpeechFromCurrentParagraph', {
      currentParagraph: paragraphPreview(getCurrentParagraph()),
      isSpeechTransitioning,
    })
    if (isSpeechTransitioning) return
    isSpeechTransitioning = true
    resetSpeechChunkState()
    store.stopTTS(false)
    if (speechRestartTimer) {
      clearTimeout(speechRestartTimer)
    }
    speechRestartTimer = window.setTimeout(() => {
      if (store.isPaused) {
        isSpeechTransitioning = false
        return
      }
      isSpeechTransitioning = false
      startSpeech()
    }, 150)
  }

  function cancelSpeechTransition() {
    if (speechRestartTimer) {
      clearTimeout(speechRestartTimer)
      speechRestartTimer = null
    }
    isSpeechTransitioning = false
  }

  function resetAutoParagraphIndex() {
    autoReadingParagraphIndex = -1
  }

  function handleContentChanged() {
    autoReadingParagraphIndex = -1
    if (store.isAutoScrolling && usesParagraphAutoReading()) {
      if (autoParagraphTimer) {
        clearTimeout(autoParagraphTimer)
        autoParagraphTimer = null
      }
      window.setTimeout(() => {
        if (store.isAutoScrolling && usesParagraphAutoReading()) {
          runAutoParagraph()
        }
      }, 100)
    }
  }

  function disposeAutoPlayback() {
    cancelSpeechTransition()
    activeSpeechParagraph = null
    stopAutoScroll()
  }

  return {
    getCurrentParagraph,
    clearReadingClass,
    startAutoScroll,
    stopAutoScroll,
    startSpeech,
    speechPrev,
    speechNext,
    restartSpeechFromCurrentParagraph,
    cancelSpeechTransition,
    resetAutoParagraphIndex,
    handleContentChanged,
    disposeAutoPlayback,
  }
}
