import type {
  AiBookAnyMemory,
  AiBookConfig,
  AiBookCharacter,
  AiBookChapterKnowledgePatch,
  AiBookLocation,
  AiBookMap,
  AiBookMemory,
  AiBookModelUpdate,
  AiBookNote,
  AiBookRelationship,
  Book,
  BookChapter,
} from '../types'
import { isAiBookConfigReady, isAiBookImageConfigReady } from './aiBookConfig'
import { isAiBookMemoryV2, reconcileAiBookMemoryV2 } from './aiBookV2'
import { summarizeHttpErrorBody } from './httpError'

export type AiBookChatMessage = {
  role: 'system' | 'user' | 'assistant' | 'tool'
  content?: string | null
  tool_calls?: AiBookToolCall[]
  tool_call_id?: string
  name?: string
}

export type AiBookToolCall = {
  id: string
  type?: 'function'
  function?: {
    name?: string
    arguments?: string
  }
}

export interface BuildPromptParams {
  bookName: string
  chapterTitle: string
  chapterIndex: number
  chapterContent: string
  memory: AiBookAnyMemory
}

export interface GenerateMemoryParams {
  config: AiBookConfig
  book: Book
  chapter: BookChapter
  chapterContent: string
  memory: AiBookAnyMemory
  fetchImpl?: typeof fetch
}

export interface GenerateMapParams {
  config: AiBookConfig
  prompt: string
  fetchImpl?: typeof fetch
}

export interface UploadGeneratedMapParams {
  b64Json?: string
  imageUrl?: string
  filename: string
  useBackendProxy?: boolean
  fetchImpl?: typeof fetch
}

export interface ApplyMapFallbackParams {
  prompt: string
  reason: string
  sourceChapterIndex?: number
  updatedAt?: number
}

interface OpenAIChatResponse {
  choices?: Array<{
    message?: {
      content?: string | null
      tool_calls?: AiBookToolCall[]
    }
  }>
}

interface OpenAIImageResponse {
  data?: Array<{
    b64_json?: string
    url?: string
  }>
}

interface AiProxyRequestParams {
  config: AiBookConfig
  baseUrl: string
  apiKey: string
  fullUrl: boolean
  path: '/v1/chat/completions' | '/v1/images/generations'
  body: Record<string, unknown>
  fetchImpl: typeof fetch
}

interface AiBookRawModelUpdate {
  memory?: UnknownRecord
  memoryPatch?: UnknownRecord
  patch?: UnknownRecord
  shouldRegenerateMap?: boolean
  mapPrompt?: string
  summary?: string
  worldview?: unknown
  characters?: unknown
  relationships?: unknown
  locations?: unknown
  mapDirty?: boolean
  chapterDigest?: unknown
  worldFacts?: unknown
  facts?: unknown
  mapChanges?: unknown
}

type AiBookToolResult = {
  content: Record<string, unknown>
  final?: boolean
  raw?: AiBookRawModelUpdate
}

const MAX_AI_BOOK_AGENT_STEPS = 6
const AI_BOOK_TOOL_GET_MEMORY = 'get_current_memory'
const AI_BOOK_TOOL_GET_CHAPTER = 'get_completed_chapter'
const AI_BOOK_TOOL_SAVE_PATCH = 'save_memory_patch'

