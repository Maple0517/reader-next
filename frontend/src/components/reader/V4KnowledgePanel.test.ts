import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getV4Knowledge,
  getV4KnowledgeCard,
  getV4KnowledgeCategory,
} from '../../api/v4/book'
import { expectNoForbiddenV3AiBookBoundaryImports } from '../../../tests/helpers/v4BoundaryGuard'
import V4KnowledgePanel from './V4KnowledgePanel.vue'

vi.mock('../../api/v4/book', () => ({
  getV4Knowledge: vi.fn(),
  getV4KnowledgeCard: vi.fn(),
  getV4KnowledgeCategory: vi.fn(),
}))

const getV4KnowledgeMock = vi.mocked(getV4Knowledge)
const getV4KnowledgeCardMock = vi.mocked(getV4KnowledgeCard)
const getV4KnowledgeCategoryMock = vi.mocked(getV4KnowledgeCategory)

describe('V4KnowledgePanel', () => {
  beforeEach(() => {
    getV4KnowledgeMock.mockReset()
    getV4KnowledgeCardMock.mockReset()
    getV4KnowledgeCategoryMock.mockReset()
  })

  it('renders cards, filters categories, groups statuses, and shows referenced survivor chips', async () => {
    getV4KnowledgeMock.mockResolvedValue({
      total: 2,
      categories: [
        { category: 'history', count: 1 },
        { category: 'secret', count: 1 },
      ],
      cards: [
        {
          id: 'card-1',
          category: 'history',
          topicKey: 'old-war',
          topicDisplay: 'Old War',
          currentSummary: 'War summary.',
          confidence: 0.9,
          importanceScore: 0.8,
          firstSeenChapter: 1,
          lastUpdatedChapter: 2,
          assertionCount: 2,
        },
      ],
    })
    getV4KnowledgeCategoryMock.mockResolvedValue({
      category: 'secret',
      total: 1,
      cards: [
        {
          id: 'card-2',
          category: 'secret',
          topicKey: 'hidden-truth',
          topicDisplay: 'Hidden Truth',
          currentSummary: 'Secret summary.',
          confidence: 0.95,
          importanceScore: 0.9,
          firstSeenChapter: 3,
          lastUpdatedChapter: 4,
          assertionCount: 2,
        },
      ],
    })
    getV4KnowledgeCardMock.mockResolvedValue({
      card: {
        id: 'card-2',
        category: 'secret',
        topicKey: 'hidden-truth',
        topicDisplay: 'Hidden Truth',
        currentSummary: 'Secret summary.',
        confidence: 0.95,
        importanceScore: 0.9,
        firstSeenChapter: 3,
        lastUpdatedChapter: 4,
        assertionCount: 2,
      },
      assertionsByStatus: {
        active: [
          {
            id: 'assertion-1',
            assertionText: 'Truth exists.',
            status: 'active',
            confidence: 0.95,
            importanceScore: 0.9,
            chapterIndex: 4,
            referencedEntities: [
              {
                entityId: 'entity-survivor',
                displayName: 'Truth Concept',
                entityType: 'concept',
                role: 'related',
              },
            ],
          },
        ],
        rumor: [
          {
            id: 'assertion-2',
            assertionText: 'Rumor only.',
            status: 'rumor',
            confidence: 0.5,
            importanceScore: 0.4,
            chapterIndex: 2,
            referencedEntities: [],
          },
        ],
      },
    })

    const wrapper = mount(V4KnowledgePanel, {
      props: { bookUrl: 'test-book' },
    })
    await flushPromises()

    expect(getV4KnowledgeMock).toHaveBeenCalledWith('test-book')
    expect(wrapper.text()).toContain('Old War')

    await wrapper.get('[data-test="knowledge-category-secret"]').trigger('click')
    await flushPromises()
    expect(getV4KnowledgeCategoryMock).toHaveBeenCalledWith('test-book', 'secret')
    expect(wrapper.text()).toContain('Hidden Truth')

    await wrapper.get('[data-test="knowledge-card-card-2"]').trigger('click')
    await flushPromises()
    expect(getV4KnowledgeCardMock).toHaveBeenCalledWith('test-book', 'card-2')
    expect(wrapper.text()).toContain('已确认')
    expect(wrapper.text()).toContain('传闻')
    expect(wrapper.text()).toContain('Truth Concept')
    expect(wrapper.text()).not.toContain('纠错')
    expect(wrapper.text()).not.toContain('地图')
  })

  it('uses Knowledge Atlas cards with a selected topic inspector', async () => {
    getV4KnowledgeMock.mockResolvedValue({
      total: 1,
      categories: [{ category: 'world_rule', count: 1 }],
      cards: [
        {
          id: 'card-1',
          category: 'world_rule',
          topicKey: 'rule-1',
          topicDisplay: '魔法规则',
          currentSummary: '魔法需要咒文和材料。',
          confidence: 0.8,
          importanceScore: 0.7,
          firstSeenChapter: 1,
          lastUpdatedChapter: 3,
          assertionCount: 2,
        },
      ],
    })
    getV4KnowledgeCardMock.mockResolvedValue({
      card: {
        id: 'card-1',
        category: 'world_rule',
        topicKey: 'rule-1',
        topicDisplay: '魔法规则',
        currentSummary: '魔法需要咒文和材料。',
        confidence: 0.8,
        importanceScore: 0.7,
        firstSeenChapter: 1,
        lastUpdatedChapter: 3,
        assertionCount: 2,
      },
      assertionsByStatus: {
        active: [
          {
            id: 'assertion-1',
            assertionText: '咒文可以触发魔法。',
            status: 'active',
            confidence: 0.9,
            importanceScore: 0.8,
            chapterIndex: 3,
            referencedEntities: [],
          },
        ],
      },
    })

    const wrapper = mount(V4KnowledgePanel, { props: { bookUrl: 'test-book' } })
    await flushPromises()

    expect(wrapper.get('[data-test="knowledge-atlas"]').text()).toContain('Knowledge Atlas')
    expect(wrapper.get('[data-test="knowledge-topic-cards"]').text()).toContain('魔法规则')
    expect(wrapper.get('[data-test="knowledge-topic-inspector"]').text()).toContain('选择一个知识主题')
    expect(wrapper.text()).not.toContain('咒文可以触发魔法。')

    await wrapper.get('[data-test="knowledge-card-card-1"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="knowledge-topic-inspector"]').text()).toContain('魔法规则')
    expect(wrapper.get('[data-test="knowledge-topic-inspector"]').text()).toContain('咒文可以触发魔法。')
  })

  it('uses V4PanelShell with title and subtitle showing category/card counts', async () => {
    getV4KnowledgeMock.mockResolvedValue({
      total: 5,
      categories: [
        { category: 'history', count: 3 },
        { category: 'secret', count: 2 },
      ],
      cards: [
        {
          id: 'card-1',
          category: 'history',
          topicKey: 'topic-1',
          topicDisplay: 'Topic One',
          currentSummary: 'Summary.',
          confidence: 0.8,
          importanceScore: 0.7,
          firstSeenChapter: 0,
          lastUpdatedChapter: 3,
          assertionCount: 1,
        },
      ],
    })

    const wrapper = mount(V4KnowledgePanel, {
      props: { bookUrl: 'test-book' },
    })
    await flushPromises()

    // V4PanelShell renders title as h2
    const h2 = wrapper.find('h2')
    expect(h2.exists()).toBe(true)
    expect(h2.text()).toBe('知识')
    // subtitle shows counts
    const subtitle = wrapper.find('.v4-panel-shell-header p')
    expect(subtitle.exists()).toBe(true)
    expect(subtitle.text()).toContain('5')
    expect(subtitle.text()).toContain('2')
  })

  it('renders a refresh button in toolbar slot', async () => {
    getV4KnowledgeMock.mockResolvedValue({
      total: 1,
      categories: [],
      cards: [
        {
          id: 'card-1',
          category: 'custom',
          topicKey: 't1',
          topicDisplay: 'T1',
          currentSummary: null,
          confidence: 0.5,
          importanceScore: 0.5,
          firstSeenChapter: 0,
          lastUpdatedChapter: 0,
          assertionCount: 0,
        },
      ],
    })

    const wrapper = mount(V4KnowledgePanel, {
      props: { bookUrl: 'test-book' },
    })
    await flushPromises()

    const refreshBtn = wrapper.find('.v4-panel-shell-toolbar button')
    expect(refreshBtn.exists()).toBe(true)
    expect(refreshBtn.text()).toContain('刷新')
  })

  it('does not render a V4 pill', async () => {
    getV4KnowledgeMock.mockResolvedValue({
      total: 0,
      categories: [],
      cards: [],
    })

    const wrapper = mount(V4KnowledgePanel, {
      props: { bookUrl: 'test-book' },
    })
    await flushPromises()

    expect(wrapper.find('.knowledge-pill').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('V4')
  })

  it('renders empty state via V4PanelShell when no cards', async () => {
    getV4KnowledgeMock.mockResolvedValue({
      total: 0,
      categories: [],
      cards: [],
    })

    const wrapper = mount(V4KnowledgePanel, {
      props: { bookUrl: 'test-book' },
    })
    await flushPromises()

    expect(wrapper.find('.v4-empty-state').exists()).toBe(true)
  })

  it('renders error state via V4PanelShell when fetch fails', async () => {
    getV4KnowledgeMock.mockRejectedValue(new Error('V4 knowledge unavailable'))

    const wrapper = mount(V4KnowledgePanel, {
      props: { bookUrl: 'test-book' },
    })
    await flushPromises()

    expect(wrapper.text()).toContain('V4 knowledge unavailable')
  })

  it('does not import V3 AI Book boundaries', () => {
    expectNoForbiddenV3AiBookBoundaryImports('src/components/reader/V4KnowledgePanel.vue')
  })
})
