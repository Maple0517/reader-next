import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getV4Knowledge,
  getV4KnowledgeCard,
  getV4KnowledgeCategory,
} from './book'
import v4Http from './http'
import type {
  V4KnowledgeCardDetailView,
  V4KnowledgeCategoryView,
  V4KnowledgeOverviewView,
} from '../../types/v4'

vi.mock('./http', () => ({
  default: {
    get: vi.fn(),
  },
}))

const httpGetMock = vi.mocked(v4Http.get)

describe('V4 knowledge API', () => {
  beforeEach(() => {
    httpGetMock.mockReset()
  })

  it('getV4Knowledge calls /knowledge with bookUrl param', async () => {
    const mockData: V4KnowledgeOverviewView = {
      cards: [],
      categories: [{ category: 'history', count: 1 }],
      total: 1,
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4Knowledge('test-book')

    expect(httpGetMock).toHaveBeenCalledWith('/knowledge', { params: { bookUrl: 'test-book' } })
    expect(result.categories[0].category).toBe('history')
  })

  it('getV4KnowledgeCategory encodes category and preserves cards', async () => {
    const mockData: V4KnowledgeCategoryView = {
      category: 'world_rule',
      cards: [
        {
          id: 'card-1',
          category: 'world_rule',
          topicKey: 'mana-rules',
          topicDisplay: 'Mana Rules',
          currentSummary: 'Mana has rules.',
          confidence: 0.9,
          importanceScore: 0.8,
          firstSeenChapter: 1,
          lastUpdatedChapter: 2,
          assertionCount: 2,
        },
      ],
      total: 1,
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4KnowledgeCategory('test-book', 'world_rule')

    expect(httpGetMock).toHaveBeenCalledWith('/knowledge/categories/world_rule', { params: { bookUrl: 'test-book' } })
    expect(result.cards[0].topicDisplay).toBe('Mana Rules')
  })

  it('getV4KnowledgeCard encodes card id and returns grouped assertions', async () => {
    const mockData: V4KnowledgeCardDetailView = {
      card: {
        id: 'card/1',
        category: 'secret',
        topicKey: 'hidden-truth',
        topicDisplay: 'Hidden Truth',
        currentSummary: 'Secret summary.',
        confidence: 0.95,
        importanceScore: 0.9,
        firstSeenChapter: 1,
        lastUpdatedChapter: 3,
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
            chapterIndex: 3,
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
      },
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4KnowledgeCard('test-book', 'card/1')

    expect(httpGetMock).toHaveBeenCalledWith(`/knowledge/cards/${encodeURIComponent('card/1')}`, { params: { bookUrl: 'test-book' } })
    expect(result.assertionsByStatus.active[0].referencedEntities[0].displayName).toBe('Truth Concept')
  })
})
