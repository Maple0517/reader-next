import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAppStore } from './app'
import { useReaderStore } from './reader'
import { getBookContent, getChapterList, getShelfBook, saveBookProgress } from '../api/bookshelf'
import { getBrowserCachedChapter, setBrowserCachedChapter } from '../utils/browserCache'
import type { Book } from '../types'

vi.mock('../api/bookshelf', () => ({
  getChapterList: vi.fn(),
  getBookContent: vi.fn(),
  getShelfBook: vi.fn(),
  saveBookProgress: vi.fn(),
  setBookSource: vi.fn(),
}))

vi.mock('../api/bookmark', () => ({
  getBookmarks: vi.fn(),
  saveBookmark: vi.fn(),
  deleteBookmark: vi.fn(),
  deleteBookmarks: vi.fn(),
}))

vi.mock('../api/replaceRule', () => ({
  getReplaceRules: vi.fn(),
}))

vi.mock('../utils/browserCache', () => ({
  getBrowserCachedChapter: vi.fn(),
  setBrowserCachedChapter: vi.fn(),
}))

vi.mock('../utils/recentBooks', () => ({
  saveRecentReadBook: vi.fn(),
}))

vi.mock('../utils/openaiSpeech', () => ({
  DEFAULT_OPENAI_BASE_URL: 'https://api.openai.com/v1',
  requestOpenAISpeechAudio: vi.fn(),
}))


const aiBookStoreMock = {
  memoryView: null as any,
  isServerModelAdmin: false,
  canUseServerModel: false,
  serverModelConfig: null as any,
  load: vi.fn(),
  generateChapterMemory: vi.fn(),
  loadChapterMemory: vi.fn(),
  loadServerModelConfig: vi.fn().mockResolvedValue(null),
}

vi.mock('./aiBook', () => ({
  useAiBookStore: () => aiBookStoreMock,
}))

