import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect } from 'vitest'

const testsDir = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const frontendDir = resolve(testsDir, '..')

export const forbiddenV3AiBookBoundaryPatterns = [
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
]

export function readFrontendSource(relativePath: string) {
  return readFileSync(resolve(frontendDir, relativePath), 'utf8')
}

export function expectNoForbiddenV3AiBookBoundaryImports(relativePath: string) {
  const sourceText = readFrontendSource(relativePath)
  for (const forbiddenPattern of forbiddenV3AiBookBoundaryPatterns) {
    expect(sourceText, `${relativePath} must not contain ${forbiddenPattern}`).not.toContain(forbiddenPattern)
  }
}
