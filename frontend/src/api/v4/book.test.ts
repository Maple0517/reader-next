import { beforeEach, describe, expect, it, vi } from 'vitest'

const getMock = vi.fn()
const postMock = vi.fn()

vi.mock('./http', () => ({
  default: {
    get: getMock,
    post: postMock,
  },
}))

const api = await import('./book')

describe('v4 book api', () => {
  beforeEach(() => {
    getMock.mockReset()
    postMock.mockReset()
  })

  it('getV4Memory calls /memory with bookUrl param', async () => {
    getMock.mockResolvedValueOnce({ data: { bookUrl: 'b1', characterCount: 3 } })
    const result = await api.getV4Memory('b1')
    expect(getMock).toHaveBeenCalledWith('/memory', { params: { bookUrl: 'b1' } })
    expect(result).toEqual({ bookUrl: 'b1', characterCount: 3 })
  })

  it('getV4Characters calls /characters with bookUrl param', async () => {
    getMock.mockResolvedValueOnce({ data: { characters: [], total: 0 } })
    const result = await api.getV4Characters('b1')
    expect(getMock).toHaveBeenCalledWith('/characters', { params: { bookUrl: 'b1' } })
    expect(result).toEqual({ characters: [], total: 0 })
  })

  it('getV4CharacterCard calls /characters/:id with bookUrl param', async () => {
    getMock.mockResolvedValueOnce({
      data: { character: { id: 'c1', name: 'Test' }, relationshipCount: 0 },
    })
    const result = await api.getV4CharacterCard('b1', 'c1')
    expect(getMock).toHaveBeenCalledWith('/characters/c1', { params: { bookUrl: 'b1' } })
    expect(result.character.id).toBe('c1')
  })

  it('getV4ChapterMemory calls /chapter-memory with both params', async () => {
    getMock.mockResolvedValueOnce({
      data: { chapterIndex: 5, summary: 'test', charactersInChapter: [] },
    })
    const result = await api.getV4ChapterMemory({ bookUrl: 'b1', chapterIndex: 5 })
    expect(getMock).toHaveBeenCalledWith('/chapter-memory', {
      params: { bookUrl: 'b1', chapterIndex: 5 },
    })
    expect(result.chapterIndex).toBe(5)
  })

  it('getV4MemoryStatus calls /memory/status', async () => {
    getMock.mockResolvedValueOnce({
      data: { maxReadChapter: 10, maxProcessedChapter: 5, processing: false, lastError: null },
    })
    const result = await api.getV4MemoryStatus('b1')
    expect(getMock).toHaveBeenCalledWith('/memory/status', { params: { bookUrl: 'b1' } })
    expect(result.processing).toBe(false)
  })

  it('resetV4Memory calls POST /memory/reset', async () => {
    postMock.mockResolvedValueOnce({ data: { ok: true } })
    const result = await api.resetV4Memory('b1')
    expect(postMock).toHaveBeenCalledWith('/memory/reset', { bookUrl: 'b1' })
    expect(result.ok).toBe(true)
  })

  it('setV4Enabled calls POST /enabled', async () => {
    postMock.mockResolvedValueOnce({ data: { enabled: true } })
    const result = await api.setV4Enabled({ bookUrl: 'b1', enabled: true })
    expect(postMock).toHaveBeenCalledWith('/enabled', { bookUrl: 'b1', enabled: true })
    expect(result.enabled).toBe(true)
  })

  it('generateV4ChapterMemory calls POST /chapter-memory/generate', async () => {
    postMock.mockResolvedValueOnce({
      data: { ok: true, chapterIndex: 3, message: 'triggered' },
    })
    const result = await api.generateV4ChapterMemory({ bookUrl: 'b1', chapterIndex: 3 })
    expect(postMock).toHaveBeenCalledWith('/chapter-memory/generate', {
      bookUrl: 'b1',
      chapterIndex: 3,
    })
    expect(result.chapterIndex).toBe(3)
  })

  it('startV4Catchup calls POST /catchup/start', async () => {
    postMock.mockResolvedValueOnce({ data: { ok: true, targetChapter: 10 } })
    const result = await api.startV4Catchup({ bookUrl: 'b1', targetChapterIndex: 10 })
    expect(postMock).toHaveBeenCalledWith('/catchup/start', {
      bookUrl: 'b1',
      targetChapterIndex: 10,
    })
    expect(result.targetChapter).toBe(10)
  })

  it('getV4CatchupStatus calls GET /catchup/status', async () => {
    getMock.mockResolvedValueOnce({
      data: { status: 'running', targetChapter: 10, currentChapter: 3, maxProcessedChapter: 2 },
    })
    const result = await api.getV4CatchupStatus('b1')
    expect(getMock).toHaveBeenCalledWith('/catchup/status', { params: { bookUrl: 'b1' } })
    expect(result.status).toBe('running')
  })

  it('cancelV4Catchup calls POST /catchup/cancel', async () => {
    postMock.mockResolvedValueOnce({ data: { ok: true } })
    const result = await api.cancelV4Catchup('b1')
    expect(postMock).toHaveBeenCalledWith('/catchup/cancel', { bookUrl: 'b1' })
    expect(result.ok).toBe(true)
  })
})
