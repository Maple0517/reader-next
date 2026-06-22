import type {
  AiBookAnyMemory,
  AiBookChapterDigest,
  AiBookChapterKnowledgePatch,
  AiBookCharacter,
  AiBookCharacterV2,
  AiBookEvidence,
  AiBookImportance,
  AiBookLocation,
  AiBookLocationScale,
  AiBookLocationV2,
  AiBookMap,
  AiBookMapEdge,
  AiBookMapNode,
  AiBookMemory,
  AiBookMemoryV2,
  AiBookRelationship,
  AiBookRelationshipV2,
  AiBookWorldFact,
  Book,
  BookChapter,
} from '../types'

const MAX_EVIDENCE_PER_ENTITY = 8
const MAX_RECENT_CHANGES = 10
const MAX_OPEN_QUESTIONS = 20

export function isAiBookMemoryV2(memory: AiBookAnyMemory | null | undefined): memory is AiBookMemoryV2 {
  return Boolean(memory && 'schemaVersion' in memory && memory.schemaVersion === 2)
}

export function createEmptyAiBookMemoryV2(book: Book): AiBookMemoryV2 {
  return {
    schemaVersion: 2,
    bookUrl: book.bookUrl,
    bookName: book.name,
    author: book.author,
    enabled: false,
    processedChapterIndex: undefined,
    processedChapterTitle: undefined,
    updatedAt: Date.now(),
    lastError: undefined,
    summary: {
      current: '',
      recentChanges: [],
      openQuestions: [],
    },
    chapterDigests: [],
    arcs: [],
    worldFacts: [],
    characters: [],
    relationships: [],
    locations: [],
    mapState: {
      dirty: false,
      nodes: [],
      edges: [],
    },
    renderArtifacts: {},
  }
}

export function toAiBookDisplayMemory(memory: AiBookAnyMemory): AiBookMemory {
  if (!isAiBookMemoryV2(memory)) return memory

  const charactersById = new Map(memory.characters.map((character) => [character.id, character]))
  const locationsById = new Map(memory.locations.map((location) => [location.id, location]))

  return {
    bookUrl: memory.bookUrl,
    bookName: memory.bookName,
    author: memory.author,
    enabled: memory.enabled,
    processedChapterIndex: memory.processedChapterIndex,
    processedChapterTitle: memory.processedChapterTitle,
    updatedAt: memory.updatedAt,
    summary: memory.summary.current,
    worldview: memory.worldFacts.map((fact) => ({
      title: fact.title,
      content: fact.content,
      category: fact.category,
      confidence: fact.confidence,
      importance: fact.importance,
      evidence: fact.evidence,
    })),
    characters: memory.characters.map((character): AiBookCharacter => ({
      name: character.name,
      aliases: character.aliases,
      status: character.currentStatus,
      faction: character.faction,
      location: character.currentLocationId ? locationsById.get(character.currentLocationId)?.name : undefined,
      description: character.description,
      lastSeenChapter: formatChapterLabel(character.lastSeenChapterIndex),
      importance: character.importance,
      evidence: character.evidence,
    })),
    relationships: memory.relationships.map((relationship): AiBookRelationship => {
      const source = charactersById.get(relationship.sourceCharacterId)
      const targetCharacter = relationship.targetKind === 'character'
        ? charactersById.get(relationship.targetEntityId)
        : undefined
      const targetLocation = relationship.targetKind === 'location'
        ? locationsById.get(relationship.targetEntityId)
        : undefined
      return {
        source: source?.name || relationship.sourceCharacterId,
        target: targetCharacter?.name || targetLocation?.name || relationship.targetEntityId,
        relation: relationship.relationType,
        status: relationship.currentStatus,
        description: relationship.description,
        importance: relationship.importance,
        evidence: relationship.evidence,
      }
    }),
    locations: memory.locations.map((location): AiBookLocation => ({
      name: location.name,
      kind: location.kind,
      parentName: location.parentId ? locationsById.get(location.parentId)?.name : undefined,
      description: location.description,
      status: location.currentStatus,
      relatedCharacters: location.relatedCharacterIds
        .map((id) => charactersById.get(id)?.name || id)
        .filter(Boolean),
      firstSeenChapter: formatChapterLabel(location.firstSeenChapterIndex),
      importance: location.importance,
      evidence: location.evidence,
    })),
    map: memory.renderArtifacts.mapImageUrl || memory.renderArtifacts.mapImagePrompt || memory.renderArtifacts.mapFallbackReason
      ? {
          imageUrl: memory.renderArtifacts.mapImageUrl,
          prompt: memory.renderArtifacts.mapImagePrompt || memory.mapState.mapPrompt,
          updatedAt: memory.mapState.lastRenderedAt,
          sourceChapterIndex: memory.mapState.sourceChapterIndex,
          fallback: memory.renderArtifacts.mapFallbackReason && !memory.renderArtifacts.mapImageUrl
            ? 'relationship-graph'
            : undefined,
          fallbackReason: memory.renderArtifacts.mapFallbackReason,
        }
      : null,
    mapDirty: memory.mapState.dirty,
    lastError: memory.lastError,
  }
}

