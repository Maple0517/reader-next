import type { AiBookCatchupStatus } from '../types'

export function formatCatchupChapter(index?: number | null): string {
  return typeof index === 'number' ? `第${index + 1}章` : '当前阅读章节'
}

export function describeCatchupProgress(status: AiBookCatchupStatus): string {
  const total = Math.max(status.totalChapters || 0, 0)
  const completed = Math.max(status.completedChapters || 0, 0)
  const target = typeof status.targetChapterIndex === 'number'
    ? formatCatchupChapter(status.targetChapterIndex)
    : '当前阅读章节'
  const parts = [total ? `本次 ${Math.min(completed, total)}/${total}` : '本次任务']
  if (status.processedChapterIndex != null) {
    parts.push(`累计到 ${formatCatchupChapter(status.processedChapterIndex)}`)
  }
  parts.push(`目标 ${target}`)
  return parts.join(' · ')
}

export function describeCatchupDetail(status: AiBookCatchupStatus): string {
  if (status.status === 'failed') {
    const failedAt = status.currentChapterIndex != null
      ? `失败在 ${formatCatchupChapter(status.currentChapterIndex)}${status.currentChapterTitle ? ` · ${status.currentChapterTitle}` : ''}`
      : '补齐任务失败'
    return status.error ? `${failedAt}：${status.error}` : failedAt
  }
  if (status.currentChapterIndex != null) {
    return `当前处理 ${formatCatchupChapter(status.currentChapterIndex)}${status.currentChapterTitle ? ` · ${status.currentChapterTitle}` : ''}`
  }
  if (status.processedChapterIndex != null) {
    return `最近完成 ${formatCatchupChapter(status.processedChapterIndex)}${status.processedChapterTitle ? ` · ${status.processedChapterTitle}` : ''}`
  }
  return '等待任务开始'
}
