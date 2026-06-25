import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AiBookView from '../src/views/AiBookView.vue'
import type {
  AiBookCatchupStatus,
  AiBookChapterMemoryViewModel,
  AiBookMemoryViewModel,
  Book,
} from '../src/types'

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

describe('AiBookView v3 behavior', () => {
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

  it('aiBook_view_renders_relationships_from_view_model', async () => {
    const wrapper = mount(AiBookView)
    await flushPromises()

    await wrapper.get('nav.tabs button:nth-child(3)').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('林舟')
    expect(wrapper.text()).toContain('苏九')
    expect(wrapper.text()).toContain('盟友')
    expect(wrapper.text()).toContain('并肩闯过外门试炼')
  })

  it('aiBook_view_renders_character_state_from_view_model', async () => {
    const wrapper = mount(AiBookView)
    await flushPromises()

    expect(wrapper.text()).toContain('角色状态')
    expect(wrapper.text()).toContain('林舟')
    expect(wrapper.text()).toContain('灵力恢复，准备夜探藏经阁')
    expect(wrapper.text()).toContain('第三章')
  })

  it('aiBook_view_calls_generate_action', async () => {
    const wrapper = mount(AiBookView)
    await flushPromises()

    const button = wrapper.get('button.primary-btn')
    expect(button.text()).toContain('生成当前章节')
    await button.trigger('click')
    await flushPromises()

    expect(aiStoreMock.generateChapterMemory).toHaveBeenCalledWith({
      bookUrl: 'book-1',
      chapterIndex: 2,
      mode: 'manual',
    })
  })



  it('disables generate and map actions while ai store is busy', async () => {
    aiStoreMock.isBusy = true
    const wrapper = mount(AiBookView)
    await flushPromises()

    const generateButton = wrapper.get('button.primary-btn')
    expect(generateButton.attributes('disabled')).toBeDefined()
    await generateButton.trigger('click')
    await flushPromises()
    expect(aiStoreMock.generateChapterMemory).not.toHaveBeenCalled()

    await wrapper.get('nav.tabs button:nth-child(4)').trigger('click')
    await flushPromises()

    const mapButton = wrapper.get('.map-head .secondary-btn')
    expect(mapButton.attributes('disabled')).toBeDefined()
    await mapButton.trigger('click')
    await flushPromises()
    expect(aiStoreMock.generateMap).not.toHaveBeenCalled()
  })


  it('hides stale memory when current book load fails after book lookup succeeds', async () => {
    aiStoreMock.memoryView = { ...createMemoryView(), bookUrl: 'other-book', summary: { current: '旧书摘要', recentChanges: [], openQuestions: [] } }
    getShelfBookMock.mockResolvedValueOnce(createBook())
    aiStoreMock.load.mockRejectedValueOnce(new Error('AI资料加载失败'))
    const wrapper = mount(AiBookView)
    await flushPromises()

    expect(wrapper.text()).toContain('AI资料加载失败')
    expect(wrapper.text()).not.toContain('旧书摘要')
  })

  it('hides stale chapter memory when store chapter does not match current book or chapter', async () => {
    aiStoreMock.chapterMemory = createChapterMemory({
      bookUrl: 'other-book',
      chapterIndex: 99,
      chapterTitle: '旧章节',
      digestSummary: '旧章节摘要',
      characterStateStatus: '旧状态',
    })
    const wrapper = mount(AiBookView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('旧章节摘要')
    expect(wrapper.text()).not.toContain('旧状态')
    expect(wrapper.text()).toContain('当前章节还没有摘要')
  })

  it('shows disabled map notice in V3 cutover', async () => {
    const wrapper = mount(AiBookView)
    await flushPromises()

    await wrapper.get('nav.tabs button:nth-child(4)').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('地图生成功能已暂时禁用')
    expect(wrapper.text()).toContain('V3 切换期间，地图生成与持久化暂未接入')
  })

  it('disables map action during V3 cutover', async () => {
    const wrapper = mount(AiBookView)
    await flushPromises()

    await wrapper.get('nav.tabs button:nth-child(4)').trigger('click')
    await flushPromises()

    const button = wrapper.get('.map-head .secondary-btn')
    expect(button.attributes('disabled')).toBeDefined()
    await button.trigger('click')
    await flushPromises()

    expect(aiStoreMock.generateMap).not.toHaveBeenCalled()
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
    summary: {
      current: '林舟在外门试炼后获得新线索。',
      recentChanges: ['林舟与苏九结成临时盟友'],
      openQuestions: ['黑石碑文的来历仍未知'],
    },
    characters: [
      {
        id: 'char-lz',
        name: '林舟',
        aliases: ['阿舟'],
        importance: 'high',
        description: '出身寒门的少年修士',
        firstSeenChapterIndex: 0,
        lastSeenChapterIndex: 2,
        evidence: [],
      },
      {
        id: 'char-sj',
        name: '苏九',
        aliases: [],
        importance: 'medium',
        description: '擅长阵法的同门',
        firstSeenChapterIndex: 1,
        lastSeenChapterIndex: 2,
        evidence: [],
      },
    ],
    relationships: [
      {
        id: 'rel-lz-sj',
        sourceCharacterId: 'char-lz',
        targetCharacterId: 'char-sj',
        kind: 'alliance',
        label: '盟友',
        polarity: 'positive',
        strength: 'moderate',
        status: 'active',
        direction: 'directed',
        summary: '并肩闯过外门试炼',
        currentDynamics: [],
        facets: [],
        evidence: [],
        history: [],
      },
    ],
    knowledgeFacts: [],
    locations: [],
    map: {
      state: {
        dirty: false,
        nodes: [],
        edges: [],
      },
      renderArtifacts: {
        imageUrl: 'https://example.com/map.png',
        updatedAt: 1710000000000,
      },
      locations: [],
      locationEdges: [],
    },
    cleanup: {
      droppedFactsCount: 0,
      droppedByReason: {},
      oldSchemaBackedUp: false,
    },
    catchupStats: null,
    lastError: null,
    lastErrorChapterIndex: null,
    lastErrorChapterTitle: null,
  }
}

function createChapterMemory(overrides: {
  bookUrl?: string
  chapterIndex?: number
  chapterTitle?: string
  digestSummary?: string
  characterStateStatus?: string
} = {}): AiBookChapterMemoryViewModel {
  const chapterIndex = overrides.chapterIndex ?? 2
  const chapterTitle = overrides.chapterTitle ?? '第三章'
  const digestSummary = overrides.digestSummary ?? '林舟夜探藏经阁前恢复灵力，并与苏九达成合作。'
  const characterStateStatus = overrides.characterStateStatus ?? '灵力恢复，准备夜探藏经阁'
  return {
    bookUrl: overrides.bookUrl ?? 'book-1',
    chapterIndex,
    chapterTitle,
    digest: {
      chapterIndex,
      chapterTitle,
      summary: digestSummary,
      keyPoints: ['夜探藏经阁前恢复灵力', '与苏九达成合作'],
      characters: [
        {
          name: '林舟',
          aliases: ['阿舟'],
          status: characterStateStatus,
          description: '状态稳定',
          lastSeenChapter: chapterTitle,
        },
      ],
      characterStates: [
        {
          name: '林舟',
          status: characterStateStatus,
          description: '状态稳定',
          lastSeenChapterIndex: chapterIndex,
          lastSeenChapterTitle: chapterTitle,
        },
      ],
      characterRelations: [
        {
          source: '林舟',
          target: '苏九',
          kind: 'alliance',
          polarity: 'positive',
          strength: 'moderate',
          status: 'active',
          description: '共同调查黑石碑文',
        },
      ],
      knowledgeFacts: [],
      locations: [],
      locationEdges: [],
    },
    characters: [],
    relationships: [],
    knowledgeFacts: [],
    locations: [],
    generationStatus: 'generated',
    lastError: null,
  }
}

function createCatchupStatus(status: AiBookCatchupStatus['status'] = 'idle'): AiBookCatchupStatus {
  return {
    status,
    bookUrl: 'book-1',
    totalChapters: 0,
    completedChapters: 0,
    updatedAt: 1710000000000,
    error: null,
    stats: null,
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
    generateChapterMemory: vi.fn().mockResolvedValue(createChapterMemory()),
    generateMap: vi.fn().mockResolvedValue(createMemoryView()),
    setEnabled: vi.fn().mockResolvedValue(createMemoryView()),
    reset: vi.fn().mockResolvedValue(createMemoryView()),
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