const AI_BOOK_AGENT_TOOLS = [
  {
    type: 'function',
    function: {
      name: AI_BOOK_TOOL_GET_MEMORY,
      description: '读取当前已保存的小说 AI 资料。只返回已读进度内的结构化记忆。',
      parameters: {
        type: 'object',
        properties: {},
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: AI_BOOK_TOOL_GET_CHAPTER,
      description: '读取本次需要处理的已完成章节正文。不会返回未读章节。',
      parameters: {
        type: 'object',
        properties: {},
        additionalProperties: false,
      },
    },
  },
  {
    type: 'function',
    function: {
      name: AI_BOOK_TOOL_SAVE_PATCH,
      description: '提交本章带来的结构化资料增量。必须在已读取当前资料和章节后调用一次作为最终结果。',
      parameters: {
        type: 'object',
        additionalProperties: false,
        properties: {
          memory: {
            type: 'object',
            additionalProperties: true,
            description: '增量资料，不要整包覆盖。可包含 summary、worldview、characters、relationships、locations。',
          },
          shouldRegenerateMap: {
            type: 'boolean',
            description: '只有重要地点、层级、路线或地图结构变化时为 true。',
          },
          mapPrompt: {
            type: 'string',
            description: '需要重绘地图时的俯视二维制图提示词。',
          },
        },
        required: ['memory', 'shouldRegenerateMap'],
      },
    },
  },
]

export function shouldRunAiBookAutoUpdate(
  memory: AiBookAnyMemory | null | undefined,
  completedChapterIndex: number,
  config: AiBookConfig,
) {
  if (!memory?.enabled) return false
  if (!isAiBookConfigReady(config)) return false
  if (typeof memory.processedChapterIndex === 'number' && memory.processedChapterIndex >= completedChapterIndex) {
    return false
  }
  return true
}

export function buildAiBookPromptMessages({
  bookName,
  chapterTitle,
  chapterIndex,
}: BuildPromptParams): AiBookChatMessage[] {
  return [
    {
      role: 'system',
      content: [
        '你是小说阅读资料维护 agent。',
        '不得使用未读章节，不得补充未来剧情，不得剧透。',
        '必须通过工具按需读取当前资料和本次已完成章节，然后只提交增量 patch。',
        `必须先调用 ${AI_BOOK_TOOL_GET_MEMORY} 和 ${AI_BOOK_TOOL_GET_CHAPTER}，最后调用 ${AI_BOOK_TOOL_SAVE_PATCH} 完成更新。`,
        '不要在普通文本中输出最终 JSON；最终结果必须放在 save_memory_patch 工具参数里。',
        '无法确认的信息必须标记为“推断”或“未知”。',
        'summary 用于记录已读剧情进展；worldview 不是章节简介。',
        '优先输出 V2 ChapterKnowledgePatch：chapterDigest、summary、worldFacts、characters、relationships、locations、mapChanges。',
        'chapterDigest 只概括当前章节；summary.current 是已读全局局势；worldFacts 是可复用设定；所有关键条目必须带 evidence。',
        'mapChanges 只描述地点结构、路线、区域边界变化，不要因普通人物状态变化触发。',
        'worldview 必须是跨章节可复用的设定集条目，只记录规则、制度、势力、历史、技术/魔法、社会文化、地理环境、组织体系、未确认设定。',
        'worldview 禁止写成本章剧情复述、人物行动流水账、案件经过、章节摘要；不要使用“本章”“这一章”“第X章”作为设定标题或内容主体。',
        '世界观必须按 category 分类，例如：基础规则、势力制度、历史传说、技术/魔法、社会文化、地理环境、组织体系、未确认信息。',
        '角色和关系必须填写 importance: high|medium|low；只保留推动剧情、反复出现或明确影响主角行动的 high/medium 项。',
        '不要输出不重要、路人、一次性提及、无状态变化的角色；不要输出寒暄、同村、路过、单纯“认识”等低价值关系。',
        '人物关系必须去重：同一对人物的同类关系只输出一条，不要再输出反向重复项；保留信息量更高的描述。',
        '地点必须填写 parentName 表示层级归属；父级必须比子级尺度更大：国家 > 区域/郡 > 城市 > 街区/村镇 > 学校/建筑/住宅 > 房间/设施。',
        '禁止把国家挂在城市下面，禁止把城市挂在学校、建筑、住宅、房间等子地点下面；无法确认父级时 parentName 留空。',
        '只有新增重要地点、地点层级、路线、区域边界或地图结构变化时，shouldRegenerateMap 才能为 true；单纯角色状态或人物关系变化必须为 false。',
        '生成 mapPrompt 时必须写成俯视地图/二维制图提示词，强调区域边界、路线、图例、地图符号和地点标签。',
        'mapPrompt 不要写成场景照片、建筑照片、室内渲染或人物插画；机房、避难所等地点只能作为地图上的标注区域、平面轮廓或图标。',
      ].join('\n'),
    },
    {
      role: 'user',
      content: JSON.stringify({
        task: 'tool-calling-ai-book-memory-update',
        finalTool: AI_BOOK_TOOL_SAVE_PATCH,
        preferredPatchSchema: 'ChapterKnowledgePatch',
        v2PatchSchema: {
          chapterDigest: {
            chapterIndex: 'number',
            chapterTitle: 'string',
            digest: 'string，当前章节短摘要',
            keyEvents: ['string，当前章节关键事件'],
          },
          summary: {
            current: 'string，已读范围内的当前全局局势，不写成单章流水账',
            recentChanges: ['string，最近关键变化'],
            openQuestions: ['string，已读范围内未确认问题'],
          },
          worldFacts: [{
            id: 'string optional，已知实体 id 可复用',
            category: '基础规则|势力制度|历史传说|技术/魔法|社会文化|地理环境|组织体系|未确认信息',
            title: 'string，设定名',
            content: 'string，稳定设定说明',
            confidence: '已知|推断|未知',
            importance: 'high|medium|low',
            evidence: [{ chapterIndex: 'number', chapterTitle: 'string', quote: 'string optional', note: 'string' }],
          }],
          characters: [{
            id: 'string optional',
            name: 'string',
            aliases: ['string'],
            importance: 'high|medium|low',
            currentStatus: 'string',
            faction: 'string optional',
            locationName: 'string optional',
            description: 'string optional',
            evidence: [{ chapterIndex: 'number', chapterTitle: 'string', quote: 'string optional', note: 'string' }],
          }],
          relationships: [{
            sourceName: 'string',
            targetName: 'string',
            targetKind: 'character|location|organization',
            relationType: 'string',
            direction: 'directed|undirected',
            currentStatus: 'string optional',
            description: 'string optional',
            importance: 'high|medium|low',
            evidence: [{ chapterIndex: 'number', chapterTitle: 'string', quote: 'string optional', note: 'string' }],
          }],
          locations: [{
            id: 'string optional',
            name: 'string',
            aliases: ['string'],
            kind: 'string',
            scale: 'world|continent|country|region|city|district|site|building|room|unknown',
            parentName: 'string optional',
            description: 'string',
            currentStatus: 'string optional',
            importance: 'high|medium|low',
            evidence: [{ chapterIndex: 'number', chapterTitle: 'string', quote: 'string optional', note: 'string' }],
          }],
          mapChanges: {
            changed: 'boolean',
            reason: 'string optional',
            affectedLocationNames: ['string'],
            routeHints: ['string'],
          },
        },
        patchSchema: {
          summary: 'string，已读剧情进展摘要，允许写章节事件',
          worldview: [{
            category: '基础规则|势力制度|历史传说|技术/魔法|社会文化|地理环境|组织体系|未确认信息',
            title: 'string，设定名，不要写“本章/第X章/剧情”',
            content: 'string，稳定设定说明，不要写章节流水账',
            confidence: '已知|推断|未知',
            importance: 'high|medium|low',
          }],
          characters: [{
            name: 'string',
            aliases: ['string'],
            status: 'string',
            faction: 'string',
            location: 'string',
            description: 'string',
            lastSeenChapter: 'string',
            importance: 'high|medium|low',
          }],
          relationships: [{
            source: 'string',
            target: 'string',
            relation: 'string',
            status: 'string',
            description: 'string',
            importance: 'high|medium|low',
          }],
          locations: [{
            name: 'string',
            parentName: 'string or empty for top-level places',
            kind: 'string',
            description: 'string',
            status: 'string',
            relatedCharacters: ['string'],
            firstSeenChapter: 'string',
            importance: 'high|medium|low',
          }],
          shouldRegenerateMap: 'boolean',
          mapPrompt: 'string when map should be regenerated; must describe a top-down cartographic world map, not a scene/photo/building illustration',
        },
        qualityRules: [
          'worldview 必须有 category；同一 category 下不要重复 title；只写设定，不写本章简介。',
          '剧情经过、角色行动、调查过程、战斗过程写入 summary 或角色状态，不要写入 worldview。',
          'characters 只输出重要角色；背景人物、一次性称呼、无独立状态者不要输出。',
          'relationships 只输出重要关系；同一 source/target/relation 只保留一条，不要反向重复。',
          'locations 必须尽量给 parentName 形成正确层级，父级尺度必须大于子级；无法确认父级时留空。',
          'shouldRegenerateMap 只在地图相关地点信息发生重要变化时为 true。',
          '所有信息只来自工具返回的当前资料和当前章节；不确定就写 推断/未知。',
        ],
        bookName,
        chapter: {
          index: chapterIndex,
          title: chapterTitle,
        },
      }),
    },
  ]
}

export async function requestAiBookMemoryUpdate({
  config,
  book,
  chapter,
  chapterContent,
  memory,
  fetchImpl = fetch,
}: GenerateMemoryParams): Promise<AiBookModelUpdate> {
  const messages: AiBookChatMessage[] = buildAiBookPromptMessages({
    bookName: book.name,
    chapterTitle: chapter.title,
    chapterIndex: chapter.index,
    chapterContent,
    memory,
  })

  for (let step = 0; step < MAX_AI_BOOK_AGENT_STEPS; step += 1) {
    const response = await requestModelJson({
      config,
      baseUrl: config.textBaseUrl,
      apiKey: config.textApiKey,
      fullUrl: config.textUseFullUrl,
      path: '/v1/chat/completions',
      fetchImpl,
      body: {
        model: config.textModel,
        messages,
        tools: AI_BOOK_AGENT_TOOLS,
        tool_choice: 'auto',
        temperature: 0.2,
      },
    })

    if (!response.ok) {
      throw new Error(await readModelError(response, 'AI 资料生成失败'))
    }

    const data = await response.json() as OpenAIChatResponse
    const message = data.choices?.[0]?.message
    const toolCalls = Array.isArray(message?.tool_calls) ? message.tool_calls : []
    if (toolCalls.length) {
      messages.push({
        role: 'assistant',
        content: message?.content || null,
        tool_calls: toolCalls,
      })

      let finalUpdate: AiBookModelUpdate | null = null
      for (const toolCall of toolCalls) {
        const result = executeAiBookToolCall(toolCall, {
          book,
          chapter,
          chapterContent,
          memory,
        })
        messages.push({
          role: 'tool',
          tool_call_id: toolCall.id,
          name: toolCall.function?.name || '',
          content: JSON.stringify(result.content),
        })
        if (result.final && result.raw) {
          finalUpdate = coerceModelUpdate(result.raw, memory, book, chapter)
        }
      }
      if (finalUpdate) return finalUpdate
      continue
    }

    const content = message?.content
    if (content) {
      return coerceModelUpdate(parseJsonContent(content), memory, book, chapter)
    }
  }

  throw new Error('AI 资料生成超过工具调用轮次限制')
}

function executeAiBookToolCall(
  toolCall: AiBookToolCall,
  {
    book,
    chapter,
    chapterContent,
    memory,
  }: {
    book: Book
    chapter: BookChapter
    chapterContent: string
    memory: AiBookAnyMemory
  },
): AiBookToolResult {
  const name = toolCall.function?.name || ''
  const args = parseToolArguments(toolCall.function?.arguments || '{}')
  if (!args.ok) {
    return {
      content: {
        ok: false,
        error: args.error,
      },
    }
  }

  if (name === AI_BOOK_TOOL_GET_MEMORY) {
    return {
      content: {
        ok: true,
        memory: buildAgentMemoryContext(memory),
      },
    }
  }

  if (name === AI_BOOK_TOOL_GET_CHAPTER) {
    return {
      content: {
        ok: true,
        book: {
          name: book.name,
          author: book.author,
          bookUrl: book.bookUrl,
        },
        chapter: {
          index: chapter.index,
          title: chapter.title,
          content: chapterContent.slice(0, 24000),
        },
      },
    }
  }

  if (name === AI_BOOK_TOOL_SAVE_PATCH) {
    const raw = normalizeToolPatch(args.value)
    return {
      final: true,
      raw,
      content: {
        ok: true,
        accepted: true,
      },
    }
  }

  return {
    content: {
      ok: false,
      error: `未知工具：${name}`,
    },
  }
}

function buildAgentMemoryContext(memory: AiBookAnyMemory) {
  if (isAiBookMemoryV2(memory)) {
    return {
      schemaVersion: 2,
      bookUrl: memory.bookUrl,
      bookName: memory.bookName,
      author: memory.author,
      processedChapterIndex: memory.processedChapterIndex,
      processedChapterTitle: memory.processedChapterTitle,
      summary: memory.summary,
      recentChapterDigests: memory.chapterDigests.slice(-5),
      worldFacts: memory.worldFacts
        .filter((item) => item.importance !== 'low')
        .map(({ id, category, title, content, confidence, importance }) => ({ id, category, title, content, confidence, importance })),
      characters: memory.characters
        .filter((item) => item.importance !== 'low')
        .map(({ id, name, aliases, importance, currentStatus, faction, currentLocationId, description }) => ({
          id,
          name,
          aliases,
          importance,
          currentStatus,
          faction,
          currentLocationId,
          description,
        })),
      relationships: memory.relationships
        .filter((item) => item.importance !== 'low')
        .map(({ id, sourceCharacterId, targetEntityId, targetKind, relationType, direction, currentStatus, description, importance }) => ({
          id,
          sourceCharacterId,
          targetEntityId,
          targetKind,
          relationType,
          direction,
          currentStatus,
          description,
          importance,
        })),
      locations: memory.locations
        .map(({ id, name, aliases, importance, kind, scale, parentId, description, currentStatus }) => ({
          id,
          name,
          aliases,
          importance,
          kind,
          scale,
          parentId,
          description,
          currentStatus,
        })),
      mapState: {
        dirty: memory.mapState.dirty,
        reason: memory.mapState.reason,
        sourceChapterIndex: memory.mapState.sourceChapterIndex,
      },
    }
  }

  return {
    bookUrl: memory.bookUrl,
    bookName: memory.bookName,
    author: memory.author,
    processedChapterIndex: memory.processedChapterIndex,
    processedChapterTitle: memory.processedChapterTitle,
    summary: memory.summary || '',
    worldview: normalizeWorldview(memory.worldview || []),
    characters: normalizeCharacters(memory.characters || []),
    relationships: normalizeRelationships(memory.relationships || []),
    locations: normalizeLocations(memory.locations || []),
    map: memory.map
      ? {
        prompt: memory.map.prompt,
        sourceChapterIndex: memory.map.sourceChapterIndex,
        fallback: memory.map.fallback,
        fallbackReason: memory.map.fallbackReason,
      }
      : null,
    mapDirty: Boolean(memory.mapDirty),
  }
}

function parseToolArguments(input: string): { ok: true; value: UnknownRecord } | { ok: false; error: string } {
  try {
    const parsed = JSON.parse(input || '{}')
    if (!isRecord(parsed)) {
      return { ok: false, error: '工具参数必须是 JSON 对象' }
    }
    return { ok: true, value: parsed }
  } catch (error) {
    return { ok: false, error: `工具参数不是有效 JSON：${(error as Error).message}` }
  }
}

function normalizeToolPatch(args: UnknownRecord): AiBookRawModelUpdate {
  const memory = isRecord(args.memory)
    ? args.memory
    : isRecord(args.memoryPatch)
      ? args.memoryPatch
      : isRecord(args.patch)
        ? args.patch
        : args
  return {
    memory,
    shouldRegenerateMap: readBoolean(args, 'shouldRegenerateMap') || readBoolean(args, 'mapDirty'),
    mapPrompt: readString(args, 'mapPrompt'),
    mapDirty: readBoolean(args, 'mapDirty'),
  }
}

export async function requestAiBookMapImage({
  config,
  prompt,
  fetchImpl = fetch,
}: GenerateMapParams) {
  if (!isAiBookImageConfigReady(config)) {
    throw new Error('图片模型未配置')
  }

  const response = await requestModelJson({
    config,
    baseUrl: config.imageBaseUrl,
    apiKey: config.imageApiKey,
    fullUrl: config.imageUseFullUrl,
    path: '/v1/images/generations',
    fetchImpl,
    body: {
      model: config.imageModel,
      prompt: buildMapImagePrompt(prompt),
      size: config.imageSize || '1024x1024',
      response_format: 'b64_json',
      n: 1,
    },
  })

  if (!response.ok) {
    throw new Error(await readModelError(response, '地图生成失败'))
  }

  const data = await response.json() as OpenAIImageResponse
  const first = data.data?.[0]
  if (!first?.b64_json && !first?.url) {
    throw new Error('地图生成结果为空')
  }
  return {
    b64Json: first.b64_json,
    imageUrl: first.url,
  }
}

export async function uploadGeneratedMap({
  b64Json,
  imageUrl,
  filename,
  useBackendProxy = false,
  fetchImpl = fetch,
}: UploadGeneratedMapParams) {
  const upstreamImageUrl = imageUrl?.trim()
  const canKeepUpstreamImageUrl = Boolean(upstreamImageUrl && !isDataImageUrl(upstreamImageUrl))
  const blob = b64Json
    ? base64ToBlob(b64Json, 'image/png')
    : isDataImageUrl(upstreamImageUrl)
      ? dataUrlToBlob(upstreamImageUrl)
      : await fetchImageBlob(upstreamImageUrl || '', fetchImpl, useBackendProxy).catch((error) => {
        if (canKeepUpstreamImageUrl) return null
        throw error
      })

  if (!blob && canKeepUpstreamImageUrl && upstreamImageUrl) {
    return upstreamImageUrl
  }
  if (!blob) {
    throw new Error('地图图片下载失败')
  }

  const formData = new FormData()
  formData.append('file', blob, filename)

  const headers: Record<string, string> = {}
  const token = safeLocalStorageGet('accessToken')
  if (token) {
    headers.Authorization = token
  }

  const response = await fetchImpl('/reader3/uploadFile?type=ai-maps', {
    method: 'POST',
    headers,
    body: formData,
  })
  if (!response.ok) {
    if (canKeepUpstreamImageUrl && upstreamImageUrl) return upstreamImageUrl
    throw new Error(await readModelError(response, '地图上传失败'))
  }

  const data = await response.json() as {
    isSuccess?: boolean
    errorMsg?: string
    data?: string[]
  }
  if (data.isSuccess === false) {
    if (canKeepUpstreamImageUrl && upstreamImageUrl) return upstreamImageUrl
    throw new Error(data.errorMsg || '地图上传失败')
  }
  const url = Array.isArray(data.data) ? data.data[0] : ''
  if (!url) {
    if (canKeepUpstreamImageUrl && upstreamImageUrl) return upstreamImageUrl
    throw new Error('地图上传结果为空')
  }
  return url
}

export function createEmptyAiBookMemory(book: Book): AiBookMemory {
  return {
    bookUrl: book.bookUrl,
    bookName: book.name,
    author: book.author,
    enabled: false,
    processedChapterIndex: undefined,
    processedChapterTitle: undefined,
    updatedAt: Date.now(),
    summary: '',
    worldview: [],
    characters: [],
    relationships: [],
    locations: [],
    map: null,
    mapDirty: false,
    lastError: undefined,
  }
}

export function applyMapToMemory(memory: AiBookMemory, map: AiBookMap): AiBookMemory {
  return {
    ...memory,
    map,
    mapDirty: false,
    updatedAt: Date.now(),
  }
}

export function applyMapFallbackToMemory(
  memory: AiBookMemory,
  {
    prompt,
    reason,
    sourceChapterIndex,
    updatedAt = Date.now(),
  }: ApplyMapFallbackParams,
): AiBookMemory {
  return {
    ...memory,
    map: {
      prompt,
      updatedAt,
      sourceChapterIndex,
      fallback: 'relationship-graph',
      fallbackReason: reason,
    },
    mapDirty: true,
    updatedAt,
    lastError: undefined,
  }
}

function coerceModelUpdate(raw: AiBookRawModelUpdate, previous: AiBookAnyMemory, book: Book, chapter: BookChapter): AiBookModelUpdate {
  if (isAiBookMemoryV2(previous)) {
    return reconcileAiBookMemoryV2(previous, normalizeV2Patch(raw, chapter), book, chapter)
  }

  const rawRecord = raw as unknown as UnknownRecord
  const rawMemory = isRecord(raw.memory) ? raw.memory : rawRecord
  const worldviewSource = mergeIncrementalItems(previous.worldview, rawMemory.worldview)
  const characterSource = mergeIncrementalItems(previous.characters, rawMemory.characters)
  const relationshipSource = mergeIncrementalItems(previous.relationships, rawMemory.relationships)
  const locationSource = mergeIncrementalItems(previous.locations, rawMemory.locations)
  const worldview = normalizeWorldview(worldviewSource)
  const characters = normalizeCharacters(characterSource)
  const relationships = normalizeRelationships(relationshipSource)
  const locations = normalizeLocations(locationSource)
  const mapPrompt = typeof raw.mapPrompt === 'string' ? raw.mapPrompt.trim() : ''
  const shouldRegenerateMap = shouldAcceptMapRegeneration({
    requested: Boolean(raw.shouldRegenerateMap || raw.mapDirty),
    mapPrompt,
    previous,
    locations,
  })
  const memory: AiBookMemory = {
    ...previous,
    ...rawMemory,
    bookUrl: book.bookUrl,
    bookName: book.name,
    author: book.author,
    enabled: previous.enabled,
    processedChapterIndex: chapter.index,
    processedChapterTitle: chapter.title,
    updatedAt: Date.now(),
    summary: typeof rawMemory.summary === 'string' ? rawMemory.summary : previous.summary || '',
    worldview,
    characters,
    relationships,
    locations,
    map: previous.map || null,
    mapDirty: shouldRegenerateMap,
    lastError: undefined,
  }

  return {
    memory,
    shouldRegenerateMap,
    mapPrompt: shouldRegenerateMap ? mapPrompt : undefined,
  }
}

function normalizeV2Patch(raw: AiBookRawModelUpdate, chapter: BookChapter): AiBookChapterKnowledgePatch {
  const rawRecord = raw as unknown as UnknownRecord
  const candidate: UnknownRecord = isRecord(raw.patch)
    ? raw.patch
    : isRecord(raw.memoryPatch)
      ? raw.memoryPatch
      : isRecord(raw.memory) && isRecord(raw.memory.chapterDigest)
        ? raw.memory
        : rawRecord
  const patch = candidate as unknown as Partial<AiBookChapterKnowledgePatch>
  return {
    chapterDigest: patch.chapterDigest || {
      chapterIndex: chapter.index,
      chapterTitle: chapter.title,
      digest: readString(candidate, 'summary') || '',
      keyEvents: [],
    },
    summary: patch.summary,
    facts: patch.facts,
    worldFacts: patch.worldFacts,
    characters: patch.characters,
    relationships: patch.relationships,
    locations: patch.locations,
    mapChanges: patch.mapChanges,
  }
}

type UnknownRecord = Record<string, unknown>

function mergeIncrementalItems(previousItems: unknown[] | undefined, nextItems: unknown) {
  const previousArray = Array.isArray(previousItems) ? previousItems : []
  return Array.isArray(nextItems) ? [...previousArray, ...nextItems] : previousArray
}

function shouldAcceptMapRegeneration({
  requested,
  mapPrompt,
  previous,
  locations,
}: {
  requested: boolean
  mapPrompt: string
  previous: AiBookMemory
  locations: AiBookLocation[]
}) {
  if (!requested || !mapPrompt) return false
  if (!previous.map) return true
  return locationSignature(normalizeLocations(previous.locations || [])) !== locationSignature(locations)
}

function locationSignature(locations: AiBookLocation[]) {
  return locations
    .map((location) => [
      normalizeKey(location.name),
      normalizeKey(location.parentName),
      normalizeKey(location.kind),
      normalizeKey(location.status),
      normalizeKey(location.description),
    ].join(':'))
    .sort()
    .join('|')
}

function normalizeWorldview(items: unknown[]): AiBookNote[] {
  const notes = new Map<string, AiBookNote>()
  for (const item of items) {
    if (!isRecord(item) || isLowImportance(readString(item, 'importance'))) continue
    const title = readString(item, 'title')
    const content = readString(item, 'content')
    if (!title || !content) continue
    const category = readString(item, 'category') || '基础设定'
    if (isChapterSummaryWorldview(title, content, category)) continue
    const note: AiBookNote = {
      title,
      content,
      category,
      confidence: readString(item, 'confidence') || undefined,
      importance: readString(item, 'importance') || undefined,
    }
    const key = `${normalizeKey(category)}::${normalizeKey(title)}`
    const existing = notes.get(key)
    notes.set(key, existing ? mergeNote(existing, note) : note)
  }
  return [...notes.values()]
}

function isChapterSummaryWorldview(title: string, content: string, category: string) {
  const categoryKey = normalizeKey(category)
  const contentKey = normalizeKey(content)
  if (/(本章|章节|剧情|简介|概要|经过|第\d+章|第[一二三四五六七八九十百千万]+章)/.test(title)) {
    return true
  }
  if (['当前事件', '章节摘要', '剧情进展', '本章剧情'].some((term) => categoryKey.includes(normalizeKey(term)))) {
    return true
  }
  if (/^(本章|本节|这一章|此章|第.+章)/.test(content.trim())) {
    return true
  }
  const plotVerbs = ['搜查', '担心', '登上', '指出', '加入', '透露', '引出', '随后']
  const plotHits = plotVerbs.filter((term) => contentKey.includes(normalizeKey(term))).length
  return content.length > 80 && plotHits >= 3 && !isSettingCategory(category)
}

function isSettingCategory(category: string) {
  const key = normalizeKey(category)
  return [
    '基础规则',
    '基础设定',
    '势力制度',
    '历史传说',
    '技术魔法',
    '社会文化',
    '地理环境',
    '组织体系',
    '未确认信息',
  ].some((term) => key.includes(normalizeKey(term)))
}

function normalizeCharacters(items: unknown[]): AiBookCharacter[] {
  const characters = new Map<string, AiBookCharacter>()
  for (const item of items) {
    if (!isRecord(item) || isLowImportance(readString(item, 'importance'))) continue
    const name = readString(item, 'name')
    if (!name) continue
    const character: AiBookCharacter = {
      name,
      aliases: uniqueStrings(readStringArray(item, 'aliases')),
      status: readString(item, 'status') || readString(item, 'description') || '状态未知',
      faction: readString(item, 'faction') || undefined,
      location: readString(item, 'location') || undefined,
      description: readString(item, 'description') || undefined,
      lastSeenChapter: readString(item, 'lastSeenChapter') || undefined,
      importance: readString(item, 'importance') || undefined,
    }
    const key = normalizeKey(name)
    const existing = characters.get(key)
    characters.set(key, existing ? mergeCharacter(existing, character) : character)
  }
  return [...characters.values()]
}

function normalizeRelationships(items: unknown[]): AiBookRelationship[] {
  const relationships = new Map<string, AiBookRelationship>()
  for (const item of items) {
    if (!isRecord(item) || isLowImportance(readString(item, 'importance'))) continue
    const source = readString(item, 'source')
    const target = readString(item, 'target')
    const relation = readString(item, 'relation')
    const description = readString(item, 'description')
    const status = readString(item, 'status')
    const importance = readString(item, 'importance')
    if (!source || !target || !relation) continue
    if (normalizeKey(source) === normalizeKey(target)) continue
    if (isLowValueRelationship(relation, description || status, importance)) continue
    const relationship: AiBookRelationship = {
      source,
      target,
      relation,
      status: status || undefined,
      description: description || undefined,
      importance: importance || undefined,
    }
    const key = relationshipKey(source, target, relation)
    const existing = relationships.get(key)
    relationships.set(key, existing ? mergeRelationship(existing, relationship) : relationship)
  }
  return [...relationships.values()]
}

function normalizeLocations(items: unknown[]): AiBookLocation[] {
  const locations = new Map<string, AiBookLocation>()
  for (const item of items) {
    if (!isRecord(item)) continue
    const name = readString(item, 'name')
    if (!name) continue
    const parentName = readString(item, 'parentName')
    const location: AiBookLocation = {
      name,
      kind: readString(item, 'kind') || undefined,
      parentName: parentName && normalizeKey(parentName) !== normalizeKey(name) ? parentName : undefined,
      description: readString(item, 'description') || readString(item, 'status') || '',
      status: readString(item, 'status') || undefined,
      relatedCharacters: uniqueStrings(readStringArray(item, 'relatedCharacters')),
      firstSeenChapter: readString(item, 'firstSeenChapter') || undefined,
      importance: readString(item, 'importance') || undefined,
    }
    const key = normalizeKey(name)
    const existing = locations.get(key)
    locations.set(key, existing ? mergeLocation(existing, location) : location)
  }
  return [...locations.values()]
}

function mergeNote(current: AiBookNote, next: AiBookNote): AiBookNote {
  return {
    ...current,
    content: richerString(current.content, next.content),
    confidence: preferString(current.confidence, next.confidence),
    importance: preferImportance(current.importance, next.importance),
  }
}

function mergeCharacter(current: AiBookCharacter, next: AiBookCharacter): AiBookCharacter {
  return {
    ...current,
    aliases: uniqueStrings([...(current.aliases || []), ...(next.aliases || [])]),
    status: next.status || current.status,
    faction: next.faction || current.faction,
    location: next.location || current.location,
    description: richerString(current.description, next.description),
    lastSeenChapter: next.lastSeenChapter || current.lastSeenChapter,
    importance: preferImportance(current.importance, next.importance),
  }
}

function mergeRelationship(current: AiBookRelationship, next: AiBookRelationship): AiBookRelationship {
  return {
    ...current,
    status: next.status || current.status,
    description: richerString(current.description, next.description),
    importance: preferImportance(current.importance, next.importance),
  }
}

function mergeLocation(current: AiBookLocation, next: AiBookLocation): AiBookLocation {
  return {
    ...current,
    kind: next.kind || current.kind,
    parentName: next.parentName || current.parentName,
    description: richerString(current.description, next.description),
    status: next.status || current.status,
    relatedCharacters: uniqueStrings([...(current.relatedCharacters || []), ...(next.relatedCharacters || [])]),
    firstSeenChapter: current.firstSeenChapter || next.firstSeenChapter,
    importance: preferImportance(current.importance, next.importance),
  }
}

function isRecord(value: unknown): value is UnknownRecord {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value))
}

