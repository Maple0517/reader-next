import type { AiBookLocation, AiBookNote } from '../types'

export interface AiBookWorldviewGroup {
  category: string
  items: AiBookNote[]
  collapsed: boolean
}

export interface AiBookLocationRow {
  location: AiBookLocation
  depth: number
  hasChildren: boolean
}

export function groupAiBookWorldview(notes: AiBookNote[], collapsedCategories: Set<string> = new Set()): AiBookWorldviewGroup[] {
  const groups = new Map<string, AiBookNote[]>()
  for (const note of notes) {
    if (isLowImportance(note.importance)) continue
    const category = note.category?.trim() || '基础设定'
    const items = groups.get(category) || []
    if (!items.some((item) => normalizeKey(item.title) === normalizeKey(note.title))) {
      items.push(note)
    }
    groups.set(category, items)
  }

  const order = ['基础设定', '基础规则', '势力制度', '历史传说', '技术/魔法', '社会文化', '当前事件', '未确认信息']
  return [...groups.entries()]
    .map(([category, items]) => ({
      category,
      items,
      collapsed: collapsedCategories.has(normalizeKey(category)),
    }))
    .sort((left, right) => {
      const leftIndex = order.indexOf(left.category)
      const rightIndex = order.indexOf(right.category)
      if (leftIndex >= 0 || rightIndex >= 0) {
        return (leftIndex >= 0 ? leftIndex : 999) - (rightIndex >= 0 ? rightIndex : 999)
      }
      return left.category.localeCompare(right.category, 'zh-CN')
    })
}

export function buildAiBookLocationRows(locations: AiBookLocation[], collapsedLocations: Set<string> = new Set()): AiBookLocationRow[] {
  const resolvedLocations = inferLocationParents(locations)
  const byKey = new Map(resolvedLocations.map((location) => [normalizeKey(location.name), location]))
  const childrenByParent = new Map<string, AiBookLocation[]>()
  const roots: AiBookLocation[] = []

  for (const location of resolvedLocations) {
    const parentKey = location.parentName ? normalizeKey(location.parentName) : ''
    const ownKey = normalizeKey(location.name)
    if (parentKey && parentKey !== ownKey && byKey.has(parentKey)) {
      const children = childrenByParent.get(parentKey) || []
      children.push(location)
      childrenByParent.set(parentKey, children)
    } else {
      roots.push(location)
    }
  }

  for (const children of childrenByParent.values()) {
    children.sort(compareLocations)
  }
  roots.sort(compareLocations)

  const rows: AiBookLocationRow[] = []
  const append = (location: AiBookLocation, depth: number, stack: Set<string>) => {
    const key = normalizeKey(location.name)
    const children = childrenByParent.get(key) || []
    rows.push({ location, depth, hasChildren: children.length > 0 })
    if (!children.length || collapsedLocations.has(key) || stack.has(key)) return
    const nextStack = new Set(stack)
    nextStack.add(key)
    for (const child of children) {
      append(child, depth + 1, nextStack)
    }
  }

  for (const root of roots) {
    append(root, 0, new Set())
  }
  return rows
}

function inferLocationParents(locations: AiBookLocation[]) {
  const unique = new Map<string, AiBookLocation>()
  for (const location of locations) {
    if (!location.name || isLowImportance(location.importance)) continue
    const key = normalizeKey(location.name)
    const normalizedParent = location.parentName && normalizeKey(location.parentName) !== key
      ? location.parentName
      : undefined
    unique.set(key, {
      ...location,
      parentName: normalizedParent,
    })
  }

  const values = [...unique.values()]
  const byKey = new Map(values.map((location) => [normalizeKey(location.name), location]))
  const parentCandidates = values.filter((location) => isParentLikeKind(location.kind) || !isChildLikeKind(location.kind))
  const defaultParentCandidates = parentCandidates.filter((location) => isStrongParentKind(location.kind))

  return values.map((location) => {
    const explicitParent = location.parentName ? byKey.get(normalizeKey(location.parentName)) : undefined
    if (explicitParent && isValidLocationParent(explicitParent, location)) {
      return location
    }
    const baseLocation = explicitParent ? { ...location, parentName: undefined } : location

    const inferredByText = findParentMention(baseLocation, parentCandidates)
    if (inferredByText) {
      return { ...baseLocation, parentName: inferredByText.name }
    }

    const inferredByContainer = findContainerMention(baseLocation, parentCandidates)
    if (inferredByContainer) {
      return { ...baseLocation, parentName: inferredByContainer.name }
    }

    const fallbackParent = findDefaultParent(baseLocation, defaultParentCandidates)
    if (fallbackParent) {
      return { ...baseLocation, parentName: fallbackParent.name }
    }

    return baseLocation
  })
}

function findParentMention(location: AiBookLocation, candidates: AiBookLocation[]) {
  const ownKey = normalizeKey(location.name)
  const text = normalizeKey([
    location.description,
    location.status,
    location.parentName,
  ].filter(Boolean).join(' '))

  return candidates
    .filter((candidate) => normalizeKey(candidate.name) !== ownKey)
    .filter((candidate) => isValidLocationParent(candidate, location))
    .filter((candidate) => text.includes(normalizeKey(candidate.name)))
    .sort((left, right) => parentScore(right) - parentScore(left) || right.name.length - left.name.length)[0]
}

