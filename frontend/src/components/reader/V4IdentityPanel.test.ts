import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getV4IdentityLinks,
  getV4MergeOperations,
} from '../../api/v4/book'

vi.mock('../../api/v4/book', () => ({
  getV4IdentityLinks: vi.fn(),
  getV4MergeOperations: vi.fn(),
}))

const getV4IdentityLinksMock = vi.mocked(getV4IdentityLinks)
const getV4MergeOperationsMock = vi.mocked(getV4MergeOperations)

describe('V4IdentityPanel', () => {
  beforeEach(() => {
    getV4IdentityLinksMock.mockReset()
    getV4MergeOperationsMock.mockReset()
  })

  it('shows identity link evidence fields without manual correction actions', async () => {
    getV4IdentityLinksMock.mockResolvedValue({
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
    })
    getV4MergeOperationsMock.mockResolvedValue({
      mergeOperations: [
        {
          id: 'merge-1',
          survivorEntityId: 'char:new',
          victimEntityId: 'char:old',
          sourceIdentityLinkId: 'link-1',
          reasonCode: 'explicit_reveal',
          confidence: 0.96,
          status: 'completed',
          propertyConflictCount: 0,
          relationshipMergeCount: 2,
          resultJson: null,
          createdAt: '2026-07-03T00:00:00Z',
          completedAt: '2026-07-03T00:00:01Z',
        },
      ],
      total: 1,
    })

    const wrapper = mount(await import('./V4IdentityPanel.vue').then((mod) => mod.default), {
      props: { bookUrl: 'test-book' },
    })
    await flushPromises()

    expect(getV4IdentityLinksMock).toHaveBeenCalledWith('test-book')
    expect(wrapper.text()).toContain('redirect')
    expect(wrapper.text()).toContain('96%')
    expect(wrapper.text()).toContain('claim-1')
    expect(wrapper.text()).toContain('char:new')
    expect(wrapper.text()).not.toContain('手动合并')
    expect(wrapper.text()).not.toContain('拆分')
  })
})