export function reconcileAiBookMemoryV2(
  previous: AiBookMemoryV2,
  patch: AiBookChapterKnowledgePatch,
  book: Book,
  chapter: BookChapter,
) {
  const next: AiBookMemoryV2 = cloneMemory(previous)
  next.bookUrl = book.bookUrl
  next.bookName = book.name
  next.author = book.author
  next.processedChapterIndex = chapter.index
  next.processedChapterTitle = chapter.title
  next.updatedAt = Date.now()
  next.lastError = undefined

  next.summary.current = readNonEmpty(patch.summary?.current) || previous.summary.current || patch.chapterDigest.digest || ''
  next.summary.recentChanges = uniqueStrings([
    ...(patch.summary?.recentChanges || []),
    ...(patch.chapterDigest.keyEvents || []),
    ...previous.summary.recentChanges,
  ]).slice(0, MAX_RECENT_CHANGES)
  next.summary.openQuestions = uniqueStrings([
    ...previous.summary.openQuestions,
    ...(patch.summary?.openQuestions || []),
  ]).slice(0, MAX_OPEN_QUESTIONS)

  const touchedEntityIds = new Set<string>()
  mergeWorldFacts(next, patch, chapter, touchedEntityIds)
  const locationNameToId = mergeLocations(next, patch, chapter, touchedEntityIds)
  mergeCharacters(next, patch, chapter, locationNameToId, touchedEntityIds)
  mergeRelationships(next, patch, touchedEntityIds)

  const digest = normalizeDigest(patch.chapterDigest, chapter, [...touchedEntityIds])
  upsertByKey(next.chapterDigests, digest, (item) => String(item.chapterIndex), mergeChapterDigest)

  const structuralLocationChange = hasStructuralLocationChange(previous, next)
  const requestedMapChange = Boolean(patch.mapChanges?.changed)
  const shouldRegenerateMap = requestedMapChange && structuralLocationChange
  next.mapState = buildMapState(next, {
    dirty: shouldRegenerateMap || previous.mapState.dirty,
    reason: shouldRegenerateMap
      ? patch.mapChanges?.reason || '地点结构发生变化'
      : previous.mapState.reason,
    sourceChapterIndex: shouldRegenerateMap ? chapter.index : previous.mapState.sourceChapterIndex,
    mapPrompt: shouldRegenerateMap
      ? buildMapPrompt(next, patch.mapChanges?.reason)
      : previous.mapState.mapPrompt,
  })

  return {
    memory: next,
    shouldRegenerateMap,
    mapPrompt: shouldRegenerateMap ? next.mapState.mapPrompt : undefined,
  }
}

export function applyMapArtifactToMemoryV2(memory: AiBookMemoryV2, map: AiBookMap): AiBookMemoryV2 {
  return {
    ...memory,
    updatedAt: Date.now(),
    mapState: {
      ...memory.mapState,
      dirty: false,
      lastRenderedAt: map.updatedAt || Date.now(),
      sourceChapterIndex: map.sourceChapterIndex,
      mapPrompt: map.prompt || memory.mapState.mapPrompt,
    },
    renderArtifacts: {
      ...memory.renderArtifacts,
      mapImageUrl: map.imageUrl,
      mapImagePrompt: map.prompt || memory.renderArtifacts.mapImagePrompt,
      mapFallbackReason: undefined,
    },
  }
}

