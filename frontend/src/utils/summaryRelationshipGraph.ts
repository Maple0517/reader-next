import type {
  AiBookMemoryViewModel,
  AiBookRelationKind,
  AiBookRelationStrength,
  AiBookRelationView,
} from '../types'

export type SummaryRelationshipTone = 'family' | 'romance' | 'ally' | 'conflict' | 'affiliation' | 'neutral'

export interface SummaryRelationshipGraphNode {
  id: string
  name: string
  description: string
  isProtagonist: boolean
  x: number
  y: number
}

export interface SummaryRelationshipGraphLink {
  id: string
  sourceId: string
  targetId: string
  label: string
  summary: string
  tone: SummaryRelationshipTone
  strength: AiBookRelationStrength
  path: string
}

export interface SummaryRelationshipGraphView {
  protagonist: SummaryRelationshipGraphNode | null
  nodes: SummaryRelationshipGraphNode[]
  links: SummaryRelationshipGraphLink[]
  rows: Array<{ id: string; name: string; label: string; summary: string; tone: SummaryRelationshipTone }>
  emptyReason: string
}

export function buildSummaryRelationshipGraph(input: {
  memory: AiBookMemoryViewModel | null
  currentChapterIndex: number
  limit?: number
}): SummaryRelationshipGraphView {
  const memory = input.memory
  if (!memory || memory.relationships.length === 0 || memory.characters.length === 0) {
    return empty('人物关系不足，继续阅读后会补全。')
  }

  const characterById = new Map(memory.characters.map((item) => [item.id, item]))
  const protagonistId = findProtagonistId(memory, input.currentChapterIndex, characterById)
  if (!protagonistId) return empty('人物关系不足，继续阅读后会补全。')
  const protagonist = characterById.get(protagonistId)
  if (!protagonist) return empty('人物关系不足，继续阅读后会补全。')

  const grouped = new Map<string, AiBookRelationView[]>()
  for (const relation of memory.relationships) {
    const otherId = relation.sourceCharacterId === protagonistId
      ? relation.targetCharacterId
      : relation.targetCharacterId === protagonistId
        ? relation.sourceCharacterId
        : ''
    if (!otherId || !characterById.has(otherId)) continue
    grouped.set(otherId, [...(grouped.get(otherId) || []), relation])
  }

  const related = [...grouped.entries()]
    .map(([characterId, relations]) => ({ characterId, relations, score: relationshipScore(relations, input.currentChapterIndex) }))
    .sort((a, b) => b.score - a.score)
    .slice(0, input.limit ?? 6)

  if (related.length === 0) return empty('人物关系不足，继续阅读后会补全。')

  const center: SummaryRelationshipGraphNode = {
    id: protagonist.id,
    name: protagonist.name,
    description: protagonist.description || '主角',
    isProtagonist: true,
    x: 50,
    y: 50,
  }

  const outerNodes = related.map((item, index) => {
    const character = characterById.get(item.characterId)!
    const angle = -90 + (360 / related.length) * index
    const radius = 34
    const rad = angle * Math.PI / 180
    return {
      id: character.id,
      name: character.name,
      description: character.description || '',
      isProtagonist: false,
      x: Math.round((50 + Math.cos(rad) * radius) * 10) / 10,
      y: Math.round((50 + Math.sin(rad) * radius) * 10) / 10,
    }
  })

  const nodeById = new Map([center, ...outerNodes].map((node) => [node.id, node]))
  const links = related.map((item) => {
    const node = nodeById.get(item.characterId)!
    const label = aggregateLabel(item.relations)
    const summary = aggregateSummary(item.relations)
    return {
      id: item.characterId,
      sourceId: protagonistId,
      targetId: item.characterId,
      label,
      summary,
      tone: toneFor(item.relations),
      strength: strongest(item.relations.map((relation) => relation.strength)),
      path: `M 50 50 L ${node.x} ${node.y}`,
    }
  })

  return {
    protagonist: center,
    nodes: [center, ...outerNodes],
    links,
    rows: links.map((link) => ({
      id: link.id,
      name: nodeById.get(link.targetId)?.name || link.targetId,
      label: link.label,
      summary: link.summary,
      tone: link.tone,
    })),
    emptyReason: '',
  }
}

