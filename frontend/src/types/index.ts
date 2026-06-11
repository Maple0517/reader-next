// ─── API 统一返回 ───
export interface ApiResponse<T = unknown> {
  isSuccess: boolean
  errorMsg: string
  data: T
}

// ─── 书籍 ───
export interface Book {
  name: string
  author: string
  bookUrl: string
  origin: string
  originName?: string
  coverUrl?: string
  tocUrl?: string
  charset?: string
  customCoverUrl?: string
  canUpdate?: boolean
  durChapterIndex?: number
  durChapterPos?: number
  durChapterTime?: number
  durChapterTitle?: string
  intro?: string
  latestChapterTitle?: string
  lastCheckTime?: number
  totalChapterNum?: number
  type?: number
  group?: number
  wordCount?: string
  infoHtml?: string
  tocHtml?: string
  kind?: string
  updateTime?: string
  cachedChapterCount?: number
  browserCachedChapterCount?: number
  recentKind?: 'book' | 'rss'
  rssSourceUrl?: string
  rssLink?: string
  rssPubDate?: string
}

// ─── 搜索结果 ───
export interface SearchBook {
  name: string
  author: string
  bookUrl: string
  origin: string
  originName?: string
  originGroup?: string
  coverUrl?: string
  intro?: string
  kind?: string
  lastChapter?: string
  updateTime?: string
  wordCount?: string
  bookSourceUrls?: string[]
}

// ─── 章节 ───
export interface BookChapter {
  title: string
  url: string
  index: number
}

// ─── 书源 ───
export interface BookSource {
  bookSourceName: string
  bookSourceGroup?: string
  bookSourceUrl: string
  bookSourceType?: number
  enabled?: boolean
  enabledExplore?: boolean
  enabledCookieJar?: boolean
  customOrder?: number
  weight?: number
  searchUrl?: string
  exploreUrl?: string
  header?: string
  loginUrl?: string
  loginCheckJs?: string
  loadWithBaseUrl?: boolean
  singleUrl?: boolean
  ruleSearch?: Record<string, unknown>
  ruleExplore?: Record<string, unknown>
  ruleBookInfo?: Record<string, unknown>
  ruleToc?: Record<string, unknown>
  ruleContent?: Record<string, unknown>
}

export interface BookSourceTestResult {
  bookSourceName: string
  bookSourceUrl: string
  valid: boolean
  searchOk: boolean
  exploreOk: boolean
  keyword: string
  exploreUrl?: string
  searchError?: string
  exploreError?: string
  markedInvalid: boolean
  group?: string
}

export interface BookSourceTestResponse {
  total: number
  valid: number
  invalid: number
  markedInvalid: number
  results: BookSourceTestResult[]
}

// ─── 分组 ───
export interface BookGroup {
  groupId: number
  groupName: string
  orderNo?: number
}

// ─── 用户 ───
export interface UserInfo {
  username: string
  lastLoginAt?: number
  accessToken: string
  enableWebdav?: boolean
  enableLocalStore?: boolean
  enableAiModel?: boolean
  createdAt?: number
  isAdmin?: boolean
}

// ─── 应用更新 ───
export interface VersionUpdateInfo {
  currentVersion: string
  latestVersion: string | null
  latestName: string | null
  releaseUrl: string | null
  publishedAt: string | null
  updateAvailable: boolean
  shouldRemind: boolean
  dismissedVersion: string | null
  checkedAt: number
  error: string | null
}

// ─── 书签 ───
export interface Bookmark {
  time?: number
  bookName: string
  bookAuthor: string
  chapterIndex?: number
  chapterPos?: number
  chapterName?: string
  bookText?: string
  content?: string
}

// ─── 净化规则 ───
export interface ReplaceRule {
  id: number
  name: string
  group?: string
  pattern: string
  replacement: string
  scope?: string
  isEnabled: boolean
  isRegex: boolean
  order: number
}

// ─── RSS ───
export interface RssSource {
  sourceUrl: string
  sourceName: string
  sourceIcon?: string
  sourceGroup?: string
  sourceComment?: string
  enabled?: boolean
  enabledCookieJar?: boolean
  concurrentRate?: string
  header?: string
  loginUrl?: string
  loginCheckJs?: string
  sortUrl?: string
  singleUrl?: boolean
  articleStyle?: number
  ruleArticles?: string
  ruleNextPage?: string
  ruleTitle?: string
  rulePubDate?: string
  ruleDescription?: string
  ruleImage?: string
  ruleLink?: string
  ruleContent?: string
  style?: string
  enableJs?: boolean
  loadWithBaseUrl?: boolean
  customOrder?: number
  lastUpdateTime?: number
}

