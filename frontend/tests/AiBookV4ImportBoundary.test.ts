import { describe, it } from 'vitest'
import { expectNoForbiddenV3AiBookBoundaryImports } from './helpers/v4BoundaryGuard'

const guardedV4BoundaryFiles = [
  'src/views/AiBookV4View.vue',
  'src/components/reader/V4BookOverviewPanel.vue',
  'src/components/reader/V4CharacterPanel.vue',
  'src/components/reader/v4/V4PanelFrame.vue',
  'src/components/reader/v4/V4LoadingState.vue',
  'src/components/reader/v4/V4ErrorState.vue',
  'src/components/reader/v4/V4EmptyState.vue',
  'src/components/reader/v4/V4EvidenceList.vue',
  'src/composables/useV4AsyncState.ts',
]

describe('AI Book V4 import boundary', () => {
  it('keeps new V4 shell files free of V3 AI Book imports and types', () => {
    for (const relativePath of guardedV4BoundaryFiles) {
      expectNoForbiddenV3AiBookBoundaryImports(relativePath)
    }
  })
})
