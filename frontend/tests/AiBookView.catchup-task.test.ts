import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AiBookView from '../src/views/AiBookView.vue'
import type { AiBookCatchupStatus, AiBookChapterMemoryViewModel, AiBookMemoryViewModel, Book } from '../src/types'

const routeMock = {
  query: {
    bookUrl: 'book-1',
  },
}

const routerBackMock = vi.fn()
const getShelfBookMock = vi.fn()
const fetchUserInfoMock = vi.fn()
const showToastMock = vi.fn()

let aiStoreMock: ReturnType<typeof createAiStoreMock>
let readerStoreMock: ReturnType<typeof createReaderStoreMock>

vi.mock('vue-router', () => ({
  useRoute: () => routeMock,
  useRouter: () => ({ back: routerBackMock }),
}))

vi.mock('../src/api/bookshelf', () => ({
  getShelfBook: (...args: unknown[]) => getShelfBookMock(...args),
}))

vi.mock('../src/stores/app', () => ({
  useAppStore: () => ({
    fetchUserInfo: fetchUserInfoMock,
    showToast: showToastMock,
  }),
}))

vi.mock('../src/stores/reader', () => ({
  useReaderStore: () => readerStoreMock,
}))

vi.mock('../src/stores/aiBook', () => ({
  useAiBookStore: () => aiStoreMock,
}))

describe('AiBookView catchup task controls', () => {
  beforeEach(() => {
    routerBackMock.mockReset()
    getShelfBookMock.mockReset()
    fetchUserInfoMock.mockReset()
    fetchUserInfoMock.mockResolvedValue(undefined)
    showToastMock.mockReset()
    routeMock.query.bookUrl = 'book-1'
    aiStoreMock = createAiStoreMock()
    readerStoreMock = createReaderStoreMock()
    getShelfBookMock.mockResolvedValue(createBook())
  })

  it('starts catchup and renders running status from store view model', async () => {
    aiStoreMock.startCatchup.mockResolvedValue(createCatchupStatus('running', { currentChapterIndex: 2, currentChapterTitle: '第三章', totalChapters: 3, completedChapters: 1 }))
    const wrapper = mount(AiBookView)
    await flushPromises()

    const button = wrapper.findAll('button.secondary-btn')[0]
    expect(button.text()).toContain('补齐到当前进度')
    await button.trigger('click')
    await flushPromises()

    expect(aiStoreMock.startCatchup).toHaveBeenCalledWith({
      bookUrl: 'book-1',
      targetChapterIndex: 2,
    })
    expect(wrapper.text()).toContain('补齐任务 · 运行中')
    expect(wrapper.text()).toContain('当前处理 第 3 章 · 第三章')
  })

  it('cancels catchup and reloads visible status text', async () => {
    aiStoreMock.loadCatchupStatus.mockResolvedValue(createCatchupStatus('running', { totalChapters: 3, completedChapters: 1 }))
    aiStoreMock.cancelCatchup.mockResolvedValue(createCatchupStatus('canceled', { processedChapterIndex: 2, processedChapterTitle: '第三章' }))
    const wrapper = mount(AiBookView)
    await flushPromises()

    await aiStoreMock.startCatchup({ bookUrl: 'book-1', targetChapterIndex: 2 })
    ;(wrapper.vm as unknown)
    const button = wrapper.findAll('button.secondary-btn')[0]
    await button.trigger('click')
    await flushPromises()

    expect(aiStoreMock.cancelCatchup).toHaveBeenCalledWith('book-1')
    expect(wrapper.text()).toContain('补齐任务 · 已取消')
    expect(wrapper.text()).toContain('最近完成 第 3 章 · 第三章')
  })

  it('shows catchup error text from store failure state', async () => {
    aiStoreMock.loadCatchupStatus.mockResolvedValue(createCatchupStatus('failed', { error: '上游限流', totalChapters: 4, completedChapters: 2 }))
    const wrapper = mount(AiBookView)
    await flushPromises()

    expect(aiStoreMock.loadCatchupStatus).toHaveBeenCalledWith('book-1')
    expect(wrapper.text()).toContain('补齐任务 · 失败')
    expect(wrapper.text()).toContain('上游限流')
  })

  it('keeps AI book load errors visible instead of rendering a blank page', async () => {
    getShelfBookMock.mockRejectedValueOnce(new Error('书籍不存在'))
    const wrapper = mount(AiBookView)
    await flushPromises()

    expect(wrapper.text()).toContain('AI资料加载失败')
    expect(wrapper.text()).toContain('书籍不存在')
  })
})

function createBook(): Book {
  return {
    name: '山海旧事',
    author: '佚名',
    bookUrl: 'book-1',
    origin: 'source-1',
    durChapterIndex: 2,
    durChapterTitle: '第三章',
  }
}

function createMemoryView(): AiBookMemoryViewModel {
  return {
    bookUrl: 'book-1',
    bookName: '山海旧事',
    author: '佚名',
    enabled: true,
    processedChapterIndex: 1,
    processedChapterTitle: '第二章',
    updatedAt: 1710000000000,
    summary: { current: '摘要', recentChanges: [], openQuestions: [] },
    characters: [],
    relationships: [],
    knowledgeFacts: [],
    locations: [],
    map: null,
    cleanup: { droppedFactsCount: 0, droppedByReason: {}, oldSchemaBackedUp: false },
    catchupStats: null,
    lastError: null,
    lastErrorChapterIndex: null,
    lastErrorChapterTitle: null,
  }
}

function createChapterMemory(): AiBookChapterMemoryViewModel {
  return {
    bookUrl: 'book-1',
    chapterIndex: 2,
    chapterTitle: '第三章',
    digest: null,
    characters: [],
    relationships: [],
    knowledgeFacts: [],
    locations: [],
    generationStatus: 'cached',
    lastError: null,
  }
}

function createCatchupStatus(status: AiBookCatchupStatus['status'], overrides: Partial<AiBookCatchupStatus> = {}): AiBookCatchupStatus {
  return {
    status,
    bookUrl: 'book-1',
    totalChapters: 0,
    completedChapters: 0,
    updatedAt: 1710000000000,
    error: null,
    stats: null,
    ...overrides,
  }
}

function createAiStoreMock() {
  return {
    memoryView: createMemoryView(),
    chapterMemory: createChapterMemory(),
    phase: 'idle',
    statusText: '',
    isBusy: false,
    load: vi.fn().mockResolvedValue(createMemoryView()),
    loadChapterMemory: vi.fn().mockResolvedValue(createChapterMemory()),
    generateChapterMemory: vi.fn(),
    generateMap: vi.fn(),
    setEnabled: vi.fn(),
    reset: vi.fn(),
    startCatchup: vi.fn().mockResolvedValue(createCatchupStatus('running')),
    loadCatchupStatus: vi.fn().mockResolvedValue(createCatchupStatus('idle')),
    cancelCatchup: vi.fn().mockResolvedValue(createCatchupStatus('canceled')),
  }
}

function createReaderStoreMock() {
  return {
    book: createBook(),
    currentIndex: 2,
    currentChapter: {
      title: '第三章',
    },
  }
}
