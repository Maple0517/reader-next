export type ChapterSummaryLayoutMode = 'auto' | 'side'
export type ChapterSummaryPlacement = 'inline' | 'side' | 'collapsed'

interface ChapterSummaryPlacementInput {
  mode: ChapterSummaryLayoutMode
  viewportWidth: number
  pageWidth: number
  isMobile: boolean
  siderWidth?: number
}

export const CHAPTER_SUMMARY_SIDER_MIN_WIDTH = 280
export const CHAPTER_SUMMARY_SIDER_MAX_WIDTH = 520
export const CHAPTER_SUMMARY_SIDER_DEFAULT_WIDTH = 360

const SIDE_PANEL_GAP = 48
const FORCED_SIDE_MIN_VIEWPORT = 1100

export function clampChapterSummarySiderWidth(width: number) {
  if (!Number.isFinite(width)) return CHAPTER_SUMMARY_SIDER_DEFAULT_WIDTH
  return Math.max(CHAPTER_SUMMARY_SIDER_MIN_WIDTH, Math.min(CHAPTER_SUMMARY_SIDER_MAX_WIDTH, Math.round(width)))
}

export function getChapterSummaryFontSize(fontSize: number) {
  if (!Number.isFinite(fontSize)) return 16
  return Math.round(Math.max(12, Math.min(36, fontSize)))
}

export function chooseChapterSummaryPlacement(input: ChapterSummaryPlacementInput): ChapterSummaryPlacement {
  if (input.isMobile) return 'inline'

  const siderWidth = clampChapterSummarySiderWidth(input.siderWidth || CHAPTER_SUMMARY_SIDER_DEFAULT_WIDTH)
  const spare = input.viewportWidth - input.pageWidth
  if (input.mode === 'side') {
    return input.viewportWidth >= FORCED_SIDE_MIN_VIEWPORT && spare >= siderWidth ? 'side' : 'collapsed'
  }

  return spare >= siderWidth + SIDE_PANEL_GAP ? 'side' : 'inline'
}
