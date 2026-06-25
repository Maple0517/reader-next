import { describe, expect, it } from 'vitest'
import source from '../src/views/AiBookView.vue?raw'

describe('AiBookView v3 removes legacy settings deep link', () => {
  it('no longer keeps server model settings query handling in the view', () => {
    expect(source).not.toContain("route.query.tab === 'settings'")
    expect(source).not.toContain("route.query.section === 'server-model'")
    expect(source).not.toContain('adminModelPanelRef')
  })
})
