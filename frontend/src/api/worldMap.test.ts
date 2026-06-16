import { beforeEach, describe, expect, it, vi } from 'vitest'
import http from './http'
import {
  buildWorldMap,
  generateCoordinates,
  getReviewItems,
  getWorldMapSpec,
  resolveReviewItem,
  saveWorldMapSpec,
  updateWorldMap,
} from './worldMap'
import type { WorldMapCoordinates, WorldMapSpec, UpdateWorldMapResponse } from '../types/worldMap'

vi.mock('./http', () => ({
  default: {
    get: vi.fn(),
    post: vi.fn(),
  },
}))

const httpMock = vi.mocked(http)

describe('worldMap api', () => {
  beforeEach(() => {
    httpMock.get.mockReset()
    httpMock.post.mockReset()
  })

  it('returns already-unwrapped response data from the shared http client', async () => {
    const spec = createSpec()
    const coordinates: WorldMapCoordinates = { placed: [], unplaced: [], status: 'Feasible' }
    const update: UpdateWorldMapResponse = {
      spec,
      added_entities: 1,
      added_relations: 2,
    }

    httpMock.get
      .mockResolvedValueOnce({ data: spec })
      .mockResolvedValueOnce({ data: [] })
    httpMock.post
      .mockResolvedValueOnce({ data: spec })
      .mockResolvedValueOnce({ data: spec })
      .mockResolvedValueOnce({ data: update })
      .mockResolvedValueOnce({ data: coordinates })
      .mockResolvedValueOnce({ data: 'ok' })

    await expect(getWorldMapSpec('book-1')).resolves.toBe(spec)
    await expect(buildWorldMap({ book_url: 'book-1', novel_title: '测试书' })).resolves.toBe(spec)
    await expect(saveWorldMapSpec(spec)).resolves.toBe(spec)
    await expect(updateWorldMap({ book_url: 'book-1', end_chapter: 3 })).resolves.toBe(update)
    await expect(generateCoordinates({ book_url: 'book-1' })).resolves.toBe(coordinates)
    await expect(getReviewItems('book-1')).resolves.toEqual([])
    await expect(resolveReviewItem({ book_url: 'book-1', item_id: 'review-1', resolution: 'accept' })).resolves.toBe('ok')
  })
})

function createSpec(): WorldMapSpec {
  return {
    metadata: {
      novel_title: '测试书',
      source_type: 'mock',
      start_chapter: 0,
      end_chapter: 0,
      allow_later_chapter_info: false,
      spec_version: '1.0',
      analysis_date: '2026-06-16',
    },
    entities: [],
    relations: [],
    routes: [],
    factions: [],
    constraints: {
      hard: [],
      soft: [],
      unknown_areas: [],
      forbidden_inferences: [],
    },
    conflicts: [],
    review_items: [],
    statistics: {
      total_entities: 0,
      total_relations: 0,
      total_routes: 0,
      total_factions: 0,
      total_hard_constraints: 0,
      total_soft_constraints: 0,
      total_conflicts: 0,
      total_review_items: 0,
      automation_rate: 0,
      coordinate_coverage_rate: 0,
    },
  }
}
