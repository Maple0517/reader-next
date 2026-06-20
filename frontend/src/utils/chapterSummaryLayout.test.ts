import { describe, expect, it } from 'vitest'
import { chooseChapterSummaryPlacement, clampChapterSummarySiderWidth, getChapterSummaryFontSize } from './chapterSummaryLayout'

describe('chooseChapterSummaryPlacement', () => {
  it('uses side panel in auto mode when the page has enough spare width', () => {
    expect(chooseChapterSummaryPlacement({ mode: 'auto', viewportWidth: 1440, pageWidth: 800, isMobile: false })).toBe('side')
  })

  it('falls back inline in auto mode when spare width is narrow', () => {
    expect(chooseChapterSummaryPlacement({ mode: 'auto', viewportWidth: 1100, pageWidth: 900, isMobile: false })).toBe('inline')
  })

  it('keeps forced side panel when the viewport can fit it', () => {
    expect(chooseChapterSummaryPlacement({ mode: 'side', viewportWidth: 1180, pageWidth: 800, isMobile: false })).toBe('side')
  })

  it('collapses forced side panel instead of squeezing narrow pages', () => {
    expect(chooseChapterSummaryPlacement({ mode: 'side', viewportWidth: 980, pageWidth: 850, isMobile: false })).toBe('collapsed')
  })

  it('keeps 340px spare width for the side panel', () => {
    expect(chooseChapterSummaryPlacement({ mode: 'side', viewportWidth: 1120, pageWidth: 800, isMobile: false })).toBe('collapsed')
  })
})


describe('chapter summary sider sizing', () => {
  it('clamps the draggable sider width', () => {
    expect(clampChapterSummarySiderWidth(200)).toBe(280)
    expect(clampChapterSummarySiderWidth(420)).toBe(420)
    expect(clampChapterSummarySiderWidth(800)).toBe(520)
  })

  it('uses an independent summary font size', () => {
    expect(getChapterSummaryFontSize(19)).toBe(19)
    expect(getChapterSummaryFontSize(8)).toBe(12)
    expect(getChapterSummaryFontSize(48)).toBe(36)
  })
})
