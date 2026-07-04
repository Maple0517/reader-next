import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getV4Map,
  getV4MapConflicts,
  getV4MapGraph,
  getV4MapLayout,
  getV4MapPlaceDetail,
  getV4MapPlaces,
} from '../../api/v4/book'

vi.mock('../../api/v4/book', () => ({
  getV4Map: vi.fn(),
  getV4MapPlaces: vi.fn(),
  getV4MapPlaceDetail: vi.fn(),
  getV4MapGraph: vi.fn(),
  getV4MapLayout: vi.fn(),
  getV4MapConflicts: vi.fn(),
}))

const getV4MapMock = vi.mocked(getV4Map)
const getV4MapPlacesMock = vi.mocked(getV4MapPlaces)
const getV4MapPlaceDetailMock = vi.mocked(getV4MapPlaceDetail)
const getV4MapGraphMock = vi.mocked(getV4MapGraph)
const getV4MapLayoutMock = vi.mocked(getV4MapLayout)
const getV4MapConflictsMock = vi.mocked(getV4MapConflicts)

describe('V4MapPanel', () => {
  beforeEach(() => {
    getV4MapMock.mockReset()
    getV4MapPlacesMock.mockReset()
    getV4MapPlaceDetailMock.mockReset()
    getV4MapGraphMock.mockReset()
    getV4MapLayoutMock.mockReset()
    getV4MapConflictsMock.mockReset()
  })

  it('renders places, hierarchy, graph fallback, detail, conflict indicators, and avoids editor/image map UI', async () => {
    getV4MapMock.mockResolvedValue({
      placeCount: 2,
      activeEdgeCount: 1,
      conflictCount: 1,
      topPlaces: [{ id: 'place-a', name: '青云城', placeType: 'city' }],
    })
    getV4MapPlacesMock.mockResolvedValue({
      total: 2,
      places: [
        { id: 'place-a', name: '青云城', placeType: 'city' },
        { id: 'place-b', name: '黑风谷', placeType: 'dungeon' },
      ],
      hierarchy: [
        {
          placeId: 'place-a',
          name: '青云城',
          placeType: 'city',
          children: [{ placeId: 'place-b', name: '黑风谷', placeType: 'dungeon', children: [] }],
        },
      ],
    })
    getV4MapGraphMock.mockResolvedValue({
      nodes: [
        { placeId: 'place-a', label: '青云城', placeType: 'city' },
        { placeId: 'place-b', label: '黑风谷', placeType: 'dungeon' },
      ],
      edges: [{ edgeId: 'edge-1', fromPlaceId: 'place-a', toPlaceId: 'place-b', edgeType: 'route_to' }],
      layout: null,
      warnings: ['layout unavailable'],
    })
    getV4MapLayoutMock.mockResolvedValue({
      nodes: [],
      edges: [],
      warnings: ['layout unavailable'],
    })
    getV4MapConflictsMock.mockResolvedValue({
      total: 1,
      conflicts: [
        {
          id: 'conflict-1',
          newEdgeClaimId: 'claim-1',
          existingEdgeId: 'edge-1',
          conflictType: 'duplicate_conflicting_direction',
          reasonCode: 'opposite_direction',
          status: 'open',
          createdAt: '2026-07-04T00:00:00Z',
        },
      ],
    })
    getV4MapPlaceDetailMock.mockResolvedValue({
      id: 'place-b',
      name: '黑风谷',
      placeType: 'dungeon',
      linkedOrganizations: [{ id: 'org-1', name: '青云门', linkType: 'based_at' }],
    })

    const wrapper = mount(await import('./V4MapPanel.vue').then((mod) => mod.default), {
      props: { bookUrl: 'book-1' },
    })
    await flushPromises()

    expect(getV4MapMock).toHaveBeenCalledWith('book-1')
    expect(getV4MapPlacesMock).toHaveBeenCalledWith('book-1')
    expect(getV4MapGraphMock).toHaveBeenCalledWith('book-1')
    expect(getV4MapLayoutMock).toHaveBeenCalledWith('book-1')
    expect(getV4MapConflictsMock).toHaveBeenCalledWith('book-1')
    expect(wrapper.text()).toContain('青云城')
    expect(wrapper.text()).toContain('黑风谷')
    expect(wrapper.text()).toContain('路线')
    expect(wrapper.text()).toContain('布局暂不可用')
    expect(wrapper.text()).toContain('冲突 1')

    await wrapper.get('[data-test="map-place-place-b"]').trigger('click')
    await flushPromises()

    expect(getV4MapPlaceDetailMock).toHaveBeenCalledWith('book-1', 'place-b')
    expect(wrapper.text()).toContain('关联组织')
    expect(wrapper.text()).toContain('青云门')
    expect(wrapper.text()).not.toContain('纠错')
    expect(wrapper.text()).not.toContain('上传图片')
    expect(wrapper.text()).not.toContain('生成图片地图')
  })
})
