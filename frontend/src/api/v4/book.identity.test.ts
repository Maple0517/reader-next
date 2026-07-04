import { describe, expect, it, vi, beforeEach } from 'vitest'
import {
  getV4CharacterIdentity,
  getV4IdentityLinks,
  getV4MergeOperations,
} from './book'
import v4Http from './http'
import type {
  V4CharacterIdentityResponse,
  V4IdentityLinksResponse,
  V4MergeOperationsResponse,
} from '../../types/v4'

vi.mock('./http', () => ({
  default: {
    get: vi.fn(),
  },
}))

const httpGetMock = vi.mocked(v4Http.get)

describe('V4 identity API', () => {
  beforeEach(() => {
    httpGetMock.mockReset()
  })

  it('getV4IdentityLinks calls /identity-links with bookUrl param', async () => {
    const mockData: V4IdentityLinksResponse = {
      identityLinks: [
        {
          id: 'link-1',
          entityAId: 'char:old',
          entityBId: 'char:new',
          linkType: 'redirect',
          status: 'active',
          confidence: 0.96,
          sourceClaimId: 'claim-1',
          redirectTargetId: 'char:new',
        },
      ],
      total: 1,
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4IdentityLinks('test-book')

    expect(httpGetMock).toHaveBeenCalledWith('/identity-links', { params: { bookUrl: 'test-book' } })
    expect(result.identityLinks[0].redirectTargetId).toBe('char:new')
  })

  it('getV4CharacterIdentity encodes character id and preserves redirect target', async () => {
    const mockData: V4CharacterIdentityResponse = {
      characterId: 'char:old/id',
      redirectTargetId: 'char:new',
      identityLinks: [],
      total: 0,
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4CharacterIdentity('test-book', 'char:old/id')

    expect(httpGetMock).toHaveBeenCalledWith(
      `/characters/${encodeURIComponent('char:old/id')}/identity`,
      { params: { bookUrl: 'test-book' } },
    )
    expect(result.redirectTargetId).toBe('char:new')
  })

  it('getV4MergeOperations calls /merge-operations with bookUrl param', async () => {
    const mockData: V4MergeOperationsResponse = {
      mergeOperations: [
        {
          id: 'op-1',
          survivorEntityId: 'char:new',
          victimEntityId: 'char:old',
          sourceIdentityLinkId: 'link-1',
          reasonCode: 'explicit_reveal',
          confidence: 0.96,
          status: 'completed',
          propertyConflictCount: 0,
          relationshipMergeCount: 2,
          resultJson: '{"ok":true}',
          createdAt: '2026-07-03T00:00:00Z',
          completedAt: '2026-07-03T00:00:01Z',
        },
      ],
      total: 1,
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4MergeOperations('test-book')

    expect(httpGetMock).toHaveBeenCalledWith('/merge-operations', { params: { bookUrl: 'test-book' } })
    expect(result.mergeOperations[0].sourceIdentityLinkId).toBe('link-1')
  })
})
