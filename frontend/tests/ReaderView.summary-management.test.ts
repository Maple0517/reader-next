import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
const source = readFileSync(new URL('../src/views/ReaderView.vue', import.meta.url), 'utf8')
describe('ReaderView summary management UI', () => {
 it('owns the summary management tabs and prompt editor', () => {
  expect(source).toContain("'content'")
  expect(source).toContain("'settings'")
 expect(source).toContain('chapterSummaryConfigDraft')
 expect(source).toContain('saveChapterSummaryGenerationSettings')
  expect(source).toContain('Number.isFinite')
 expect(source).toContain('打开 AI 后端设置')
 })

 it('does not keep the old inline collapse interaction', () => {
  expect(source).not.toContain('chapterSummaryExpanded')
  expect(source).not.toContain("chapterSummaryExpanded ? '收起'")
  expect(source).not.toContain('@click="chapterSummaryExpanded = !chapterSummaryExpanded"')
 })

 it('keeps an entry point when side placement collapses', () => {
  expect(source).toContain('showCollapsedChapterSummary')
  expect(source).toContain('chapter-summary-collapsed-pill')
  expect(source).toContain('@click="expandCollapsedChapterSummary"')
 })

 it('keeps a restore entry after hiding summary on mobile or inline layouts', () => {
  expect(source).toContain('showHiddenChapterSummaryPill')
  expect(source).toContain('@click="restoreHiddenChapterSummary"')
  expect(source).toContain('重新显示')
 })

 it('hides summary through a helper that clears pending generation', () => {
  expect(source).toContain('function hideChapterSummary()')
  expect(source).toContain('clearChapterSummaryTimer()')
  expect(source).not.toContain('@click="showChapterSummary = false"')
  expect(source).toContain('if (!showChapterSummary.value) return')
 })
})