function readString(record: UnknownRecord, key: string) {
  const value = record[key]
  return typeof value === 'string' ? value.trim() : ''
}

function readStringArray(record: UnknownRecord, key: string) {
  const value = record[key]
  if (!Array.isArray(value)) return []
  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter(Boolean)
}

function readBoolean(record: UnknownRecord, key: string) {
  return record[key] === true
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

function normalizeKey(value: string | undefined) {
  return (value || '')
    .trim()
    .toLowerCase()
    .replace(/[·•・]/g, '.')
    .replace(/\s+/g, '')
}

function isLowImportance(value: string | undefined) {
  const normalized = normalizeKey(value || '')
  if (!normalized) return false
  return [
    'low',
    '低',
    '低重要性',
    '不重要',
    '路人',
    '背景',
    'minor',
    'background',
    'oneoff',
    '一次性',
  ].some((term) => normalized.includes(term))
}

function isLowValueRelationship(relation: string, detail: string, importance: string) {
  const normalizedImportance = normalizeKey(importance)
  if (normalizedImportance.includes('high') || normalizedImportance.includes('medium') || normalizedImportance.includes('高') || normalizedImportance.includes('中')) {
    return false
  }
  const normalizedRelation = normalizeKey(relation)
  if (!['认识', '见过', '路过', '同村', '同校', '位于', '相关'].includes(normalizedRelation)) {
    return false
  }
  return normalizeKey(detail).length < 18
}

function relationshipKey(source: string, target: string, relation: string) {
  const pair = [normalizeKey(source), normalizeKey(target)].sort().join('::')
  return `${pair}::${normalizeKey(relation)}`
}

function richerString(current: string | undefined, next: string | undefined) {
  if (!current) return next || ''
  if (!next) return current
  return next.length > current.length ? next : current
}

function preferString(current: string | undefined, next: string | undefined) {
  return current || next || undefined
}

function preferImportance(current: string | undefined, next: string | undefined) {
  return importanceRank(next) > importanceRank(current) ? next : current || next
}

function importanceRank(value: string | undefined) {
  const normalized = normalizeKey(value || '')
  if (normalized.includes('high') || normalized.includes('高')) return 3
  if (normalized.includes('medium') || normalized.includes('中')) return 2
  if (isLowImportance(value)) return 1
  return 0
}

function buildModelHeaders(apiKey: string) {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  }
  if (apiKey.trim()) {
    headers.Authorization = `Bearer ${apiKey.trim()}`
  }
  return headers
}