export interface RssArticle {
  origin: string
  sort: string
  title: string
  order: number
  link: string
  pubDate?: string
  description?: string
  content?: string
  image?: string
  read?: boolean
  variable?: string
}

// ─── AI 设定集 ───
export interface AiBookConfig {
  modelSource: 'browser' | 'server'
  textBaseUrl: string
  textApiKey: string
  textModel: string
  textUseFullUrl: boolean
  imageBaseUrl: string
  imageApiKey: string
  imageModel: string
  imageSize: string
  imageUseFullUrl: boolean
  useBackendProxy: boolean
}

export interface AiModelEndpointConfig {
  enabled: boolean
  baseUrl: string
  apiKey: string
  model: string
  useFullUrl: boolean
}

export interface AiImageModelConfig extends AiModelEndpointConfig {
  imageSize: string
}

export interface AiSpeechModelConfig extends AiModelEndpointConfig {
  voice: string
  responseFormat: string
}

export interface AiServerModelConfig {
  text: AiModelEndpointConfig
  image: AiImageModelConfig
  speech: AiSpeechModelConfig
}

export interface AiServerModelConfigResponse {
  config: AiServerModelConfig
  canUseServerModel: boolean
  isAdmin: boolean
}

export interface AiBookNote {
  title: string
  content: string
  category?: string
  confidence?: string
  importance?: string
}

export interface AiBookCharacter {
  name: string
  aliases?: string[]
  status: string
  faction?: string
  location?: string
  description?: string
  lastSeenChapter?: string
  importance?: string
}

export interface AiBookRelationship {
  source: string
  target: string
  relation: string
  status?: string
  description?: string
  importance?: string
}

export interface AiBookLocation {
  name: string
  kind?: string
  parentName?: string
  description: string
  status?: string
  relatedCharacters?: string[]
  firstSeenChapter?: string
  importance?: string
}

export interface AiBookMap {
  imageUrl?: string
  prompt?: string
  updatedAt?: number
  sourceChapterIndex?: number
  fallback?: 'relationship-graph'
  fallbackReason?: string
}

export interface AiBookMemory {
  bookUrl: string
  bookName?: string
  author?: string
  enabled: boolean
  processedChapterIndex?: number
  processedChapterTitle?: string
  updatedAt: number
  summary?: string
  worldview: AiBookNote[]
  characters: AiBookCharacter[]
  relationships: AiBookRelationship[]
  locations: AiBookLocation[]
  map?: AiBookMap | null
  mapDirty?: boolean
  lastError?: string
}

export interface AiBookModelUpdate {
  memory: AiBookAnyMemory
  shouldRegenerateMap: boolean
  mapPrompt?: string
}

export type AiBookImportance = 'high' | 'medium' | 'low'
export type AiBookConfidence = '已知' | '推断' | '未知'
export type AiBookLocationScale =
  | 'world'
  | 'continent'
  | 'country'
  | 'region'
  | 'city'
  | 'district'
  | 'site'
  | 'building'
  | 'room'
  | 'unknown'

export interface AiBookEvidence {
  chapterIndex: number
  chapterTitle: string
  quote?: string
  note: string
}

export interface AiBookSummaryState {
  current: string
  recentChanges: string[]
  openQuestions: string[]
}

export interface AiBookChapterDigest {
  chapterIndex: number
  chapterTitle: string
  digest: string
  keyEvents: string[]
  touchedEntityIds: string[]
  createdAt: number
}

export interface AiBookArcSummary {
  id: string
  startChapterIndex: number
  endChapterIndex: number
  title: string
  summary: string
  keyEntityIds: string[]
}

export interface AiBookWorldFact {
  id: string
  category: string
  title: string
  content: string
  confidence: AiBookConfidence
  importance: AiBookImportance
  firstSeenChapterIndex?: number
  lastConfirmedChapterIndex?: number
  evidence: AiBookEvidence[]
  supersedes?: string[]
}

export interface AiBookEntityState {
  chapterIndex: number
  chapterTitle: string
  status: string
  locationId?: string
  faction?: string
  evidence?: AiBookEvidence
}

export interface AiBookCharacterV2 {
  id: string
  name: string
  aliases: string[]
  importance: AiBookImportance
  ambiguous?: boolean
  currentStatus: string
  faction?: string
  currentLocationId?: string
  description?: string
  firstSeenChapterIndex?: number
  lastSeenChapterIndex?: number
  statusHistory: AiBookEntityState[]
  evidence: AiBookEvidence[]
}