export function applyMapFallbackToMemoryV2(
  memory: AiBookMemoryV2,
  prompt: string,
  reason: string,
  sourceChapterIndex?: number,
): AiBookMemoryV2 {
  return {
    ...memory,
    updatedAt: Date.now(),
    mapState: {
      ...memory.mapState,
      dirty: true,
      reason,
      sourceChapterIndex,
      mapPrompt: prompt,
    },
    renderArtifacts: {
      ...memory.renderArtifacts,
      mapImageUrl: undefined,
      mapImagePrompt: prompt,
      mapFallbackReason: reason,
    },
  }
}

function mergeWorldFacts(
  memory: AiBookMemoryV2,
  patch: AiBookChapterKnowledgePatch,
  chapter: BookChapter,
  touchedEntityIds: Set<string>,
) {
  for (const item of [...(patch.worldFacts || []), ...(patch.facts || [])]) {
    const title = readNonEmpty(item.title)
    const content = readNonEmpty(item.content)
    if (!title || !content || item.importance === 'low') continue
    const id = item.id || stableId('fact', `${item.category || '基础设定'}-${title}`)
    const next: AiBookWorldFact = {
      id,
      category: item.category || '基础设定',
      title,
      content,
      confidence: item.confidence || '推断',
      importance: item.importance || 'medium',
      firstSeenChapterIndex: chapter.index,
      lastConfirmedChapterIndex: chapter.index,
      evidence: capEvidence(item.evidence || []),
    }
    upsertByKey(memory.worldFacts, next, (fact) => fact.id, mergeWorldFact)
    touchedEntityIds.add(id)
  }
}

function mergeCharacters(
  memory: AiBookMemoryV2,
  patch: AiBookChapterKnowledgePatch,
  chapter: BookChapter,
  locationNameToId: Map<string, string>,
  touchedEntityIds: Set<string>,
) {
  for (const item of patch.characters || []) {
    const name = readNonEmpty(item.name)
    if (!name || item.importance === 'low') continue
    const id = item.id || findCharacterId(memory, name, item.aliases || []) || stableId('char', name)
    const currentLocationId = item.locationName
      ? locationNameToId.get(normalizeKey(item.locationName))
      : undefined
    const status = readNonEmpty(item.currentStatus) || readNonEmpty(item.status) || '状态未知'
    const evidence = capEvidence(item.evidence || [])
    const next: AiBookCharacterV2 = {
      id,
      name,
      aliases: uniqueStrings(item.aliases || []),
      importance: item.importance || 'medium',
      currentStatus: status,
      faction: readNonEmpty(item.faction) || undefined,
      currentLocationId,
      description: readNonEmpty(item.description) || undefined,
      firstSeenChapterIndex: chapter.index,
      lastSeenChapterIndex: chapter.index,
      statusHistory: [{
        chapterIndex: chapter.index,
        chapterTitle: chapter.title,
        status,
        locationId: currentLocationId,
        faction: readNonEmpty(item.faction) || undefined,
        evidence: evidence[0],
      }],
      evidence,
    }
    upsertByKey(memory.characters, next, (character) => character.id, mergeCharacter)
    touchedEntityIds.add(id)
  }
}

function mergeRelationships(
  memory: AiBookMemoryV2,
  patch: AiBookChapterKnowledgePatch,
  touchedEntityIds: Set<string>,
) {
  for (const item of patch.relationships || []) {
    if (item.importance === 'low') continue
    const sourceCharacterId = item.sourceId || findCharacterId(memory, item.sourceName || '', []) || ''
    if (!sourceCharacterId) continue
    const targetKind = item.targetKind || 'character'
    const targetEntityId = item.targetId
      || findTargetEntityId(memory, item.targetName || '', targetKind)
      || item.targetName
      || ''
    const relationType = readNonEmpty(item.relationType) || readNonEmpty(item.relation) || ''
    if (!targetEntityId || !relationType || sourceCharacterId === targetEntityId) continue
    const direction = item.direction || 'undirected'
    const id = item.id || relationshipId(sourceCharacterId, targetKind, targetEntityId, relationType, direction)
    const next: AiBookRelationshipV2 = {
      id,
      sourceCharacterId,
      targetEntityId,
      targetKind,
      relationType,
      direction,
      currentStatus: readNonEmpty(item.currentStatus) || readNonEmpty(item.status) || undefined,
      description: readNonEmpty(item.description) || undefined,
      importance: item.importance || 'medium',
      evidence: capEvidence(item.evidence || []),
    }
    upsertByKey(memory.relationships, next, (relationship) => relationship.id, mergeRelationship)
    touchedEntityIds.add(id)
  }
}