async function requestModelJson({
  config,
  baseUrl,
  apiKey,
  fullUrl,
  path,
  body,
  fetchImpl,
}: AiProxyRequestParams) {
  if (config.modelSource === 'server') {
    return fetchImpl('/reader3/aiProxy', {
      method: 'POST',
      headers: {
        ...buildReaderAuthHeaders(),
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        useServerConfig: true,
        kind: path === '/v1/images/generations' ? 'image' : 'text',
        path,
        body,
      }),
    })
  }

  const endpointUrl = fullUrl ? normalizeBaseUrl(baseUrl) : `${normalizeBaseUrl(baseUrl)}${path}`
  if (config.useBackendProxy) {
    return fetchImpl('/reader3/aiProxy', {
      method: 'POST',
      headers: {
        ...buildReaderAuthHeaders(),
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        baseUrl: normalizeBaseUrl(baseUrl),
        apiKey: apiKey.trim(),
        path,
        fullUrl,
        body,
      }),
    })
  }

  return fetchImpl(endpointUrl, {
    method: 'POST',
    headers: buildModelHeaders(apiKey),
    body: JSON.stringify(body),
  })
}

function normalizeBaseUrl(url: string) {
  return url.trim().replace(/\/+$/, '')
}

function buildMapImagePrompt(prompt: string) {
  const sourcePrompt = prompt.trim() || '根据已读进度中的已知地点绘制小说世界地图。'
  return [
    '请生成一张小说世界地图，而不是场景插画。',
    '画面类型：俯视地图（top-down / orthographic map）、二维制图、设定集地图。',
    '必须表现：区域边界、道路或虚线连接、地形/空间分区、地图符号、地点标签、图例、罗盘或比例尺感。',
    '地点呈现方式：机房、避难所等室内或建筑地点只能表现为地图上的标注区域、平面轮廓或小图标。',
    '禁止内容：不要生成写实照片、电影截图、建筑外观特写、室内房间透视图、服务器机柜照片、避难所入口照片。',
    '不要画人物，不要把地点画成可进入的真实建筑场景，不要用巨大门牌或数字替代地图标注。',
    '构图要求：清晰分区，路线关系可读，整体像游戏世界地图、桌面 RPG 区域地图或小说设定集地图。',
    `原始地图信息：${sourcePrompt}`,
  ].join('\n')
}