export interface AiBookRelationshipV2 {
  id: string
  sourceCharacterId: string
  targetEntityId: string
  targetKind: 'character' | 'location' | 'organization'
  relationType: string
  direction: 'directed' | 'undirected'
  currentStatus?: string
  description?: string
  importance: AiBookImportance
  firstSeenChapterIndex?: number
  lastSeenChapterIndex?: number
  evidence: AiBookEvidence[]
}

export interface AiBookLocationV2 {
  id: string
  name: string
  aliases: string[]
  importance: AiBookImportance
  kind: string
  scale: AiBookLocationScale
  parentId?: string
  description: string
  currentStatus?: string
  relatedCharacterIds: string[]
  firstSeenChapterIndex?: number
  lastSeenChapterIndex?: number
  evidence: AiBookEvidence[]
}

export interface AiBookMapNode {
  id: string
  locationId: string
  label: string
  scale: AiBookLocationScale
  parentNodeId?: string
  status?: string
}

export interface AiBookMapEdge {
  id: string
  sourceNodeId: string
  targetNodeId: string
  kind: 'contains' | 'route' | 'adjacent' | 'character-movement'
  label?: string
  evidence?: AiBookEvidence
}

export interface AiBookMapState {
  dirty: boolean
  reason?: string
  lastRenderedAt?: number
  sourceChapterIndex?: number
  mapPrompt?: string
  nodes: AiBookMapNode[]
  edges: AiBookMapEdge[]
}

export interface AiBookRenderArtifacts {
  mapImageUrl?: string
  mapImagePrompt?: string
  mapFallbackReason?: string
}

export interface AiBookMemoryV2 {
  schemaVersion: 2
  bookUrl: string
  bookName?: string
  author?: string
  enabled: boolean
  processedChapterIndex?: number
  processedChapterTitle?: string
  updatedAt: number
  lastError?: string
  summary: AiBookSummaryState
  chapterDigests: AiBookChapterDigest[]
  arcs: AiBookArcSummary[]
  worldFacts: AiBookWorldFact[]
  characters: AiBookCharacterV2[]
  relationships: AiBookRelationshipV2[]
  locations: AiBookLocationV2[]
  mapState: AiBookMapState
  renderArtifacts: AiBookRenderArtifacts
}

export interface AiBookPatchWorldFact {
  id?: string
  category?: string
  title: string
  content: string
  confidence?: AiBookConfidence
  importance?: AiBookImportance
  evidence?: AiBookEvidence[]
}

export interface AiBookPatchCharacter {
  id?: string
  name: string
  aliases?: string[]
  importance?: AiBookImportance
  currentStatus?: string
  status?: string
  faction?: string
  locationName?: string
  description?: string
  evidence?: AiBookEvidence[]
}

export interface AiBookPatchRelationship {
  id?: string
  sourceId?: string
  sourceName?: string
  targetId?: string
  targetName?: string
  targetKind?: 'character' | 'location' | 'organization'
  relationType?: string
  relation?: string
  direction?: 'directed' | 'undirected'
  currentStatus?: string
  status?: string
  description?: string
  importance?: AiBookImportance
  evidence?: AiBookEvidence[]
}

export interface AiBookPatchLocation {
  id?: string
  name: string
  aliases?: string[]
  kind?: string
  scale?: AiBookLocationScale
  parentId?: string
  parentName?: string
  description?: string
  currentStatus?: string
  status?: string
  relatedCharacterNames?: string[]
  relatedCharacterIds?: string[]
  importance?: AiBookImportance
  evidence?: AiBookEvidence[]
}

export interface AiBookChapterKnowledgePatch {
  chapterDigest: Omit<AiBookChapterDigest, 'touchedEntityIds' | 'createdAt'> & Partial<Pick<AiBookChapterDigest, 'touchedEntityIds' | 'createdAt'>>
  summary?: Partial<AiBookSummaryState>
  facts?: AiBookPatchWorldFact[]
  worldFacts?: AiBookPatchWorldFact[]
  characters?: AiBookPatchCharacter[]
  relationships?: AiBookPatchRelationship[]
  locations?: AiBookPatchLocation[]
  mapChanges?: {
    changed: boolean
    reason?: string
    affectedLocationNames: string[]
    routeHints: string[]
  }
}

export type AiBookAnyMemory = AiBookMemory | AiBookMemoryV2
