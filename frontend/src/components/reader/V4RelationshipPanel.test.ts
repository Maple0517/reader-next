import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { getV4Relationships } from '../../api/v4/book'
import { expectNoForbiddenV3AiBookBoundaryImports } from '../../../tests/helpers/v4BoundaryGuard'
import V4RelationshipPanel from './V4RelationshipPanel.vue'

vi.mock('../../api/v4/book', () => ({
  getV4Relationships: vi.fn(),
}))

const getV4RelationshipsMock = vi.mocked(getV4Relationships)

const MOCK_NODES = [
  { id: 'n1', name: '林玄', aliases: ['玄子'], importance: 0.8, firstSeenChapter: 0, lastSeenChapter: 10 },
  { id: 'n2', name: '苏瑶', aliases: [], importance: 0.6, firstSeenChapter: 1, lastSeenChapter: 8 },
  { id: 'n3', name: '张老', aliases: ['师父'], importance: 0.9, firstSeenChapter: 0, lastSeenChapter: 15 },
]

const MOCK_EDGES = [
  {
    id: 'e1',
    sourceId: 'n1',
    targetId: 'n2',
    group: 'romance',
    label: '暗恋',
    directionality: 'directed',
    currentState: '单相思',
    strength: 0.7,
    polarity: 'positive',
    confidence: 0.85,
    importanceScore: 0.6,
    firstSeenChapter: 2,
    lastChangedChapter: 5,
    lastSeenChapter: 8,
    eventCount: 5,
    latestSourceClaimId: 'claim-1',
    evidenceAvailable: true,
  },
  {
    id: 'e2',
    sourceId: 'n1',
    targetId: 'n3',
    group: 'mentorship',
    label: '师徒',
    directionality: 'undirected',
    currentState: '信任',
    strength: 0.9,
    polarity: 'positive',
    confidence: 0.92,
    importanceScore: 0.8,
    firstSeenChapter: 0,
    lastChangedChapter: 3,
    lastSeenChapter: 15,
    eventCount: 12,
    latestSourceClaimId: 'claim-2',
    evidenceAvailable: true,
  },
  {
    id: 'e3',
    sourceId: 'n2',
    targetId: 'n3',
    group: 'hostility',
    label: '敌对',
    directionality: 'directed',
    strength: 0.3,
    polarity: 'negative',
    confidence: 0.5,
    importanceScore: 0.15,
    firstSeenChapter: 6,
    lastChangedChapter: 6,
    lastSeenChapter: 7,
    eventCount: 2,
    latestSourceClaimId: '',
    evidenceAvailable: false,
  },
]

const MOCK_RESPONSE = {
  nodes: MOCK_NODES,
  edges: MOCK_EDGES,
  groups: ['romance', 'mentorship', 'hostility'],
  total: 3,
}