function findContainerMention(location: AiBookLocation, candidates: AiBookLocation[]) {
  const ownKey = normalizeKey(location.name)
  return candidates
    .filter((candidate) => normalizeKey(candidate.name) !== ownKey)
    .filter((candidate) => isValidLocationParent(candidate, location))
    .filter((candidate) => normalizeKey([
      candidate.description,
      candidate.status,
      candidate.parentName,
    ].filter(Boolean).join(' ')).includes(ownKey))
    .sort((left, right) => parentScore(right) - parentScore(left) || right.name.length - left.name.length)[0]
}

function findDefaultParent(location: AiBookLocation, candidates: AiBookLocation[]) {
  if (!isChildLikeKind(location.kind)) return undefined
  const localParents = candidates.filter((candidate) => {
    if (normalizeKey(candidate.name) === normalizeKey(location.name)) return false
    if (!isValidLocationParent(candidate, location)) return false
    return isCityLikeKind(candidate.kind) || isTownLikeKind(candidate.kind)
  })
  if (localParents.length === 1) return localParents[0]

  const validParents = candidates.filter((candidate) => {
    if (normalizeKey(candidate.name) === normalizeKey(location.name)) return false
    return isValidLocationParent(candidate, location)
  })
  return validParents.length === 1 ? validParents[0] : undefined
}

function compareLocations(left: AiBookLocation, right: AiBookLocation) {
  return locationRank(right) - locationRank(left)
    || importanceRank(right.importance) - importanceRank(left.importance)
    || (left.kind || '').localeCompare(right.kind || '', 'zh-CN')
    || left.name.localeCompare(right.name, 'zh-CN')
}

function locationRank(location: AiBookLocation) {
  const rank = locationHierarchyLevel(location.kind)
  if (rank >= 70) return 4
  if (rank >= 50) return 3
  if (rank >= 40) return 2
  if (isChildLikeKind(location.kind)) return 0
  return 1
}

function parentScore(location: AiBookLocation) {
  return locationHierarchyLevel(location.kind) * 10 + importanceRank(location.importance)
}

function isStrongParentKind(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['world', 'continent', 'country', 'nation', 'kingdom', 'empire', 'region', 'province', 'state', 'county', 'territory', 'city', 'town', 'district', 'neighborhood', 'quarter', 'village', 'street', '大陆', '世界', '国家', '王国', '帝国', '区域', '地区', '省', '州', '郡', '城市', '城镇', '市', '城', '村落', '村', '街区', '社区']
    .some((item) => key.includes(item))
}

function isParentLikeKind(kind: string | undefined) {
  const key = normalizeKey(kind)
  return isStrongParentKind(kind)
    || ['school', 'academy', 'college', 'university', 'campus', 'church', 'organization', 'manor', 'camp', '教会', '组织', '庄园', '营地', '学校', '学院', '大学'].some((item) => key.includes(item))
}

function isChildLikeKind(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['building', 'school', 'academy', 'college', 'university', 'house', 'apartment', 'home', 'room', 'classroom', 'study', 'shop', 'store', 'lab', 'laboratory', 'facility', 'site', 'grounds', '住宅', '公寓', '住处', '房间', '建筑', '机构', '学校', '学院', '大学', '教室', '书房', '酒馆', '店铺', '商店', '机房', '实验室', '避难所', '设施']
    .some((item) => key.includes(item))
}

function isValidLocationParent(parent: AiBookLocation, child: AiBookLocation) {
  if (locationHierarchyLevel(parent.kind) > locationHierarchyLevel(child.kind)) return true
  return isSchoolLikeKind(parent.kind) && isFacilityLikeKind(child.kind)
}

function locationHierarchyLevel(kind: string | undefined) {
  const key = normalizeKey(kind)
  if (!key) return 35
  if (['world', '世界'].some((item) => key.includes(item))) return 90
  if (['continent', '大陆', '洲'].some((item) => key.includes(item))) return 80
  if (['country', 'nation', 'kingdom', 'empire', '国家', '王国', '帝国'].some((item) => key.includes(item))) return 70
  if (['region', 'province', 'state', 'county', 'territory', '区域', '地区', '省', '州', '郡', '领'].some((item) => key.includes(item))) return 60
  if (isCityLikeKind(kind)) return 50
  if (isTownLikeKind(kind)) return 40
  if (['manor', 'camp', 'church', 'organization', '庄园', '营地', '教会', '组织'].some((item) => key.includes(item))) return 35
  if (['building', 'school', 'academy', 'college', 'university', 'house', 'apartment', 'home', 'shop', 'store', '学校', '学院', '大学', '建筑', '机构', '住宅', '公寓', '住处', '酒馆', '店铺', '商店', '避难所'].some((item) => key.includes(item))) return 30
  if (['room', 'classroom', 'study', 'lab', 'laboratory', 'facility', 'site', 'grounds', 'yard', '房间', '教室', '书房', '机房', '实验室', '设施', '场地', '遗址'].some((item) => key.includes(item))) return 20
  return 35
}

function isCityLikeKind(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['city', 'town', '城市', '城镇', '市', '城'].some((item) => key.includes(item))
}

function isTownLikeKind(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['district', 'neighborhood', 'quarter', 'village', 'street', '村落', '村', '街区', '社区', '街道'].some((item) => key.includes(item))
}

function isSchoolLikeKind(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['school', 'academy', 'college', 'university', '学校', '学院', '大学'].some((item) => key.includes(item))
}

function isFacilityLikeKind(kind: string | undefined) {
  const key = normalizeKey(kind)
  return ['facility', 'site', 'grounds', 'campus', 'yard', '设施', '场地', '训练场', '遗址'].some((item) => key.includes(item))
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

function normalizeKey(value: string | undefined) {
  return (value || '')
    .trim()
    .toLowerCase()
    .replace(/[·•・]/g, '.')
    .replace(/\s+/g, '')
}
