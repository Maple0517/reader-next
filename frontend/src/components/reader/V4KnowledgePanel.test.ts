import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getV4Knowledge,
  getV4KnowledgeCard,
  getV4KnowledgeCategory,
} from '../../api/v4/book'

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

    const wrapper = mount(await import('./V4KnowledgePanel.vue').then((mod) => mod.default), {
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
})