function parseJsonContent(content: string): AiBookRawModelUpdate {
  const trimmed = content.trim()
  const json = extractFirstJsonObject(trimmed)
  try {
    return JSON.parse(json) as AiBookRawModelUpdate
  } catch (error) {
    throw new Error(`AI 资料生成结果不是有效 JSON：${(error as Error).message}`)
  }
}

function extractFirstJsonObject(content: string) {
  const text = content
    .replace(/^```(?:json)?\s*/i, '')
    .trim()
  const start = text.indexOf('{')
  if (start < 0) {
    throw new Error('AI 资料生成结果未包含 JSON 对象')
  }

  let depth = 0
  let inString = false
  let escaped = false
  for (let index = start; index < text.length; index += 1) {
    const char = text[index]
    if (inString) {
      if (escaped) {
        escaped = false
      } else if (char === '\\') {
        escaped = true
      } else if (char === '"') {
        inString = false
      }
      continue
    }

    if (char === '"') {
      inString = true
    } else if (char === '{') {
      depth += 1
    } else if (char === '}') {
      depth -= 1
      if (depth === 0) {
        return text.slice(start, index + 1)
      }
    }
  }

  throw new Error('AI 资料生成结果 JSON 对象不完整')
}

async function readModelError(response: Response, fallback: string) {
  try {
    const contentType = response.headers.get('content-type') || ''
    if (contentType.includes('application/json')) {
      const data = await response.json() as {
        error?: { message?: string }
        errorMsg?: string
      }
      return data.error?.message || data.errorMsg || `${fallback} (${response.status})`
    }
    const text = await response.text()
    return summarizeHttpErrorBody(text, { fallback, status: response.status })
  } catch {
    return `${fallback} (${response.status})`
  }
}

function base64ToBlob(value: string, contentType: string) {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return new Blob([bytes], { type: contentType })
}

function isDataImageUrl(value: string | undefined): value is string {
  return Boolean(value?.trim().startsWith('data:image/'))
}

function dataUrlToBlob(value: string) {
  const match = value.match(/^data:([^;,]+)(;base64)?,(.*)$/)
  if (!match) {
    throw new Error('地图图片 data URL 无效')
  }
  const contentType = match[1] || 'image/png'
  const isBase64 = Boolean(match[2])
  const data = match[3] || ''
  if (isBase64) {
    return base64ToBlob(data, contentType)
  }
  return new Blob([decodeURIComponent(data)], { type: contentType })
}

async function fetchImageBlob(imageUrl: string, fetchImpl: typeof fetch, useBackendProxy: boolean) {
  if (!imageUrl) {
    throw new Error('地图图片地址为空')
  }
  const response = useBackendProxy
    ? await fetchImpl('/reader3/aiProxyImage', {
      method: 'POST',
      headers: {
        ...buildReaderAuthHeaders(),
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ url: imageUrl }),
    })
    : await fetchImpl(imageUrl)
  if (!response.ok) {
    throw new Error(await readModelError(response, '地图图片下载失败'))
  }
  return response.blob()
}

function buildReaderAuthHeaders() {
  const headers: Record<string, string> = {}
  const token = safeLocalStorageGet('accessToken')
  if (token) {
    headers.Authorization = token
  }
  return headers
}

function safeLocalStorageGet(key: string) {
  try {
    return localStorage.getItem(key) || ''
  } catch {
    return ''
  }
}
