import { describe, expect, it } from 'vitest'
import { buildChapterSummaryIdentity, isCurrentChapterSummaryIdentity } from './chapterSummaryState'

describe('chapterSummaryState', () => {
  it('builds a stable identity from book chapter and index', () => {
    expect(buildChapterSummaryIdentity('book', 'chapter', 3)).toBe('book\nchapter\n3')
  })

  it('rejects stale chapter identities', () => {
    const current = buildChapterSummaryIdentity('book', 'chapter-2', 2)
    const stale = buildChapterSummaryIdentity('book', 'chapter-1', 1)

    expect(isCurrentChapterSummaryIdentity(current, stale)).toBe(false)
    expect(isCurrentChapterSummaryIdentity(current, current)).toBe(true)
  })
})
