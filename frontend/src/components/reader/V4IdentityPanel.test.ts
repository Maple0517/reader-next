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

const sampleLinksResponse = {
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

const sampleOperationsResponse = {
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
}

describe('V4IdentityPanel', () => {
  beforeEach(() => {
    getV4IdentityLinksMock.mockReset()
    getV4MergeOperationsMock.mockReset()
  })

  async function mountPanel(props = { bookUrl: 'test-book' }) {
    const wrapper = mount(
      await import('./V4IdentityPanel.vue').then((mod) => mod.default),
      { props },
    )
    await flushPromises()
    return wrapper
  }

  it('uses V4PanelShell with Chinese title', async () => {
    getV4IdentityLinksMock.mockResolvedValue(sampleLinksResponse)
    getV4MergeOperationsMock.mockResolvedValue(sampleOperationsResponse)

    const wrapper = await mountPanel()

    const shell = wrapper.findComponent({ name: 'V4PanelShell' })
    expect(shell.exists()).toBe(true)
    expect(shell.props('title')).toBe('身份线索')
  })

  it('does not render Debug only pill', async () => {
    getV4IdentityLinksMock.mockResolvedValue(sampleLinksResponse)
    getV4MergeOperationsMock.mockResolvedValue(sampleOperationsResponse)

    const wrapper = await mountPanel()

    expect(wrapper.text()).not.toContain('Debug only')
  })

  it('shows Chinese labels for sections and metadata', async () => {
    getV4IdentityLinksMock.mockResolvedValue(sampleLinksResponse)
    getV4MergeOperationsMock.mockResolvedValue(sampleOperationsResponse)

    const wrapper = await mountPanel()

    // Section headers
    expect(wrapper.text()).toContain('身份链接')
    expect(wrapper.text()).toContain('合并操作')

    // Link metadata labels (Chinese)
    expect(wrapper.text()).toContain('来源：')
    expect(wrapper.text()).toContain('重定向目标：')

    // Operation metadata labels (Chinese)
    expect(wrapper.text()).toContain('原因：')
    expect(wrapper.text()).toContain('关系合并：')
    expect(wrapper.text()).toContain('属性冲突：')
  })

  it('shows identity link evidence fields', async () => {
    getV4IdentityLinksMock.mockResolvedValue(sampleLinksResponse)
    getV4MergeOperationsMock.mockResolvedValue(sampleOperationsResponse)

    const wrapper = await mountPanel()

    expect(getV4IdentityLinksMock).toHaveBeenCalledWith('test-book')
    expect(wrapper.text()).toContain('redirect')
    expect(wrapper.text()).toContain('96%')
    expect(wrapper.text()).toContain('claim-1')
    expect(wrapper.text()).toContain('char:new')
  })

  it('passes loading state to V4PanelShell', async () => {
    let resolveLinks!: (v: any) => void
    getV4IdentityLinksMock.mockReturnValue(
      new Promise<any>((r) => { resolveLinks = r }),
    )
    let resolveOps!: (v: any) => void
    getV4MergeOperationsMock.mockReturnValue(
      new Promise<any>((r) => { resolveOps = r }),
    )

    const wrapper = mount(
      (await import('./V4IdentityPanel.vue')).default,
      { props: { bookUrl: 'test-book' } },
    )

    const shell = wrapper.findComponent({ name: 'V4PanelShell' })
    expect(shell.props('loading')).toBe(true)

    resolveLinks(sampleLinksResponse)
    resolveOps(sampleOperationsResponse)
    await flushPromises()
  })

  it('passes error state to V4PanelShell on fetch failure', async () => {
    getV4IdentityLinksMock.mockRejectedValue(new Error('网络错误'))
    getV4MergeOperationsMock.mockResolvedValue(sampleOperationsResponse)

    const wrapper = await mountPanel()

    const shell = wrapper.findComponent({ name: 'V4PanelShell' })
    expect(shell.props('error')).toBeTruthy()
  })

  it('passes empty state to V4PanelShell when no data', async () => {
    getV4IdentityLinksMock.mockResolvedValue({ identityLinks: [], total: 0 })
    getV4MergeOperationsMock.mockResolvedValue({ mergeOperations: [], total: 0 })

    const wrapper = await mountPanel()

    const shell = wrapper.findComponent({ name: 'V4PanelShell' })
    expect(shell.props('empty')).toBe(true)
  })

  it('renders refresh button in toolbar slot', async () => {
    getV4IdentityLinksMock.mockResolvedValue(sampleLinksResponse)
    getV4MergeOperationsMock.mockResolvedValue(sampleOperationsResponse)

    const wrapper = await mountPanel()

    const refreshBtn = wrapper.find('button.v4-identity-refresh')
    expect(refreshBtn.exists()).toBe(true)
    expect(refreshBtn.text()).toBe('刷新')
  })
})
