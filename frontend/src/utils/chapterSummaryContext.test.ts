import { describe, expect, it } from 'vitest'
import { buildChapterSummaryContext } from './chapterSummaryContext'
import type { AiBookChapterMemoryViewModel, AiBookMemoryViewModel } from '../types'

const memory = {
  bookUrl: 'book',
  enabled: true,
  updatedAt: 1,
  summary: {
    current: '全书摘要',
    recentChanges: ['主角加入宗门'],
    openQuestions: ['神秘玉佩未解'],
  },
  characters: [
    { id: 'c1', name: '林动', aliases: [], importance: 'high', description: '主角', lastSeenChapterIndex: 3, evidence: [] },
  ],
  relationships: [
    {
      id: 'r1',
      sourceCharacterId: 'c1',
      targetCharacterId: 'c2',
      kind: 'alliance',
      label: '临时盟友',
      polarity: 'positive',
      strength: 'moderate',
      status: 'active',
      direction: 'grouped',
      summary: '共同调查遗迹',
      currentDynamics: ['互相试探'],
      facets: [],
      evidence: [],
      history: [],
    },
  ],
  knowledgeFacts: [
    { id: 'f1', category: 'techMagic', title: '符师', content: '精神力修炼体系', confidence: 'high', importance: 'high', evidence: [] },
  ],
  locations: [
    { id: 'l1', name: '炎城', aliases: [], kind: 'city', scale: 'city', description: '当前主舞台', importance: 'medium', evidence: [] },
  ],
  cleanup: { droppedFactsCount: 0, droppedByReason: {}, oldSchemaBackedUp: false },
} satisfies AiBookMemoryViewModel

const chapter = {
  bookUrl: 'book',
  chapterIndex: 3,
  chapterTitle: '第三章',
  digest: {
    chapterIndex: 3,
    chapterTitle: '第三章',
    summary: '本章进入炎城',
    keyPoints: ['抵达炎城'],
    characters: [],
    characterStates: [{ name: '林动', status: '进入炎城', lastSeenChapterIndex: 3 }],
    characterRelations: [{ source: '林动', target: '城主府', kind: 'affiliation', polarity: 'neutral', strength: 'weak', status: 'developing', description: '产生交集' }],
    knowledgeFacts: [{ title: '符师', content: '精神力修炼体系', category: 'techMagic', confidence: 'high', importance: 'high' }],
    locations: [{ name: '炎城', kind: 'city', description: '本章舞台', relatedCharacters: ['林动'] }],
    locationEdges: [],
  },
  characters: [],
  relationships: [],
  knowledgeFacts: [],
  locations: [],
  generationStatus: 'ready',
} satisfies AiBookChapterMemoryViewModel

describe('buildChapterSummaryContext', () => {
  it('builds focused rows and grouped more sections', () => {
    const view = buildChapterSummaryContext({ memory, chapter, currentChapterIndex: 3 })

    expect(view.focusRows.map((row) => row.kind)).toEqual([
      'relation',
      'clue',
      'character',
      'fact',
      'location',
    ])
    expect(view.moreSections.map((section) => section.key)).toEqual([
      'characters',
      'relationships',
      'clues',
      'facts',
      'locations',
    ])
  })

  it('uses character names in relationship rows and keeps the protagonist before minor characters', () => {
    const view = buildChapterSummaryContext({
      memory: {
        ...memory,
        characters: [
          { id: 'hero', name: '张羽', aliases: [], importance: 'high', description: '主角', lastSeenChapterIndex: 8, evidence: [] },
          { id: 'mother', name: '张羽母亲', aliases: [], importance: 'low', description: '只短暂出现', lastSeenChapterIndex: 1, evidence: [] },
        ],
        relationships: [
          {
            id: 'r2',
            sourceCharacterId: 'hero',
            targetCharacterId: 'mother',
            kind: 'family',
            label: '母子',
            polarity: 'positive',
            strength: 'moderate',
            status: 'active',
            direction: 'grouped',
            summary: '家人关系',
            currentDynamics: [],
            facets: [],
            evidence: [],
            history: [],
          },
        ],
      },
      chapter: { ...chapter, digest: null },
      currentChapterIndex: 8,
      limit: 3,
    })

    expect(view.focusRows.find((row) => row.kind === 'relation')?.title).toBe('张羽 · 张羽母亲')
    expect(view.focusRows.find((row) => row.kind === 'character')?.title).toBe('张羽')
    expect(view.focusRows.some((row) => row.kind === 'character' && row.title === '张羽母亲')).toBe(false)
  })

})