function mergeLocations(
  memory: AiBookMemoryV2,
  patch: AiBookChapterKnowledgePatch,
  chapter: BookChapter,
  touchedEntityIds: Set<string>,
) {
  const locationNameToId = new Map(memory.locations.map((location) => [normalizeKey(location.name), location.id]))
  for (const item of patch.locations || []) {
    const name = readNonEmpty(item.name)
    if (!name || item.importance === 'low') continue
    const id = item.id || locationNameToId.get(normalizeKey(name)) || stableId('loc', name)
    locationNameToId.set(normalizeKey(name), id)
    const next: AiBookLocationV2 = {
      id,
      name,
      aliases: uniqueStrings(item.aliases || []),
      importance: item.importance || 'medium',
      kind: readNonEmpty(item.kind) || '未知地点',
      scale: item.scale || inferScale(item.kind),
      parentId: item.parentId,
      description: readNonEmpty(item.description) || readNonEmpty(item.currentStatus) || readNonEmpty(item.status) || '',
      currentStatus: readNonEmpty(item.currentStatus) || readNonEmpty(item.status) || undefined,
      relatedCharacterIds: uniqueStrings(item.relatedCharacterIds || []),
      firstSeenChapterIndex: chapter.index,
      lastSeenChapterIndex: chapter.index,
      evidence: capEvidence(item.evidence || []),
    }
    upsertByKey(memory.locations, next, (location) => location.id, mergeLocation)
    touchedEntityIds.add(id)
  }

  const byId = new Map(memory.locations.map((location) => [location.id, location]))
  for (const item of patch.locations || []) {
    const childId = item.id || locationNameToId.get(normalizeKey(item.name))
    if (!childId) continue
    const child = byId.get(childId)
    if (!child) continue
    const parentId = item.parentId || (item.parentName ? locationNameToId.get(normalizeKey(item.parentName)) : undefined)
    if (!parentId || parentId === child.id) continue
    const parent = byId.get(parentId)
    if (!parent) continue
    if (isValidParent(parent, child)) {
      child.parentId = parent.id
    } else {
      child.parentId = undefined
    }
  }

  return new Map(memory.locations.map((location) => [normalizeKey(location.name), location.id]))
}

function normalizeDigest(
  digest: AiBookChapterKnowledgePatch['chapterDigest'],
  chapter: BookChapter,
  touchedEntityIds: string[],
): AiBookChapterDigest {
  return {
    chapterIndex: digest.chapterIndex ?? chapter.index,
    chapterTitle: digest.chapterTitle || chapter.title,
    digest: digest.digest || '',
    keyEvents: uniqueStrings(digest.keyEvents || []),
    touchedEntityIds: uniqueStrings([...(digest.touchedEntityIds || []), ...touchedEntityIds]),
    createdAt: digest.createdAt || Date.now(),
  }
}

function buildMapState(
  memory: AiBookMemoryV2,
  overrides: Pick<AiBookMemoryV2['mapState'], 'dirty'> & Partial<AiBookMemoryV2['mapState']>,
) {
  const nodes: AiBookMapNode[] = memory.locations.map((location) => ({
    id: `map-${location.id}`,
    locationId: location.id,
    label: location.name,
    scale: location.scale,
    parentNodeId: location.parentId ? `map-${location.parentId}` : undefined,
    status: location.currentStatus,
  }))
  const edges: AiBookMapEdge[] = memory.locations.flatMap((location) => {
    if (!location.parentId) return []
    return [{
      id: `map-edge-${location.parentId}-${location.id}`,
      sourceNodeId: `map-${location.parentId}`,
      targetNodeId: `map-${location.id}`,
      kind: 'contains' as const,
      label: '包含',
    }]
  })
  return {
    ...memory.mapState,
    ...overrides,
    nodes,
    edges,
  }
}

