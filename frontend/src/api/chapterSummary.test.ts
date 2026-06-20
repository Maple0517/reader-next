import { describe, expect, it, vi, beforeEach } from 'vitest'

const getMock = vi.fn()
const postMock = vi.fn()

vi.mock('./http', () => ({
  default: {
    get: getMock,
    post: postMock,
  },
}))

const api = await import('./chapterSummary')

describe('chapterSummary api', () => {
  beforeEach(() => {
    getMock.mockReset()
    postMock.mockReset()
  })

  it('reads cached summary with book and chapter urls', async () => {
    getMock.mockResolvedValueOnce({ data: { summary: null } })

    await expect(api.getChapterSummary('book a', 'chapter 1')).resolves.toEqual({ summary: null })

    expect(getMock).toHaveBeenCalledWith('/chapterSummary', {
      params: { bookUrl: 'book a', chapterUrl: 'chapter 1' },
    })
  })

  it('generates summary through backend endpoint', async () => {
    postMock.mockResolvedValueOnce({ data: { summary: { summary: 'ok' } } })

    await api.generateChapterSummary({
      bookUrl: 'book',
      chapterUrl: 'chapter',
      chapterIndex: 1,
      chapterTitle: '第一章',
      content: '正文内容',
      force: true,
      previousChapters: [
        { chapterUrl: 'chapter-0', chapterIndex: 0, chapterTitle: '序章' },
      ],
    })

    expect(postMock).toHaveBeenCalledWith('/chapterSummary/generate', {
      bookUrl: 'book',
      chapterUrl: 'chapter',
      chapterIndex: 1,
      chapterTitle: '第一章',
      content: '正文内容',
      force: true,
      previousChapters: [
        { chapterUrl: 'chapter-0', chapterIndex: 0, chapterTitle: '序章' },
      ],
    })
  })
})
