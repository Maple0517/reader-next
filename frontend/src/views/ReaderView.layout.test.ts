import { describe, expect, it } from 'vitest'
import source from './ReaderView.vue?raw'


function ruleBody(pattern: RegExp) {
  return source.match(pattern)?.groups?.body ?? ''
}

describe('ReaderView summary heading layout', () => {
  it('keeps long chapter titles from squeezing the summary tabs', () => {
    const leftColumn = ruleBody(/\.chapter-summary-header > :first-child,\s*\.chapter-summary-sider-head > :first-child\s*\{(?<body>[^}]*)\}/)
    const tabs = ruleBody(/\.summary-tabs\s*\{(?<body>[^}]*)\}/)

    expect(leftColumn).toContain('min-width: 0')
    expect(leftColumn).toContain('overflow: hidden')
    expect(tabs).toContain('flex: 0 0 auto')
  })
})
