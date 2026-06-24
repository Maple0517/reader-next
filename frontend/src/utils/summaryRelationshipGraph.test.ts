import { describe, expect, it } from 'vitest'
import type { AiBookMemoryViewModel } from '../types'
import { buildSummaryRelationshipGraph } from './summaryRelationshipGraph'

const memory = {
  bookUrl: 'book',
  enabled: true,
  updatedAt: 1,
  summary: { current: 'summary', recentChanges: [], openQuestions: [] },
  characters: [
    { id: 'hero', name: '张宇', aliases: [], importance: 'high', description: '主角', lastSeenChapterIndex: 10, evidence: [{ chapterIndex: 10, chapterTitle: '第十章', note: '出现' }] },
    { id: 'ally', name: '李青', aliases: [], importance: 'medium', description: '盟友', lastSeenChapterIndex: 10, evidence: [] },
    { id: 'enemy', name: '城主府', aliases: [], importance: 'medium', description: '压力来源', lastSeenChapterIndex: 9, evidence: [] },
    { id: 'mother', name: '张宇母亲', aliases: [], importance: 'low', description: '背景亲属', lastSeenChapterIndex: 1, evidence: [] },
    { id: 'stranger', name: '路人甲', aliases: [], importance: 'low', description: '路人', lastSeenChapterIndex: 10, evidence: [] },
  ],
  relationships: [
    {
      id: 'r1', sourceCharacterId: 'hero', targetCharacterId: 'ally', kind: 'alliance', label: '盟友', polarity: 'positive', strength: 'strong', status: 'developing', direction: 'grouped', summary: '共同调查遗迹', currentDynamics: ['互相信任上升'], facets: [], lastUpdatedChapterIndex: 10, evidence: [], history: [],
    },
    {
      id: 'r2', sourceCharacterId: 'hero', targetCharacterId: 'ally', kind: 'friendship', label: '信任', polarity: 'positive', strength: 'moderate', status: 'active', direction: 'grouped', summary: '私下交换线索', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 9, evidence: [], history: [],
    },
    {
      id: 'r3', sourceCharacterId: 'enemy', targetCharacterId: 'hero', kind: 'conflict', label: '压力', polarity: 'negative', strength: 'critical', status: 'active', direction: 'grouped', summary: '开始关注张宇', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 9, evidence: [], history: [],
    },
    {
      id: 'r4', sourceCharacterId: 'hero', targetCharacterId: 'mother', kind: 'family', label: '家族', polarity: 'positive', strength: 'weak', status: 'distant', direction: 'grouped', summary: '背景牵引', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 1, evidence: [], history: [],
    },
    {
      id: 'r5', sourceCharacterId: 'stranger', targetCharacterId: 'mother', kind: 'unknown', label: '路人关系', polarity: 'neutral', strength: 'weak', status: 'active', direction: 'grouped', summary: '不应进入主角图', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 10, evidence: [], history: [],
    },
  ],
  knowledgeFacts: [],
  locations: [],
  cleanup: { droppedFactsCount: 0, droppedByReason: {}, oldSchemaBackedUp: false },
} satisfies AiBookMemoryViewModel

describe('buildSummaryRelationshipGraph', () => {
  it('centers the most-connected protagonist and aggregates direct relationships', () => {
    const graph = buildSummaryRelationshipGraph({ memory, currentChapterIndex: 10, limit: 6 })

    expect(graph.protagonist?.name).toBe('张宇')
    expect(graph.nodes.map((node) => node.name)).toEqual(['张宇', '李青', '城主府', '张宇母亲'])
    expect(graph.links).toHaveLength(3)
    expect(graph.links[0]).toMatchObject({ targetId: 'ally', label: '盟友 / 信任', summary: '互相信任上升' })
    expect(graph.links[1]).toMatchObject({ targetId: 'enemy', label: '压力', summary: '开始关注张宇' })
    expect(graph.links.find((link) => link.targetId === 'mother')?.tone).toBe('family')
  })

  it('applies the limit to related nodes and links', () => {
    const graph = buildSummaryRelationshipGraph({ memory, currentChapterIndex: 10, limit: 2 })

    expect(graph.links).toHaveLength(2)
    expect(graph.nodes).toHaveLength(3)
    expect(graph.nodes.map((node) => node.id)).not.toContain('mother')
  })

  it('ignores unknown relationship endpoints when choosing protagonist', () => {
    const graph = buildSummaryRelationshipGraph({
      memory: {
        ...memory,
        relationships: [
          ...memory.relationships,
          {
            id: 'r6',
            sourceCharacterId: 'ghost',
            targetCharacterId: 'hero',
            kind: 'conflict',
            label: '幽灵关系',
            polarity: 'negative',
            strength: 'critical',
            status: 'active',
            direction: 'grouped',
            summary: '不在角色列表',
            currentDynamics: [],
            facets: [],
            lastUpdatedChapterIndex: 10,
            evidence: [],
            history: [],
          },
        ],
      },
      currentChapterIndex: 10,
    })

    expect(graph.protagonist?.id).toBe('hero')
    expect(graph.links.map((link) => link.targetId)).not.toContain('ghost')
  })

  it('returns an empty reason when memory has no usable relationships', () => {
    const graph = buildSummaryRelationshipGraph({
      memory: { ...memory, relationships: [] },
      currentChapterIndex: 10,
    })

    expect(graph.protagonist).toBeNull()
    expect(graph.emptyReason).toBe('人物关系不足，继续阅读后会补全。')
  })
})
