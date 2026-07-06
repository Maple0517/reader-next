import { describe, expect, it } from 'vitest'
import { readFrontendSource } from '../../tests/helpers/v4BoundaryGuard'

describe('router AI Book route', () => {
  it('routes /ai-book to AiBookV4View while preserving name and path', () => {
    const routerSource = readFrontendSource('src/router/index.ts')

    expect(routerSource).toContain("path: '/ai-book'")
    expect(routerSource).toContain("name: 'ai-book'")
    expect(routerSource).toContain("component: () => import('../views/AiBookV4View.vue')")
    expect(routerSource).not.toContain("component: () => import('../views/AiBookView.vue')")
  })
})
