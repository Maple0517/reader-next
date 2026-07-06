import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { getV4CatchupStatus, getV4Map, getV4Memory, getV4MemoryStatus, getV4Quality } from '../../api/v4/book'
import { expectNoForbiddenV3AiBookBoundaryImports, readFrontendSource } from '../../../tests/helpers/v4BoundaryGuard'
import V4BookOverviewPanel from './V4BookOverviewPanel.vue'

vi.mock('../../api/v4/book', () => ({
  getV4Memory: vi.fn(),
  getV4MemoryStatus: vi.fn(),
  getV4CatchupStatus: vi.fn(),
  getV4Map: vi.fn(),
  getV4Quality: vi.fn(),
}))

const getV4MemoryMock = vi.mocked(getV4Memory)
const getV4MemoryStatusMock = vi.mocked(getV4MemoryStatus)
const getV4CatchupStatusMock = vi.mocked(getV4CatchupStatus)
const getV4MapMock = vi.mocked(getV4Map)
const getV4QualityMock = vi.mocked(getV4Quality)

describe('V4BookOverviewPanel', () => {
  beforeEach(() => {
    getV4MemoryMock.mockReset()
    getV4MemoryStatusMock.mockReset()
    getV4CatchupStatusMock.mockReset()
    getV4MapMock.mockReset()
    getV4QualityMock.mockReset()
  })

  it('renders V4 aggregate summaries without V3 labels', async () => {
    getV4MemoryMock.mockResolvedValue({
      bookUrl: 'book-1',
      bookName: '山海旧事',
      author: '佚名',
      maxReadChapter: 8,
      maxProcessedChapter: 5,
      processing: false,
      characterCount: 3,
      relationshipCount: 2,
      knowledgeCount: 5,
    })
    getV4MemoryStatusMock.mockResolvedValue({ maxReadChapter: 8, maxProcessedChapter: 5, processing: false, lastError: null })
    getV4CatchupStatusMock.mockResolvedValue({ status: 'idle', targetChapter: null, currentChapter: null, maxProcessedChapter: 5, lastError: null })
    getV4MapMock.mockResolvedValue({ placeCount: 4, activeEdgeCount: 3, conflictCount: 1, topPlaces: [] })
    getV4QualityMock.mockResolvedValue({
      bookUrl: 'book-1',
      qualityMetrics: { open_findings: 2 },
      auditRuns: [],
      findings: [
        {
          id: 'finding-1',
          findingType: 'duplicate_entity',
          severity: 'medium',
          targetType: 'entity',
          targetId: 'entity-1',
          reasonCode: 'duplicate_name',
          suggestedAction: 'review',
          status: 'open',
        },
        {
          id: 'finding-2',
          findingType: 'relationship_pollution',
          severity: 'low',
          targetType: 'relationship',
          targetId: 'rel-1',
          reasonCode: 'low_confidence',
          suggestedAction: 'review',
          status: 'open',
        },
      ],
      corrections: [
        {
          id: 'correction-1',
          targetType: 'entity',
          targetId: 'entity-1',
          correctionType: 'merge_entities',
          status: 'proposed',
          source: 'audit',
        },
      ],
      reprocessJobs: [],
      promptRegressionRuns: [],
    })

    const wrapper = mount(V4BookOverviewPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(getV4MemoryMock).toHaveBeenCalledWith('book-1')
    expect(wrapper.text()).toContain('角色 3')
    expect(wrapper.text()).toContain('关系 2')
    expect(wrapper.text()).toContain('知识 5')
    expect(wrapper.text()).toContain('地点 4')
    expect(wrapper.text()).toContain('冲突 1')
    expect(wrapper.text()).toContain('质量发现 2')
    expect(wrapper.text()).toContain('已处理 第 6 章')
    expect(wrapper.text()).not.toContain('后端 V3 视图模型')
    expect(wrapper.text()).not.toContain('digest.characterStates')
  })

  it('keeps partial V4 data visible when one domain fails', async () => {
    getV4MemoryMock.mockResolvedValue({
      bookUrl: 'book-1',
      bookName: '山海旧事',
      author: '佚名',
      maxReadChapter: 8,
      maxProcessedChapter: 5,
      processing: false,
      characterCount: 3,
      relationshipCount: 2,
      knowledgeCount: 5,
    })
    getV4MemoryStatusMock.mockResolvedValue({ maxReadChapter: 8, maxProcessedChapter: 5, processing: false, lastError: null })
    getV4CatchupStatusMock.mockRejectedValue(new Error('catchup unavailable'))
    getV4MapMock.mockRejectedValue(new Error('map unavailable'))
    getV4QualityMock.mockResolvedValue({
      bookUrl: 'book-1',
      qualityMetrics: {},
      auditRuns: [],
      findings: [],
      corrections: [],
      reprocessJobs: [],
      promptRegressionRuns: [],
    })

    const wrapper = mount(V4BookOverviewPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.text()).toContain('角色 3')
    expect(wrapper.text()).toContain('地图不可用')
    expect(wrapper.text()).toContain('质量发现 0')
  })

  it('does not import V3 boundaries or the heavy relationship graph endpoint for counts', () => {
    expectNoForbiddenV3AiBookBoundaryImports('src/components/reader/V4BookOverviewPanel.vue')
    expect(readFrontendSource('src/components/reader/V4BookOverviewPanel.vue')).not.toContain('getV4Relationships')
  })
})