function buildMapPrompt(memory: AiBookMemoryV2, reason?: string) {
  const locations = memory.locations
    .filter((location) => locationImportanceRank(locationImportance(location)) >= 2)
    .map((location) => {
      const parent = location.parentId
        ? memory.locations.find((item) => item.id === location.parentId)?.name
        : ''
      return `${parent ? `${parent} > ` : ''}${location.name}（${location.kind}）: ${location.description}`
    })
    .join('\n')
  return [
    `为小说《${memory.bookName || '未知书籍'}》绘制已读范围的俯视二维地图。`,
    reason ? `地图变化原因：${reason}` : '',
    '强调区域边界、地点层级、路线连接、图例、地图符号和中文地点标签。',
    '不要画人物插画、建筑照片、室内渲染或写实场景。',
    locations || memory.summary.current,
  ].filter(Boolean).join('\n')
}

function hasStructuralLocationChange(previous: AiBookMemoryV2, next: AiBookMemoryV2) {
  return locationSignature(previous.locations) !== locationSignature(next.locations)
}

function locationSignature(locations: AiBookLocationV2[]) {
  return locations
    .filter((location) => locationImportanceRank(locationImportance(location)) >= 2)
    .map((location) => [
      location.id,
      location.scale,
      location.parentId || '',
      location.kind,
      location.name,
    ].join(':'))
    .sort()
    .join('|')
}

function mergeWorldFact(current: AiBookWorldFact, next: AiBookWorldFact): AiBookWorldFact {
  return {
    ...current,
    category: next.category || current.category,
    content: richerString(current.content, next.content),
    confidence: confidenceRank(next.confidence) > confidenceRank(current.confidence) ? next.confidence : current.confidence,
    importance: preferImportance(current.importance, next.importance),
    lastConfirmedChapterIndex: Math.max(current.lastConfirmedChapterIndex || 0, next.lastConfirmedChapterIndex || 0) || undefined,
    evidence: capEvidence([...(current.evidence || []), ...(next.evidence || [])]),
  }
}

function mergeCharacter(current: AiBookCharacterV2, next: AiBookCharacterV2): AiBookCharacterV2 {
  return {
    ...current,
    aliases: uniqueStrings([...(current.aliases || []), ...(next.aliases || [])]),
    importance: preferImportance(current.importance, next.importance),
    currentStatus: next.currentStatus || current.currentStatus,
    faction: next.faction || current.faction,
    currentLocationId: next.currentLocationId || current.currentLocationId,
    description: richerString(current.description, next.description),
    firstSeenChapterIndex: current.firstSeenChapterIndex ?? next.firstSeenChapterIndex,
    lastSeenChapterIndex: Math.max(current.lastSeenChapterIndex || 0, next.lastSeenChapterIndex || 0) || undefined,
    statusHistory: capStateHistory([...(current.statusHistory || []), ...(next.statusHistory || [])]),
    evidence: capEvidence([...(current.evidence || []), ...(next.evidence || [])]),
  }
}

function mergeRelationship(current: AiBookRelationshipV2, next: AiBookRelationshipV2): AiBookRelationshipV2 {
  return {
    ...current,
    currentStatus: next.currentStatus || current.currentStatus,
    description: richerString(current.description, next.description),
    importance: preferImportance(current.importance, next.importance),
    lastSeenChapterIndex: Math.max(current.lastSeenChapterIndex || 0, next.lastSeenChapterIndex || 0) || undefined,
    evidence: capEvidence([...(current.evidence || []), ...(next.evidence || [])]),
  }
}

function mergeLocation(current: AiBookLocationV2, next: AiBookLocationV2): AiBookLocationV2 {
  return {
    ...current,
    aliases: uniqueStrings([...(current.aliases || []), ...(next.aliases || [])]),
    importance: preferImportance(current.importance, next.importance),
    kind: next.kind || current.kind,
    scale: next.scale || current.scale,
    parentId: next.parentId || current.parentId,
    description: richerString(current.description, next.description),
    currentStatus: next.currentStatus || current.currentStatus,
    relatedCharacterIds: uniqueStrings([...(current.relatedCharacterIds || []), ...(next.relatedCharacterIds || [])]),
    firstSeenChapterIndex: current.firstSeenChapterIndex ?? next.firstSeenChapterIndex,
    lastSeenChapterIndex: Math.max(current.lastSeenChapterIndex || 0, next.lastSeenChapterIndex || 0) || undefined,
    evidence: capEvidence([...(current.evidence || []), ...(next.evidence || [])]),
  }
}