function empty(emptyReason: string): SummaryRelationshipGraphView {
  return { protagonist: null, nodes: [], links: [], rows: [], emptyReason }
}

function findProtagonistId(
  memory: AiBookMemoryViewModel,
  currentChapterIndex: number,
  characterById: Map<string, AiBookMemoryViewModel['characters'][number]>,
) {
  const scores = new Map<string, number>()
  for (const character of memory.characters) {
    scores.set(character.id, (character.importance === 'high' ? 4 : 0) + recencyScore(character.lastSeenChapterIndex, currentChapterIndex))
  }
  for (const relation of memory.relationships) {
    if (characterById.has(relation.sourceCharacterId)) {
      scores.set(relation.sourceCharacterId, (scores.get(relation.sourceCharacterId) || 0) + 10)
    }
    if (characterById.has(relation.targetCharacterId)) {
      scores.set(relation.targetCharacterId, (scores.get(relation.targetCharacterId) || 0) + 10)
    }
  }
  return [...scores.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] || ''
}

function relationshipScore(relations: AiBookRelationView[], currentChapterIndex: number) {
  return relations.reduce((score, relation) => score
    + strengthScore(relation.strength)
    + (relation.status === 'developing' ? 6 : 0)
    + recencyScore(relation.lastUpdatedChapterIndex, currentChapterIndex)
    + relation.currentDynamics.length
    + relation.history.length
    + relation.evidence.length, 0)
}

function recencyScore(index: number | null | undefined, currentChapterIndex: number) {
  if (index == null) return 0
  return Math.max(0, 8 - Math.abs(currentChapterIndex - index))
}

function strengthScore(strength: AiBookRelationStrength) {
  return { critical: 8, strong: 6, moderate: 4, weak: 2, unknown: 0 }[strength] || 0
}

function aggregateLabel(relations: AiBookRelationView[]) {
  return unique(relations.flatMap((relation) => [
    ...relation.facets.map((facet) => facet.subtype || labelForKind(facet.kind)),
    relation.label,
    relation.label ? '' : labelForKind(relation.kind),
  ])).slice(0, 3).join(' / ')
}

function aggregateSummary(relations: AiBookRelationView[]) {
  return relations.flatMap((relation) => [...relation.currentDynamics, relation.summary]).find(Boolean) || '关系仍在发展'
}

function toneFor(relations: AiBookRelationView[]): SummaryRelationshipTone {
  if (relations.some((relation) => relation.kind === 'family')) return 'family'
  if (relations.some((relation) => relation.kind === 'romance')) return 'romance'
  if (relations.some((relation) => relation.kind === 'conflict' || relation.kind === 'rivalry' || relation.polarity === 'negative')) return 'conflict'
  if (relations.some((relation) => relation.kind === 'alliance' || relation.kind === 'friendship' || relation.polarity === 'positive')) return 'ally'
  if (relations.some((relation) => relation.kind === 'affiliation' || relation.kind === 'supervision')) return 'affiliation'
  return 'neutral'
}

function strongest(strengths: AiBookRelationStrength[]) {
  return strengths.sort((a, b) => strengthScore(b) - strengthScore(a))[0] || 'unknown'
}

function labelForKind(kind: AiBookRelationKind) {
  return {
    family: '家族',
    romance: '情感',
    friendship: '友情',
    rivalry: '竞争',
    alliance: '盟友',
    conflict: '冲突',
    affiliation: '阵营',
    supervision: '师承',
    unknown: '关联',
  }[kind]
}

function unique(values: string[]) {
  return [...new Set(values.filter(Boolean))]
}