describe('V4RelationshipPanel', () => {
  beforeEach(() => {
    getV4RelationshipsMock.mockReset()
  })

  it('renders V4PanelShell with correct title', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.find('.v4-panel-shell-header h2').text()).toBe('人物关系')
    expect(wrapper.find('.v4-panel-shell').exists()).toBe(true)
  })

  it('shows subtitle with total and group count', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const subtitle = wrapper.find('.v4-panel-shell-header p')
    expect(subtitle.text()).toContain('3 条关系')
    expect(subtitle.text()).toContain('3 个分组')
  })

  it('calls getV4Relationships with bookUrl', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(getV4RelationshipsMock).toHaveBeenCalledWith('book-1')
  })

  it('shows loading state via V4PanelShell', () => {
    getV4RelationshipsMock.mockReturnValue(new Promise(() => {}))
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })

    expect(wrapper.findComponent({ name: 'V4LoadingState' }).exists()).toBe(true)
  })

  it('shows error state when fetcher fails', async () => {
    getV4RelationshipsMock.mockRejectedValue(new Error('network down'))
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const errorState = wrapper.findComponent({ name: 'V4ErrorState' })
    expect(errorState.exists()).toBe(true)
  })

  it('shows empty state when no edges', async () => {
    getV4RelationshipsMock.mockResolvedValue({
      nodes: [],
      edges: [],
      groups: [],
      total: 0,
    })
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const emptyState = wrapper.findComponent({ name: 'V4EmptyState' })
    expect(emptyState.exists()).toBe(true)
  })

  it('renders relationship edges with source, arrow, target, label', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const directedItem = wrapper.find('[data-edge-id="e1"]')
    expect(directedItem.exists()).toBe(true)
    expect(directedItem.text()).toContain('林玄')
    expect(directedItem.text()).toContain('苏瑶')
    expect(directedItem.text()).toContain('→')
    expect(directedItem.text()).toContain('暗恋')
  })

  it('renders network, edge list, and selected edge inspector regions', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.get('[data-test="relationship-network"]').text()).toContain('林玄')
    expect(wrapper.get('[data-test="relationship-edge-list"]').text()).toContain('暗恋')
    expect(wrapper.get('[data-test="relationship-edge-inspector"]').text()).toContain('选中关系')

    await wrapper.get('[data-edge-id="e1"]').trigger('click')
    await flushPromises()

    const inspector = wrapper.get('[data-test="relationship-edge-inspector"]')
    expect(inspector.text()).toContain('林玄')
    expect(inspector.text()).toContain('苏瑶')
    expect(inspector.text()).toContain('暗恋')
    expect(inspector.text()).toContain('事件 5')
    expect(inspector.text()).not.toContain('claim-1')
  })

  it('renders undirected arrow for undirected edges', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const undirectedItem = wrapper.find('[data-edge-id="e2"]')
    expect(undirectedItem.exists()).toBe(true)
    expect(undirectedItem.text()).toContain('↔')
  })

  it('renders polarity labels', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.find('[data-edge-id="e1"]').text()).toContain('正面')
    expect(wrapper.find('[data-edge-id="e3"]').text()).toContain('负面')
  })

  it('applies low-importance styling for edges with importanceScore < 0.3', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const lowItem = wrapper.find('[data-edge-id="e3"]')
    expect(lowItem.classes()).toContain('low-importance')
  })

  it('expands edge detail on click', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.find('[data-edge-detail="e1"]').exists()).toBe(false)

    await wrapper.find('[data-edge-id="e1"]').trigger('click')
    await flushPromises()

    const detail = wrapper.find('[data-edge-detail="e1"]')
    expect(detail.exists()).toBe(true)
    expect(detail.text()).toContain('单相思')
    expect(detail.text()).toContain('5')
  })

  it('collapses edge detail on second click', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    await wrapper.find('[data-edge-id="e1"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-edge-detail="e1"]').exists()).toBe(true)

    await wrapper.find('[data-edge-id="e1"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-edge-detail="e1"]').exists()).toBe(false)
  })

  it('renders group filter pills', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const pills = wrapper.findAll('.group-pill')
    expect(pills.length).toBe(4)
    expect(pills[0].text()).toContain('全部')
    expect(pills[1].text()).toContain('情感')
    expect(pills[1].text()).toContain('1')
  })

  it('filters edges when group pill is clicked', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const romancePill = wrapper.findAll('.group-pill').find(p => p.text().includes('情感'))
    expect(romancePill).toBeDefined()
    await romancePill!.trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-edge-id="e1"]').exists()).toBe(true)
    expect(wrapper.find('[data-edge-id="e2"]').exists()).toBe(false)
    expect(wrapper.find('[data-edge-id="e3"]').exists()).toBe(false)
  })

  it('renders refresh button in toolbar', async () => {
    getV4RelationshipsMock.mockResolvedValue(MOCK_RESPONSE)
    const wrapper = mount(V4RelationshipPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    const refreshBtn = wrapper.find('.v4-relationship-refresh')
    expect(refreshBtn.exists()).toBe(true)
    expect(refreshBtn.text()).toContain('刷新')
  })

  it('does not import V3 AI book boundary code', () => {
    expectNoForbiddenV3AiBookBoundaryImports('src/components/reader/V4RelationshipPanel.vue')
  })
})
