export function buildChapterSummaryIdentity(bookUrl?: string, chapterUrl?: string, chapterIndex?: number) {
  return `${bookUrl || ''}\n${chapterUrl || ''}\n${typeof chapterIndex === 'number' ? chapterIndex : ''}`
}

export function isCurrentChapterSummaryIdentity(current: string, candidate: string) {
  return current === candidate && candidate.trim().length > 0
}
