import { describe, expect, it } from 'vitest'
import {
  filterDisplayCharacters,
  normalizeDisplayCharacters,
  normalizeDisplayLocations,
  normalizeDisplayRelationships,
} from './aiBookSelectors'

describe('aiBookSelectors', () => {
  it('dedupes characters and merges aliases, richer status, and evidence', () => {
    const characters = normalizeDisplayCharacters([
      {
        name: '林舟',
        aliases: ['阿舟'],
        status: '抵达北境',
        description: '主角。',
        importance: 'medium',
        evidence: [{ chapterIndex: 0, chapterTitle: '第一章', note: '首次登场' }],
      },
      {
        name: '林舟',
        aliases: ['林小舟'],
        status: '抵达北境学院并取得入学资格',
        faction: '北境学院',
        location: '北境学院',
        importance: 'high',
        evidence: [{ chapterIndex: 1, chapterTitle: '第二章', note: '入学' }],
      },
      {
        name: '路人甲',
        status: '路过',
        importance: 'low',
      },
    ])

    expect(characters).toHaveLength(1)
    expect(characters[0]).toMatchObject({
      name: '林舟',
      aliases: ['阿舟', '林小舟'],
      status: '抵达北境学院并取得入学资格',
      faction: '北境学院',
      location: '北境学院',
      importance: 'high',
    })
    expect(characters[0].evidence).toHaveLength(2)
    expect(filterDisplayCharacters(characters, '学院')).toHaveLength(1)
  })

  it('drops low-value relationships and dedupes the same pair', () => {
    const relationships = normalizeDisplayRelationships([
      { source: '林舟', target: '沈月', relation: '认识', description: '见过。', importance: 'low' },
      { source: '林舟', target: '沈月', relation: '盟友', description: '共同调查旧神。', importance: 'medium' },
      { source: '沈月', target: '林舟', relation: '盟友', description: '共同调查旧神并交换情报。', importance: 'high' },
    ])

    expect(relationships).toHaveLength(1)
    expect(relationships[0]).toMatchObject({
      source: '林舟',
      target: '沈月',
      relation: '盟友',
      description: '共同调查旧神并交换情报。',
      importance: 'high',
    })
  })

  it('normalizes locations and removes self parents', () => {
    const locations = normalizeDisplayLocations([
      { name: '北境学院', parentName: '北境学院', kind: '学校', description: '北境学校。', importance: 'high' },
      { name: '北境学院', kind: 'school', description: '培养术士的学院。', relatedCharacters: ['林舟'], importance: 'medium' },
      { name: '背景街道', kind: 'street', description: '路过。', importance: 'low' },
    ])

    expect(locations).toHaveLength(1)
    expect(locations[0]).toMatchObject({
      name: '北境学院',
      parentName: undefined,
      description: '培养术士的学院。',
      relatedCharacters: ['林舟'],
      importance: 'high',
    })
  })
})