function mergeChapterDigest(current: AiBookChapterDigest, next: AiBookChapterDigest): AiBookChapterDigest {
  return {
    ...current,
    digest: next.digest || current.digest,
    keyEvents: uniqueStrings([...(current.keyEvents || []), ...(next.keyEvents || [])]),
    touchedEntityIds: uniqueStrings([...(current.touchedEntityIds || []), ...(next.touchedEntityIds || [])]),
    createdAt: current.createdAt || next.createdAt,
  }
}

function upsertByKey<T>(items: T[], next: T, keyFn: (item: T) => string, merge: (current: T, next: T) => T) {
  const key = keyFn(next)
  const index = items.findIndex((item) => keyFn(item) === key)
  if (index >= 0) {
    items[index] = merge(items[index], next)
  } else {
    items.push(next)
  }
}

function findCharacterId(memory: AiBookMemoryV2, name: string, aliases: string[]) {
  const names = [name, ...aliases].map(normalizeKey).filter(Boolean)
  return memory.characters.find((character) => {
    const characterNames = [character.name, ...(character.aliases || [])].map(normalizeKey)
    return names.some((candidate) => characterNames.includes(candidate))
  })?.id
}

function findTargetEntityId(
  memory: AiBookMemoryV2,
  name: string,
  targetKind: AiBookRelationshipV2['targetKind'],
) {
  const key = normalizeKey(name)
  if (!key) return undefined
  if (targetKind === 'location') {
    return memory.locations.find((location) => {
      return [location.name, ...(location.aliases || [])].map(normalizeKey).includes(key)
    })?.id
  }
  if (targetKind === 'character') {
    return findCharacterId(memory, name, [])
  }
  return stableId('org', name)
}

function relationshipId(
  sourceCharacterId: string,
  targetKind: AiBookRelationshipV2['targetKind'],
  targetEntityId: string,
  relationType: string,
  direction: AiBookRelationshipV2['direction'],
) {
  if (direction === 'undirected') {
    const pair = [sourceCharacterId, targetEntityId].sort().join('-')
    return stableId('rel', `${pair}-${targetKind}-${relationType}-undirected`)
  }
  return stableId('rel', `${sourceCharacterId}-${targetKind}-${targetEntityId}-${relationType}-directed`)
}

function stableId(prefix: string, value: string) {
  const normalized = normalizeKey(value)
  return `${prefix}-${normalized || 'unknown'}`
}

function inferScale(kind: string | undefined): AiBookLocationScale {
  const key = normalizeKey(kind)
  if (!key) return 'unknown'
  if (['unknown', '未知', '不明', '无法确认', 'unclear'].some((item) => key.includes(item))) return 'unknown'
  if (key.includes('world') || key.includes('世界')) return 'world'
  if (key.includes('continent') || key.includes('大陆') || key.includes('洲')) return 'continent'
  if (['country', 'nation', 'kingdom', 'empire'].some((item) => key.includes(item))
    || key.includes('国家')
    || key.includes('王国')
    || key.includes('帝国')) return 'country'
  if (['region', 'province', 'state', 'county', 'territory'].some((item) => key.includes(item))
    || key.includes('区域')
    || key.includes('地区')
    || key.includes('省')
    || key.includes('州')
    || key.includes('郡')) return 'region'
  if (['district', 'neighborhood', 'quarter', 'village', 'street'].some((item) => key.includes(item))
    || key.includes('街区')
    || key.includes('城区')
    || key.includes('社区')
    || key.includes('村')) return 'district'
  if (['city', 'town'].some((item) => key.includes(item))
    || key.includes('城市')
    || key.includes('城镇')
    || key.endsWith('市')
    || key.endsWith('城')) return 'city'
  if (['room', 'classroom', 'office', 'bedroom', 'study', 'lab', 'laboratory'].some((item) => key.includes(item))
    || key.includes('房间')
    || key.includes('教室')
    || key.includes('办公室')
    || key.includes('卧室')
    || key.includes('书房')
    || key.includes('实验室')) return 'room'
  if (['building', 'school', 'academy', 'college', 'university', 'house', 'apartment', 'home'].some((item) => key.includes(item))
    || key.includes('学校')
    || key.includes('学院')
    || key.includes('大学')
    || key.includes('住宅')
    || key.includes('公寓')
    || key.includes('建筑')) return 'building'
  if (['site', 'facility', 'grounds', 'campus', 'yard'].some((item) => key.includes(item))
    || key.includes('地点')
    || key.includes('遗址')
    || key.includes('设施')
    || key.includes('场地')) return 'site'
  if (key.includes('世界')) return 'world'
  if (key.includes('大陆') || key.includes('洲')) return 'continent'
  if (key.includes('国家') || key.includes('王国') || key.includes('帝国')) return 'country'
  if (key.includes('区域') || key.includes('地区') || key.includes('省') || key.includes('州') || key.includes('郡')) return 'region'
  if (key.includes('城市') || key.includes('城镇') || key.endsWith('市') || key.endsWith('城')) return 'city'
  if (key.includes('街区') || key.includes('城区') || key.includes('社区')) return 'district'
  if (key.includes('学校') || key.includes('学院') || key.includes('住宅') || key.includes('建筑') || key.includes('设施')) return 'building'
  if (key.includes('房间') || key.includes('教室') || key.includes('书房')) return 'room'
  return 'unknown'
}