describe('reader local txt chapters', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    const storage = new Map<string, string>()
    vi.stubGlobal('localStorage', {
      getItem: vi.fn((key: string) => storage.get(key) ?? null),
      setItem: vi.fn((key: string, value: string) => storage.set(key, value)),
      removeItem: vi.fn((key: string) => storage.delete(key)),
      clear: vi.fn(() => storage.clear()),
    })
    vi.mocked(getBookContent).mockReset()
    vi.mocked(getChapterList).mockReset()
    vi.mocked(getShelfBook).mockReset()
    vi.mocked(saveBookProgress).mockReset()
    vi.mocked(saveBookProgress).mockResolvedValue('ok')
    vi.mocked(getBrowserCachedChapter).mockReset()
    vi.mocked(setBrowserCachedChapter).mockReset()
  })

  it('switches chinese mode back to simplified after traditional conversion is loaded', async () => {
    const readerStore = useReaderStore()
    readerStore.content = '爱学习'

    readerStore.updateConfig('chineseMode', 'traditional')
    await vi.dynamicImportSettled()
    expect(readerStore.displayContent).toBe('愛學習')

    readerStore.updateConfig('chineseMode', 'simplified')

    expect(readerStore.displayContent).toBe('爱学习')
  })

  it('fetches uploaded local txt content from backend even when browser reports offline', async () => {
    vi.mocked(getBookContent).mockResolvedValue('本地正文')
    vi.mocked(getBrowserCachedChapter).mockResolvedValue(null)
    const appStore = useAppStore()
    const readerStore = useReaderStore()
    appStore.setOnlineStatus(false)
    readerStore.book = {
      name: '本地书',
      author: '本地导入',
      origin: 'local-txt',
      bookUrl: 'local-txt:abc123',
    }
    readerStore.chapters = [
      { title: '第一章', url: 'local-txt:abc123#0', index: 0 },
    ]

    await expect(readerStore.fetchChapterContent(0)).resolves.toBe('本地正文')

    expect(getBrowserCachedChapter).not.toHaveBeenCalled()
    expect(getBookContent).toHaveBeenCalledWith({
      chapterUrl: 'local-txt:abc123#0',
      bookSourceUrl: 'local-txt',
      refresh: 0,
    })
  })

  it('loads the latest server reading progress before opening a stale local book', async () => {
    const staleBook: Book = {
      name: '同步书',
      author: '作者',
      origin: 'source-1',
      bookUrl: 'book-1',
      durChapterIndex: 1,
      durChapterPos: 1200,
      durChapterTitle: '旧章节',
    }
    const serverBook: Book = {
      ...staleBook,
      durChapterIndex: 5,
      durChapterPos: 7200,
      durChapterTime: 1_765_000_000,
      durChapterTitle: '新章节',
    }
    vi.mocked(getShelfBook).mockResolvedValue(serverBook)
    vi.mocked(getChapterList).mockResolvedValue([
      { title: '第1章', url: 'chapter-1', index: 0 },
      { title: '第2章', url: 'chapter-2', index: 1 },
      { title: '第3章', url: 'chapter-3', index: 2 },
      { title: '第4章', url: 'chapter-4', index: 3 },
      { title: '第5章', url: 'chapter-5', index: 4 },
      { title: '第6章', url: 'chapter-6', index: 5 },
    ])
    const readerStore = useReaderStore()

    await readerStore.loadBook(staleBook)

    expect(getShelfBook).toHaveBeenCalledWith('book-1')
    expect(readerStore.book?.durChapterIndex).toBe(5)
    expect(readerStore.book?.durChapterPos).toBe(7200)
    expect(readerStore.currentIndex).toBe(5)
    expect(getChapterList).toHaveBeenCalledWith({
      bookUrl: 'book-1',
      bookSourceUrl: 'source-1',
    })
  })

  it('prefers newer server progress when restoring the persisted reader session', async () => {
    const localBook: Book = {
      name: '恢复书',
      author: '作者',
      origin: 'source-1',
      bookUrl: 'book-restore',
      durChapterIndex: 1,
      durChapterPos: 1000,
      durChapterTitle: '旧章节',
    }
    const serverBook: Book = {
      ...localBook,
      durChapterIndex: 4,
      durChapterPos: 6400,
      durChapterTime: 1_765_000_000,
      durChapterTitle: '新章节',
    }
    const chapters = [
      { title: '第1章', url: 'chapter-1', index: 0 },
      { title: '第2章', url: 'chapter-2', index: 1 },
      { title: '第3章', url: 'chapter-3', index: 2 },
      { title: '第4章', url: 'chapter-4', index: 3 },
      { title: '第5章', url: 'chapter-5', index: 4 },
    ]
    localStorage.setItem('reader-last-session', JSON.stringify({
      book: localBook,
      chapters,
      currentIndex: 1,
      chapterScrollProgress: 0.1,
      updatedAt: 1_000,
    }))
    vi.mocked(getShelfBook).mockResolvedValue(serverBook)
    vi.mocked(getChapterList).mockResolvedValue(chapters)
    vi.mocked(getBookContent).mockResolvedValue('服务端进度章节正文')
    vi.mocked(getBrowserCachedChapter).mockResolvedValue(null)
    vi.mocked(setBrowserCachedChapter).mockResolvedValue(undefined)
    const appStore = useAppStore()
    appStore.setOnlineStatus(true)
    const readerStore = useReaderStore()

    const restored = await readerStore.restorePersistedSession()

    expect(getShelfBook).toHaveBeenCalledWith('book-restore')
    expect(getChapterList).toHaveBeenCalledWith({
      bookUrl: 'book-restore',
      bookSourceUrl: 'source-1',
    })
    expect(readerStore.chapters.length).toBe(5)
    expect(getBrowserCachedChapter).toHaveBeenCalledWith('book-restore', 'chapter-5')
    expect(getBookContent).toHaveBeenCalledWith({
      chapterUrl: 'chapter-5',
      bookSourceUrl: 'source-1',
      refresh: 0,
    })
    expect(restored).toBe(true)
    expect(readerStore.currentIndex).toBe(4)
    expect(readerStore.book?.durChapterPos).toBe(6400)
  })

  it('keeps newer local session even when server has a deeper older chapter', async () => {
    const localBook: Book = {
      name: '恢复书',
      author: '作者',
      origin: 'source-1',
      bookUrl: 'book-restore',
      durChapterIndex: 0,
      durChapterPos: 0,
      durChapterTitle: '第1章',
    }
    const serverBook: Book = {
      ...localBook,
      durChapterIndex: 4,
      durChapterPos: 6400,
      durChapterTime: 1_765_000_000,
      durChapterTitle: '第5章',
    }
    const chapters = [
      { title: '第1章', url: 'chapter-1', index: 0 },
      { title: '第2章', url: 'chapter-2', index: 1 },
      { title: '第3章', url: 'chapter-3', index: 2 },
      { title: '第4章', url: 'chapter-4', index: 3 },
      { title: '第5章', url: 'chapter-5', index: 4 },
    ]
    localStorage.setItem('reader-last-session', JSON.stringify({
      book: localBook,
      chapters,
      currentIndex: 0,
      chapterScrollProgress: 0,
      updatedAt: Date.now(),
    }))
    vi.mocked(getShelfBook).mockResolvedValue(serverBook)
    vi.mocked(getChapterList).mockResolvedValue(chapters)
    vi.mocked(getBookContent).mockResolvedValue('服务端进度章节正文')
    vi.mocked(getBrowserCachedChapter).mockResolvedValue(null)
    vi.mocked(setBrowserCachedChapter).mockResolvedValue(undefined)
    useAppStore().setOnlineStatus(true)
    const readerStore = useReaderStore()

    const restored = await readerStore.restorePersistedSession()

    expect(restored).toBe(true)
    expect(readerStore.currentIndex).toBe(0)
    expect(readerStore.book?.durChapterPos).toBe(0)
    expect(getBookContent).toHaveBeenCalledWith({
      chapterUrl: 'chapter-1',
      bookSourceUrl: 'source-1',
      refresh: 0,
    })
  })
})

