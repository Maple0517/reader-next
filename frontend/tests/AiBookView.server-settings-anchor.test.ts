import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
const source = readFileSync(new URL('../src/views/AiBookView.vue', import.meta.url), 'utf8')
describe('AiBookView server model deep link', () => {
 it('supports settings tab and server model section query params', () => {
  expect(source).toContain("route.query.tab === 'settings'")
  expect(source).toContain("route.query.section === 'server-model'")
  expect(source).toContain('adminModelPanelRef')
 })
})