function isValidParent(parent: AiBookLocationV2, child: AiBookLocationV2) {
  if (scaleRank(parent.scale) > scaleRank(child.scale)) return true
  return isSchoolLike(parent.kind) && isFacilityLike(child.kind)
}

function scaleRank(scale: AiBookLocationScale) {
  const ranks: Record<AiBookLocationScale, number> = {
    world: 90,
    continent: 80,
    country: 70,
    region: 60,
    city: 50,
    district: 40,
    site: 30,
    building: 20,
    room: 10,
    unknown: 0,
  }
  return ranks[scale] || 0
}

function isSchoolLike(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['school', 'academy', 'college', 'university', '学校', '学院', '大学'].some((item) => key.includes(item))
}

function isFacilityLike(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['facility', 'site', 'grounds', 'campus', 'yard', '设施', '场地', '训练场', '遗址'].some((item) => key.includes(item))
}

function locationImportance(location: AiBookLocationV2): AiBookImportance {
  return location.importance
}

function locationImportanceRank(value: AiBookImportance) {
  if (value === 'high') return 3
  if (value === 'medium') return 2
  return 1
}

function preferImportance(current: AiBookImportance, next: AiBookImportance): AiBookImportance {
  return importanceRank(next) > importanceRank(current) ? next : current
}

function importanceRank(value: AiBookImportance) {
  if (value === 'high') return 3
  if (value === 'medium') return 2
  return 1
}

function confidenceRank(value: string) {
  if (value === '已知') return 3
  if (value === '推断') return 2
  return 1
}

function capEvidence(evidence: AiBookEvidence[]) {
  const seen = new Set<string>()
  const result: AiBookEvidence[] = []
  for (const item of evidence) {
    const key = `${item.chapterIndex}:${item.note}:${item.quote || ''}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(item)
  }
  return result.slice(-MAX_EVIDENCE_PER_ENTITY)
}

function capStateHistory(history: AiBookCharacterV2['statusHistory']) {
  return history.slice(-MAX_EVIDENCE_PER_ENTITY)
}

function uniqueStrings(values: string[]) {
  const seen = new Set<string>()
  const result: string[] = []
  for (const value of values) {
    const normalized = normalizeKey(value)
    if (!normalized || seen.has(normalized)) continue
    seen.add(normalized)
    result.push(value)
  }
  return result
}

function richerString(current: string | undefined, next: string | undefined) {
  if (!current) return next || ''
  if (!next) return current
  return next.length > current.length ? next : current
}

function readNonEmpty(value: unknown) {
  return typeof value === 'string' && value.trim() ? value.trim() : ''
}

function normalizeKey(value: string | undefined) {
  return (value || '')
    .trim()
    .toLowerCase()
    .replace(/[·•・]/g, '.')
    .replace(/\s+/g, '')
}

function formatChapterLabel(index: number | undefined) {
  return typeof index === 'number' ? `第 ${index + 1} 章` : undefined
}

function cloneMemory(memory: AiBookMemoryV2): AiBookMemoryV2 {
  return JSON.parse(JSON.stringify(memory)) as AiBookMemoryV2
}
