import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const pushMock = vi.fn()
const backMock = vi.fn()
const getShelfBookMock = vi.fn()

const readerBook = {
  name: '旧书',
  author: '作者',
  origin: 'source-1',
  bookUrl: 'book-1',
  durChapterIndex: 2,
  durChapterTitle: '第三章',
}

const aiStoreMock = {
  memoryView: {
    bookUrl: 'book-1',
    enabled: false,
    processedChapterIndex: 1,
    summary: { current: '', recentChanges: [], openQuestions: [] },
    characters: [],
    relationships: [],
    knowledgeFacts: [],
    locations: [],
    map: null,
    lastError: null,
  },
  chapterMemory: {
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
  },
  catchupStatus: null,
  isBusy: false,
  phase: 'idle',
  statusText: '',
  load: vi.fn(),
  loadChapterMemory: vi.fn(),
  setEnabled: vi.fn(),
  generateChapterMemory: vi.fn(),
  startCatchup: vi.fn(),
  cancelCatchup: vi.fn(),
  loadCatchupStatus: vi.fn(),
  reset: vi.fn(),
}

const appStoreMock = {
  fetchUserInfo: vi.fn().mockResolvedValue(undefined),
  showToast: vi.fn(),
}

const readerStoreMock = {
  book: readerBook,
  currentIndex: 2,
  currentChapter: { title: '第三章' },
}

vi.mock('vue-router', () => ({
  useRoute: () => ({ query: { bookUrl: 'book-1' } }),
  useRouter: () => ({ push: pushMock, back: backMock }),
}))

vi.mock('../api/bookshelf', () => ({
  getShelfBook: getShelfBookMock,
}))

vi.mock('../stores/aiBook', () => ({
  useAiBookStore: () => aiStoreMock,
}))

vi.mock('../stores/app', () => ({
  useAppStore: () => appStoreMock,
}))

vi.mock('../stores/reader', () => ({
  useReaderStore: () => readerStoreMock,
}))

describe('AiBookView', () => {
  beforeEach(() => {
    pushMock.mockReset()
    backMock.mockReset()
    getShelfBookMock.mockReset()
    aiStoreMock.load.mockReset()
    aiStoreMock.loadChapterMemory.mockReset()
    appStoreMock.fetchUserInfo.mockClear()
    appStoreMock.showToast.mockClear()
  })

  it('falls back to current reader book when shelf lookup returns 404', async () => {
    getShelfBookMock.mockRejectedValueOnce(new Error('Request failed with status code 404'))
    aiStoreMock.load.mockResolvedValueOnce(aiStoreMock.memoryView)
    aiStoreMock.loadChapterMemory.mockResolvedValueOnce(aiStoreMock.chapterMemory)

    const wrapper = mount(await import('./AiBookView.vue').then((mod) => mod.default))
    await flushPromises()

    expect(aiStoreMock.load).toHaveBeenCalledWith(readerBook)
    expect(aiStoreMock.loadChapterMemory).toHaveBeenCalledWith(readerBook.bookUrl, 2)
    expect(wrapper.text()).not.toContain('AI资料加载失败')
  })

  it('exposes a lightweight V4 identity debug tab', async () => {
    aiStoreMock.load.mockResolvedValueOnce(aiStoreMock.memoryView)
    aiStoreMock.loadChapterMemory.mockResolvedValueOnce(aiStoreMock.chapterMemory)

    const wrapper = mount(await import('./AiBookView.vue').then((mod) => mod.default))
    await flushPromises()

    expect(wrapper.findAll('.tabs button').map((button) => button.text())).toContain('身份')
  })

  it('exposes a lightweight V4 knowledge tab', async () => {
    aiStoreMock.load.mockResolvedValueOnce(aiStoreMock.memoryView)
    aiStoreMock.loadChapterMemory.mockResolvedValueOnce(aiStoreMock.chapterMemory)

    const wrapper = mount(await import('./AiBookView.vue').then((mod) => mod.default))
    await flushPromises()

    expect(wrapper.findAll('.tabs button').map((button) => button.text())).toContain('知识')
  })
})