describe('reader summary display config', () => {
  beforeEach(() => {
    localStorage.clear()
    setActivePinia(createPinia())
  })

  it('defaults key points to card style', () => {
    const store = useReaderStore()
    expect(store.config.chapterSummaryKeyPointStyle).toBe('card')
  })
})


describe('reader ai book auto-update', () => {
  beforeEach(() => {
    aiBookStoreMock.memoryView = null
    aiBookStoreMock.load.mockReset()
    aiBookStoreMock.generateChapterMemory.mockReset()
    aiBookStoreMock.loadChapterMemory.mockReset()
    aiBookStoreMock.loadServerModelConfig.mockReset()
    aiBookStoreMock.loadServerModelConfig.mockResolvedValue(null)
  })

  it('reader auto-update no longer passes chapterContent into aiBook store', async () => {
    aiBookStoreMock.load.mockResolvedValue({ enabled: true, bookUrl: 'book-1' })
    aiBookStoreMock.generateChapterMemory.mockResolvedValue({})
    vi.mocked(getBookContent).mockResolvedValue('下一章正文')
    vi.mocked(getBrowserCachedChapter).mockResolvedValue(null)
    vi.mocked(setBrowserCachedChapter).mockResolvedValue(undefined)

    const appStore = useAppStore()
    appStore.setOnlineStatus(true)
    const readerStore = useReaderStore()
    readerStore.book = {
      name: '测试书',
      author: '作者',
      origin: 'source-1',
      bookUrl: 'book-1',
    }
    readerStore.chapters = [
      { title: '第一章', url: 'chapter-1', index: 0 },
      { title: '第二章', url: 'chapter-2', index: 1 },
    ]
    readerStore.currentIndex = 0
    readerStore.content = '第一章正文'

    await readerStore.nextChapter()

    expect(aiBookStoreMock.load).toHaveBeenCalledWith(expect.objectContaining({ bookUrl: 'book-1' }))
    expect(aiBookStoreMock.generateChapterMemory).toHaveBeenCalledWith({
      bookUrl: 'book-1',
      chapterIndex: 0,
      mode: 'auto',
    })
    expect(aiBookStoreMock.generateChapterMemory.mock.calls[0][0]).not.toHaveProperty('chapterContent')
  })


  it('swallows background ai book generate failures after chapter switch', async () => {
    aiBookStoreMock.load.mockResolvedValue({ enabled: true, bookUrl: 'book-1' })
    aiBookStoreMock.generateChapterMemory.mockRejectedValue(new Error('AI失败'))
    vi.mocked(getBookContent).mockResolvedValue('下一章正文')
    vi.mocked(getBrowserCachedChapter).mockResolvedValue(null)
    vi.mocked(setBrowserCachedChapter).mockResolvedValue(undefined)

    const appStore = useAppStore()
    appStore.setOnlineStatus(true)
    const readerStore = useReaderStore()
    readerStore.book = {
      name: '测试书',
      author: '作者',
      origin: 'source-1',
      bookUrl: 'book-1',
    }
    readerStore.chapters = [
      { title: '第一章', url: 'chapter-1', index: 0 },
      { title: '第二章', url: 'chapter-2', index: 1 },
    ]
    readerStore.currentIndex = 0
    readerStore.content = '第一章正文'

    await expect(readerStore.nextChapter()).resolves.toBeUndefined()
    await Promise.resolve()
    await Promise.resolve()

    expect(aiBookStoreMock.generateChapterMemory).toHaveBeenCalledWith({
      bookUrl: 'book-1',
      chapterIndex: 0,
      mode: 'auto',
    })
  })
})
