import { describe, expect, it } from 'vitest'
import { forbiddenV3AiBookBoundaryPatterns, readFrontendSource } from './helpers/v4BoundaryGuard'

describe('AI Book V4 boundary baseline', () => {
  it('documents that /ai-book now routes to the V4 shell while preserving route contract', () => {
    const routerSource = readFrontendSource('src/router/index.ts')

    expect(routerSource).toContain("path: '/ai-book'")
    expect(routerSource).toContain("name: 'ai-book'")
    expect(routerSource).toContain("component: () => import('../views/AiBookV4View.vue')")
    expect(routerSource).not.toContain("component: () => import('../views/AiBookView.vue')")
  })

  it('documents that the old AiBookView remains mixed legacy code after route cutover', () => {
    const sourceText = readFrontendSource('src/views/AiBookView.vue')

    expect(sourceText).toContain("from '../stores/aiBook'")
    expect(sourceText).toContain("from '../api/v4/book'")
    expect(sourceText).toContain('后端 V3 视图模型')
    expect(sourceText).toContain('来自章节 digest.characterStates')
    expect(sourceText).toContain('V3 角色')
    expect(sourceText).toContain('V4 角色')
    expect(sourceText).toContain('V3 关系')
    expect(sourceText).toContain('V4 关系')
  })

  it('defines the forbidden V3 boundary patterns for the future V4 shell guard', () => {
    expect(forbiddenV3AiBookBoundaryPatterns).toEqual([
      'src/stores/aiBook',
      '../stores/aiBook',
      '../../stores/aiBook',
      'src/api/ai/book',
      '../api/ai/book',
      '../../api/ai/book',
      'AiBookMemoryViewModel',
      'AiBookCharacterView',
      'AiBookRelationView',
      'AiBookKnowledgeFactView',
      'AiBookChapterDigestView',
    ])
  })
})
