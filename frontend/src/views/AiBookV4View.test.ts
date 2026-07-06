import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { expectNoForbiddenV3AiBookBoundaryImports } from '../../tests/helpers/v4BoundaryGuard'

const backMock = vi.fn()
const getShelfBookMock = vi.fn()
const getV4MemoryStatusMock = vi.fn()
const getV4CatchupStatusMock = vi.fn()
const generateV4ChapterMemoryMock = vi.fn()
const startV4CatchupMock = vi.fn()
const resetV4MemoryMock = vi.fn()

const routeMock = {
  query: {
    bookUrl: 'book-1' as string | undefined,
    chapterIndex: undefined as string | undefined,
  },
}

vi.mock('vue-router', () => ({
  useRoute: () => routeMock,
  useRouter: () => ({ back: backMock }),
}))

vi.mock('../api/bookshelf', () => ({
  getShelfBook: (...args: unknown[]) => getShelfBookMock(...args),
}))

vi.mock('../api/v4/book', () => ({
  getV4MemoryStatus: (...args: unknown[]) => getV4MemoryStatusMock(...args),
  getV4CatchupStatus: (...args: unknown[]) => getV4CatchupStatusMock(...args),
  generateV4ChapterMemory: (...args: unknown[]) => generateV4ChapterMemoryMock(...args),
  startV4Catchup: (...args: unknown[]) => startV4CatchupMock(...args),
  resetV4Memory: (...args: unknown[]) => resetV4MemoryMock(...args),
}))

vi.mock('../components/reader/V4BookOverviewPanel.vue', () => ({
  default: { props: ['bookUrl'], template: '<section data-test="v4-overview-panel">overview {{ bookUrl }}</section>' },
}))
vi.mock('../components/reader/V4CharacterPanel.vue', () => ({
  default: { props: ['bookUrl'], template: '<section data-test="v4-character-panel">characters {{ bookUrl }}</section>' },
}))
vi.mock('../components/reader/V4RelationshipPanel.vue', () => ({
  default: { props: ['bookUrl'], template: '<section data-test="v4-relationship-panel">relationships {{ bookUrl }}</section>' },
}))
vi.mock('../components/reader/V4KnowledgePanel.vue', () => ({
  default: { props: ['bookUrl'], template: '<section data-test="v4-knowledge-panel">knowledge {{ bookUrl }}</section>' },
}))
vi.mock('../components/reader/V4MapPanel.vue', () => ({
  default: { props: ['bookUrl'], template: '<section data-test="v4-map-panel">map {{ bookUrl }}</section>' },
}))
vi.mock('../components/reader/V4IdentityPanel.vue', () => ({
  default: { props: ['bookUrl'], template: '<section data-test="v4-identity-panel">identity {{ bookUrl }}</section>' },
}))
vi.mock('../components/reader/V4QualityPanel.vue', () => ({
  default: { props: ['bookUrl'], template: '<section data-test="v4-quality-panel">quality {{ bookUrl }}</section>' },
}))

