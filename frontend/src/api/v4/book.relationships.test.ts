import { describe, expect, it, vi, beforeEach } from 'vitest'
import { getV4Relationships, getV4CharacterRelationships } from './book'
import v4Http from './http'
import type { V4RelationshipGraphView, V4CharacterRelationshipsResponse } from '../../types/v4'

vi.mock('./http', () => ({
  default: {
    get: vi.fn(),
  },
}))

const httpGetMock = vi.mocked(v4Http.get)

describe('V4 relationship API', () => {
  beforeEach(() => {
    httpGetMock.mockReset()
  })

  describe('getV4Relationships', () => {
    it('calls /relationships with bookUrl param', async () => {
      const mockData: V4RelationshipGraphView = {
        nodes: [
          { id: 'e1', name: 'Alice', aliases: [], importance: 0.9, firstSeenChapter: 1, lastSeenChapter: 5 },
          { id: 'e2', name: 'Bob', aliases: [], importance: 0.7, firstSeenChapter: 1, lastSeenChapter: 3 },
        ],
        edges: [
          {
            id: 'r1',
            sourceId: 'e1',
            targetId: 'e2',
            group: 'friendship',
            label: 'friends',
            directionality: 'undirected',
            strength: 0.8,
            polarity: 'positive',
            confidence: 0.9,
            importanceScore: 0.7,
            firstSeenChapter: 1,
            lastChangedChapter: 3,
            lastSeenChapter: 5,
            eventCount: 2,
            latestSourceClaimId: 'claim-1',
            evidenceAvailable: true,
          },
        ],
        groups: ['friendship'],
        total: 1,
      }
      httpGetMock.mockResolvedValue({ data: mockData } as any)

      const result = await getV4Relationships('test-book')
      expect(httpGetMock).toHaveBeenCalledWith('/relationships', { params: { bookUrl: 'test-book' } })
      expect(result).toEqual(mockData)
      expect(result.edges).toHaveLength(1)
      expect(result.edges[0].sourceId).toBe('e1')
    })

    it('passes group and minImportance filters', async () => {
      httpGetMock.mockResolvedValue({ data: { nodes: [], edges: [], groups: [], total: 0 } } as any)

      await getV4Relationships('test-book', { group: 'family', minImportance: 0.5 })
      expect(httpGetMock).toHaveBeenCalledWith('/relationships', {
        params: { bookUrl: 'test-book', group: 'family', minImportance: 0.5 },
      })
    })

    it('works without optional params', async () => {
      httpGetMock.mockResolvedValue({ data: { nodes: [], edges: [], groups: [], total: 0 } } as any)

      await getV4Relationships('test-book')
      expect(httpGetMock).toHaveBeenCalledWith('/relationships', { params: { bookUrl: 'test-book' } })
    })
  })

  describe('getV4CharacterRelationships', () => {
    it('calls /characters/:id/relationships with bookUrl', async () => {
      const mockData: V4CharacterRelationshipsResponse = {
        relationships: [
          {
            id: 'r1',
            sourceId: 'e1',
            targetId: 'e2',
            group: 'mentorship',
            label: 'mentor',
            directionality: 'directed',
            currentState: 'teaching',
            strength: 0.9,
            polarity: 'positive',
            confidence: 0.95,
            importanceScore: 0.8,
            firstSeenChapter: 1,
            lastChangedChapter: 5,
            lastSeenChapter: 10,
            eventCount: 3,
            latestSourceClaimId: 'claim-2',
            evidenceAvailable: true,
          },
        ],
        total: 1,
      }
      httpGetMock.mockResolvedValue({ data: mockData } as any)

      const result = await getV4CharacterRelationships('test-book', 'e1')
      expect(httpGetMock).toHaveBeenCalledWith('/characters/e1/relationships', { params: { bookUrl: 'test-book' } })
      expect(result.relationships).toHaveLength(1)
      expect(result.relationships[0].group).toBe('mentorship')
    })

    it('encodes characterId in URL', async () => {
      httpGetMock.mockResolvedValue({ data: { relationships: [], total: 0 } } as any)

      await getV4CharacterRelationships('test-book', 'char:special/id')
      expect(httpGetMock).toHaveBeenCalledWith(
        `/characters/${encodeURIComponent('char:special/id')}/relationships`,
        { params: { bookUrl: 'test-book' } },
      )
    })
  })
})
