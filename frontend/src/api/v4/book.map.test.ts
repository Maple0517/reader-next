import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getV4Map,
  getV4MapConflicts,
  getV4MapGraph,
  getV4MapLayout,
  getV4MapPlaceDetail,
  getV4MapPlaces,
} from './book'
import v4Http from './http'
import type {
  V4MapConflictsResponse,
  V4MapGraphView,
  V4MapLayoutView,
  V4MapOverviewView,
  V4MapPlacesResponse,
  V4PlaceDetailView,
} from '../../types/v4'

vi.mock('./http', () => ({
  default: {
    get: vi.fn(),
  },
}))

const httpGetMock = vi.mocked(v4Http.get)

describe('V4 map API', () => {
  beforeEach(() => {
    httpGetMock.mockReset()
  })

  it('getV4Map calls /map with bookUrl param', async () => {
    const mockData: V4MapOverviewView = {
      placeCount: 2,
      activeEdgeCount: 1,
      conflictCount: 0,
      topPlaces: [],
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4Map('test-book')

    expect(httpGetMock).toHaveBeenCalledWith('/map', { params: { bookUrl: 'test-book' } })
    expect(result.placeCount).toBe(2)
  })

  it('getV4MapPlaces calls /map/places', async () => {
    const mockData: V4MapPlacesResponse = {
      places: [],
      hierarchy: [],
      total: 0,
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    await getV4MapPlaces('test-book')

    expect(httpGetMock).toHaveBeenCalledWith('/map/places', { params: { bookUrl: 'test-book' } })
  })

  it('getV4MapPlaceDetail encodes place id', async () => {
    const mockData: V4PlaceDetailView = {
      id: 'place/1',
      name: '青云城',
      placeType: 'city',
      linkedOrganizations: [],
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    const result = await getV4MapPlaceDetail('test-book', 'place/1')

    expect(httpGetMock).toHaveBeenCalledWith(`/map/places/${encodeURIComponent('place/1')}`, { params: { bookUrl: 'test-book' } })
    expect(result.name).toBe('青云城')
  })

  it('getV4MapGraph and layout call expected routes', async () => {
    const graph: V4MapGraphView = {
      nodes: [],
      edges: [],
      layout: null,
      warnings: [],
    }
    const layout: V4MapLayoutView = {
      nodes: [],
      edges: [],
      warnings: [],
    }
    httpGetMock.mockResolvedValueOnce({ data: graph } as any)
    httpGetMock.mockResolvedValueOnce({ data: layout } as any)

    await getV4MapGraph('test-book')
    await getV4MapLayout('test-book')

    expect(httpGetMock).toHaveBeenNthCalledWith(1, '/map/graph', { params: { bookUrl: 'test-book' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(2, '/map/layout', { params: { bookUrl: 'test-book' } })
  })

  it('getV4MapConflicts calls /map/conflicts', async () => {
    const mockData: V4MapConflictsResponse = {
      conflicts: [],
      total: 0,
    }
    httpGetMock.mockResolvedValue({ data: mockData } as any)

    await getV4MapConflicts('test-book')

    expect(httpGetMock).toHaveBeenCalledWith('/map/conflicts', { params: { bookUrl: 'test-book' } })
  })
})