describe('AiBookV4View shell', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  beforeEach(() => {
    backMock.mockReset()
    getShelfBookMock.mockReset()
    getV4MemoryStatusMock.mockReset()
    getV4CatchupStatusMock.mockReset()
    generateV4ChapterMemoryMock.mockReset()
    startV4CatchupMock.mockReset()
    resetV4MemoryMock.mockReset()
    routeMock.query.bookUrl = 'book-1'
    routeMock.query.chapterIndex = undefined
    getShelfBookMock.mockResolvedValue({
      name: '山海旧事',
      author: '佚名',
      bookUrl: 'book-1',
      origin: 'source-1',
      durChapterIndex: 4,
      durChapterTitle: '第五章',
    })
    getV4MemoryStatusMock.mockResolvedValue({
      maxReadChapter: 8,
      maxProcessedChapter: 5,
      processing: false,
      lastError: null,
    })
    getV4CatchupStatusMock.mockResolvedValue({
      status: 'idle',
      targetChapterIndex: null,
      currentChapterIndex: null,
      error: null,
    })
    generateV4ChapterMemoryMock.mockResolvedValue({})
    startV4CatchupMock.mockResolvedValue({})
    resetV4MemoryMock.mockResolvedValue({})
  })

  it('renders a V4-only shell with bookUrl route input', async () => {
    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    expect(getShelfBookMock).toHaveBeenCalledWith('book-1')
    expect(getV4MemoryStatusMock).toHaveBeenCalledWith('book-1')
    expect(wrapper.text()).toContain('山海旧事')
    expect(wrapper.text()).toContain('AI Book Memory V4')
    expect(wrapper.text()).not.toContain('后端 V3 视图模型')
    expect(wrapper.text()).not.toContain('V3 角色')
  })

  it('renders a route-level error when bookUrl is missing', async () => {
    routeMock.query.bookUrl = undefined

    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    expect(getShelfBookMock).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('缺少 bookUrl')
  })

  it('mounts V4-only tab panels without nested V3/V4 toggles', async () => {
    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    expect(wrapper.findAll('.v4-tabs button').map((button) => button.text())).toEqual([
      '总览',
      '角色',
      '关系',
      '知识',
      '地图',
      '身份',
      '质量',
    ])
    expect(wrapper.text()).not.toContain('V3 角色')
    expect(wrapper.text()).not.toContain('V4 角色')
    expect(wrapper.text()).not.toContain('V3 关系')
    expect(wrapper.text()).not.toContain('V4 关系')
    expect(wrapper.get('[data-test="v4-overview-panel"]').text()).toContain('book-1')

    await wrapper.findAll('.v4-tabs button').find((button) => button.text() === '角色')!.trigger('click')
    expect(wrapper.get('[data-test="v4-character-panel"]').text()).toContain('book-1')
    await wrapper.findAll('.v4-tabs button').find((button) => button.text() === '关系')!.trigger('click')
    expect(wrapper.get('[data-test="v4-relationship-panel"]').text()).toContain('book-1')
    await wrapper.findAll('.v4-tabs button').find((button) => button.text() === '质量')!.trigger('click')
    expect(wrapper.get('[data-test="v4-quality-panel"]').text()).toContain('book-1')
  })

  it('does not import V3 AI Book store, API, or view-model types', () => {
    expectNoForbiddenV3AiBookBoundaryImports('src/views/AiBookV4View.vue')
  })

  it('renders V4-safe header actions', async () => {
    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    expect(wrapper.findAll('button').some((button) => button.text() === '刷新状态')).toBe(true)
    expect(wrapper.findAll('button').some((button) => button.text() === '生成当前章节')).toBe(true)
    expect(wrapper.findAll('button').some((button) => button.text() === '补齐到当前阅读')).toBe(true)
    expect(wrapper.findAll('button').some((button) => button.text() === '重置 V4 资料')).toBe(true)
  })

  it('generates the route chapter first and refreshes V4 status', async () => {
    routeMock.query.chapterIndex = '2'
    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    await wrapper.findAll('button').find((button) => button.text() === '生成当前章节')!.trigger('click')
    await flushPromises()

    expect(generateV4ChapterMemoryMock).toHaveBeenCalledWith({ bookUrl: 'book-1', chapterIndex: 2 })
    expect(getV4MemoryStatusMock).toHaveBeenCalledTimes(2)
    expect(getV4CatchupStatusMock).toHaveBeenCalledTimes(2)
  })

  it('falls back to shelf reading chapter for generate and catchup actions', async () => {
    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    await wrapper.findAll('button').find((button) => button.text() === '生成当前章节')!.trigger('click')
    await flushPromises()
    await wrapper.findAll('button').find((button) => button.text() === '补齐到当前阅读')!.trigger('click')
    await flushPromises()

    expect(generateV4ChapterMemoryMock).toHaveBeenCalledWith({ bookUrl: 'book-1', chapterIndex: 4 })
    expect(startV4CatchupMock).toHaveBeenCalledWith({ bookUrl: 'book-1', targetChapterIndex: 4 })
  })

  it('disables current-chapter actions when current chapter is unavailable', async () => {
    getShelfBookMock.mockResolvedValue({
      name: '山海旧事',
      author: '佚名',
      bookUrl: 'book-1',
      origin: 'source-1',
      durChapterIndex: null,
      durChapterTitle: '',
    })

    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    expect(wrapper.findAll('button').find((button) => button.text() === '生成当前章节')!.attributes('disabled')).toBeDefined()
    expect(wrapper.findAll('button').find((button) => button.text() === '补齐到当前阅读')!.attributes('disabled')).toBeDefined()
  })

  it('disables catchup while V4 processing is active', async () => {
    getV4MemoryStatusMock.mockResolvedValue({
      maxReadChapter: 8,
      maxProcessedChapter: 5,
      processing: true,
      lastError: null,
    })

    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    const catchupButton = wrapper.findAll('button').find((button) => button.text().includes('补齐'))!
    expect(catchupButton.text()).toBe('补齐运行中')
    expect(catchupButton.attributes('disabled')).toBeDefined()
  })

  it('requires confirmation before resetting V4 memory and refreshes status after reset', async () => {
    const confirmMock = vi.fn().mockReturnValueOnce(false).mockReturnValueOnce(true)
    vi.stubGlobal('confirm', confirmMock)
    const AiBookV4View = await import('./AiBookV4View.vue').then((mod) => mod.default)
    const wrapper = mount(AiBookV4View)
    await flushPromises()

    await wrapper.findAll('button').find((button) => button.text() === '重置 V4 资料')!.trigger('click')
    await flushPromises()
    expect(resetV4MemoryMock).not.toHaveBeenCalled()

    await wrapper.findAll('button').find((button) => button.text() === '重置 V4 资料')!.trigger('click')
    await flushPromises()

    expect(resetV4MemoryMock).toHaveBeenCalledWith('book-1')
    expect(getV4MemoryStatusMock).toHaveBeenCalledTimes(2)
    expect(getV4CatchupStatusMock).toHaveBeenCalledTimes(2)
  })
})
