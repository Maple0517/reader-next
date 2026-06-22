import type { AiBookCharacter, AiBookEvidence, AiBookLocation, AiBookRelationship } from '../types'

export function normalizeDisplayCharacters(characters: AiBookCharacter[]) {
  const byName = new Map<string, AiBookCharacter>()
  for (const character of characters) {
    if (!character.name || isLowImportance(character.importance)) continue
    const key = normalizeKey(character.name)
    const existing = byName.get(key)
    byName.set(key, existing ? mergeDisplayCharacter(existing, character) : character)
  }
  return [...byName.values()]
}

export function filterDisplayCharacters(characters: AiBookCharacter[], query: string) {
  const normalizedQuery = normalizeSearch(query)
  if (!normalizedQuery) return characters
  return characters.filter((character) => normalizeSearch([
    character.name,
    character.aliases?.join(' '),
    character.status,
    character.faction,
    character.location,
    character.description,
  ].filter(Boolean).join(' ')).includes(normalizedQuery))
}

export function normalizeDisplayRelationships(relationships: AiBookRelationship[]) {
  const byPair = new Map<string, AiBookRelationship>()
  for (const relationship of relationships) {
    if (
      !relationship.source
      || !relationship.target
      || !relationship.relation
      || normalizeKey(relationship.source) === normalizeKey(relationship.target)
      || isLowImportance(relationship.importance)
      || isLowValueRelationship(relationship)
    ) {
      continue
    }
    const key = relationshipKey(relationship.source, relationship.target, relationship.relation)
    const existing = byPair.get(key)
    byPair.set(key, existing ? mergeDisplayRelationship(existing, relationship) : relationship)
  }
  return [...byPair.values()]
}

export function normalizeDisplayLocations(locations: AiBookLocation[]) {
  const byName = new Map<string, AiBookLocation>()
  for (const location of locations) {
    if (!location.name || isLowImportance(location.importance)) continue
    const parentName = location.parentName && normalizeKey(location.parentName) !== normalizeKey(location.name)
      ? location.parentName
      : undefined
    const normalized = { ...location, parentName }
    const key = normalizeKey(location.name)
    const existing = byName.get(key)
    byName.set(key, existing ? mergeDisplayLocation(existing, normalized) : normalized)
  }
  return [...byName.values()]
}

function mergeDisplayCharacter(current: AiBookCharacter, next: AiBookCharacter): AiBookCharacter {
  return {
    ...current,
    aliases: uniqueStrings([...(current.aliases || []), ...(next.aliases || [])]),
    status: richerString(current.status, next.status),
    faction: current.faction || next.faction,
    location: current.location || next.location,
    description: richerString(current.description, next.description),
    lastSeenChapter: current.lastSeenChapter || next.lastSeenChapter,
    importance: preferImportance(current.importance, next.importance),
    evidence: mergeEvidence(current.evidence, next.evidence),
  }
}

function mergeDisplayRelationship(current: AiBookRelationship, next: AiBookRelationship): AiBookRelationship {
  return {
    ...current,
    status: richerString(current.status, next.status),
    description: richerString(current.description, next.description),
    importance: preferImportance(current.importance, next.importance),
    evidence: mergeEvidence(current.evidence, next.evidence),
  }
}

function mergeDisplayLocation(current: AiBookLocation, next: AiBookLocation): AiBookLocation {
  return {
    ...current,
    kind: current.kind || next.kind,
    parentName: current.parentName || next.parentName,
    description: richerString(current.description, next.description),
    status: richerString(current.status, next.status),
    relatedCharacters: uniqueStrings([...(current.relatedCharacters || []), ...(next.relatedCharacters || [])]),
    firstSeenChapter: current.firstSeenChapter || next.firstSeenChapter,
    importance: preferImportance(current.importance, next.importance),
    evidence: mergeEvidence(current.evidence, next.evidence),
  }
}

function mergeEvidence(current: AiBookEvidence[] | undefined, next: AiBookEvidence[] | undefined) {
  const seen = new Set<string>()
  const result: AiBookEvidence[] = []
  for (const item of [...(current || []), ...(next || [])]) {
    const key = `${item.chapterIndex}-${item.chapterTitle}-${item.note}-${item.quote || ''}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(item)
  }
  return result
}

function relationshipKey(source: string, target: string, relation: string) {
  return `${[normalizeKey(source), normalizeKey(target)].sort().join('::')}::${normalizeKey(relation)}`
}

function isLowValueRelationship(relationship: AiBookRelationship) {
  if (importanceRank(relationship.importance) >= 2) return false
  const relation = normalizeKey(relationship.relation)
  if (!['认识', '见过', '路过', '同村', '同校', '位于', '相关'].includes(relation)) return false
  return normalizeKey(relationship.description || relationship.status || '').length < 18
}

function normalizeSearch(value: string) {
  return value.trim().toLowerCase().replace(/\s+/g, '')
}

function normalizeKey(value: string | undefined) {
  return (value || '')
    .trim()
    .toLowerCase()
    .replace(/[·•・]/g, '.')
    .replace(/\s+/g, '')
}

function isLowImportance(value: string | undefined) {
  const key = normalizeKey(value)
  if (!key) return false
  return ['low', '低', '低重要性', '不重要', '路人', '背景', 'minor', 'background', 'oneoff', '一次性']
    .some((term) => key.includes(term))
}

function importanceRank(value: string | undefined) {
  const key = normalizeKey(value)
  if (key.includes('high') || key.includes('高')) return 3
  if (key.includes('medium') || key.includes('中')) return 2
  if (isLowImportance(value)) return 1
  return 0
}

function richerString(current: string | undefined, next: string | undefined) {
  if (!current) return next || ''
  if (!next) return current
  return next.length > current.length ? next : current
}

function preferImportance(current: string | undefined, next: string | undefined) {
  return importanceRank(next) > importanceRank(current) ? next : current || next
}

function uniqueStrings(values: string[]) {
  const seen = new Set<string>()
  const result: string[] = []
  for (const value of values) {
    const key = normalizeKey(value)
    if (!key || seen.has(key)) continue
    seen.add(key)
    result.push(value)
  }
  return result
}
