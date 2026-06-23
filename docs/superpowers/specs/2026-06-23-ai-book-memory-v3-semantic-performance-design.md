# AI Book Memory V3 Semantic + Performance Design

## Context

Reader 的 AI资料 V2 把角色、地点、世界观、关系都存在一个 JSON memory 中。最近在《没钱修什么仙》的测试里，`relationships` 出现了大量不该进入“人物关系”的条目，例如“赵天行身处学校大屏幕”、“赵天行修炼健体 36 式”等。这不是单个 prompt 词没写好，而是现有设计把“关系”当成了自由三元组，缺少领域边界。

同一条补齐链路还有性能问题：后台补齐每章串行调用模型，每次把完整 `currentMemory` 和最多 12000 字符章节正文一起发给模型，且输出上限设到 8192 token。memory 越大，后续章节越慢，脏关系还会被反复塞回 prompt，造成质量和速度双重退化。

## Goals

1. 让“关系”页只展示用户直觉中的重要人物关系。
2. 把人物状态、技能/功法、地点归属、世界设定从人物关系中拆出来。
3. 统一浏览器生成和后台补齐的语义模型，避免两套规则漂移。
4. 为 V2 脏数据提供确定性迁移和清洗，不要求用户手动重置。
5. 让章节补齐耗时不随完整 memory 线性增长。
6. 为补齐任务增加可观测性能数据，后续优化有证据。

## Non-goals

- 不引入图数据库。
- 不重做整页视觉设计。
- 不做多模型调度平台。
- 第一版不做并发写同一本书；避免章节上下文竞争。

## Recommended Approach

采用 V3 语义分层模型，而不是在 V2 上继续堆过滤词。

V3 将 AI资料拆成明确领域对象：

- `characters`: 角色身份和稳定描述。
- `characterStates`: 角色当前状态、地点、阵营、技能/功法进度、短期行动状态。
- `characterRelations`: 只存人物-人物之间长期、重要、会影响剧情理解的关系。
- `knowledgeFacts`: 世界规则、制度、功法、资源、社会设定。
- `locations`: 地点实体。
- `locationEdges`: 地点包含、路线、邻接等地图结构。
- `chapterDigests`: 每章短摘要和关键事件。
- `droppedFacts`: 被清洗/重定向的旧脏条目，默认不展示，仅用于排查。
- `catchupStats`: 补齐性能统计。

## Relationship Semantics

`characterRelations` 是窄表，不是万能边。

允许的关系类型：

- `family`: 亲属。
- `mentor`: 师生、指导。
- `peer`: 同伴、同学、队友，必须有持续互动价值。
- `ally`: 合作、盟友。
- `rival`: 竞争。
- `enemy`: 敌对。
- `debt`: 借贷、债务、经济控制。
- `authority`: 上下级、管理、管束。
- `protector`: 保护、庇护。
- `manipulates`: 利用、诱导、控制。
- `suspicious_of`: 怀疑、追踪、调查。
- `transaction`: 明确交易关系。
- `other_significant`: 无法归类但持续影响剧情的人物关系。

禁止进入 `characterRelations`：

- 人物-地点：身处、位于、就读、住在、进入、出现在。
- 人物-功法/技能/概念：修炼、学习、掌握、使用。
- 人物-物品：拥有、拿着、购买。
- 地点-地点：包含、相邻、路线。
- 一次性动作：看到、听说、路过、站在、说话。
- 低价值熟人：单纯认识、同校、同村、同屏出现。

这些信息的归属：

| 输入语义 | V3 归属 |
| --- | --- |
| 张羽修炼健体三十六式 | `characterStates.abilities` + `knowledgeFacts` |
| 张羽就读嵩阳高中 | `characterStates.affiliations` 或 `currentLocationId` |
| 赵天行出现在学校大屏幕 | 通常丢弃；若持续重要，写入 `characterStates.status` |
| 嵩阳高中包含法力教室 | `locationEdges(kind=contains)` |
| 张羽和白真真有借贷互助 | `characterRelations(kind=debt/transaction/peer)` |
| 苏海峰用补助计划诱导张羽签债务合同 | `characterRelations(kind=manipulates/authority/debt)` |

## V3 Data Shape

```ts
interface AiBookMemoryV3 {
  schemaVersion: 3
  bookUrl: string
  bookName?: string
  author?: string
  enabled: boolean
  processedChapterIndex?: number
  processedChapterTitle?: string
  updatedAt: number
  summary: {
    current: string
    recentChanges: string[]
    openQuestions: string[]
  }
  chapterDigests: AiBookChapterDigest[]
  characters: AiBookCharacterV3[]
  characterStates: AiBookCharacterStateV3[]
  characterRelations: AiBookCharacterRelationV3[]
  knowledgeFacts: AiBookKnowledgeFactV3[]
  locations: AiBookLocationV3[]
  locationEdges: AiBookLocationEdgeV3[]
  mapState: AiBookMapState
  renderArtifacts: AiBookRenderArtifacts
  droppedFacts: AiBookDroppedFact[]
  catchupStats?: AiBookCatchupStats
  lastError?: string
  lastErrorChapterIndex?: number
  lastErrorChapterTitle?: string
}

interface AiBookCharacterRelationV3 {
  id: string
  sourceCharacterId: string
  targetCharacterId: string
  kind: 'family' | 'mentor' | 'peer' | 'ally' | 'rival' | 'enemy' | 'debt' | 'authority' | 'protector' | 'manipulates' | 'suspicious_of' | 'transaction' | 'other_significant'
  label: string
  direction: 'directed' | 'undirected'
  status?: string
  description?: string
  importance: 'high' | 'medium'
  firstSeenChapterIndex?: number
  lastSeenChapterIndex?: number
  evidence: AiBookEvidence[]
}

interface AiBookCharacterStateV3 {
  characterId: string
  currentStatus?: string
  currentLocationId?: string
  affiliations: string[]
  abilities: Array<{
    name: string
    level?: string
    status?: string
    knowledgeFactId?: string
    evidence: AiBookEvidence[]
  }>
  resources: string[]
  lastSeenChapterIndex?: number
  evidence: AiBookEvidence[]
}

interface AiBookKnowledgeFactV3 {
  id: string
  category: '基础规则' | '势力制度' | '历史传说' | '技术/魔法' | '社会文化' | '地理环境' | '组织体系' | '资源经济' | '未确认信息'
  title: string
  content: string
  confidence: '已知' | '推断' | '未知'
  importance: 'high' | 'medium'
  firstSeenChapterIndex?: number
  lastConfirmedChapterIndex?: number
  evidence: AiBookEvidence[]
}

interface AiBookLocationEdgeV3 {
  id: string
  sourceLocationId: string
  targetLocationId: string
  kind: 'contains' | 'route' | 'adjacent'
  label?: string
  evidence: AiBookEvidence[]
}

interface AiBookDroppedFact {
  source: 'migration' | 'model_patch' | 'manual_save'
  reason: string
  originalKind: string
  originalValue: unknown
  chapterIndex?: number
  createdAt: number
}
```

## Generation Contract

所有新生成统一使用 `KnowledgePatchV3`。浏览器生成和后台补齐都走同一套 schema、同一套归一化规则。

```ts
interface KnowledgePatchV3 {
  chapterDigest: {
    chapterIndex: number
    chapterTitle: string
    digest: string
    keyEvents: string[]
  }
  summary?: Partial<AiBookMemoryV3['summary']>
  characters?: Partial<AiBookCharacterV3>[]
  characterStates?: Partial<AiBookCharacterStateV3>[]
  characterRelations?: Partial<AiBookCharacterRelationV3>[]
  knowledgeFacts?: Partial<AiBookKnowledgeFactV3>[]
  locations?: Partial<AiBookLocationV3>[]
  locationEdges?: Partial<AiBookLocationEdgeV3>[]
}
```

Prompt 必须明确：

- `characterRelations` 只输出人物-人物重要关系。
- 动作、位置、技能、功法、物品、地点层级不得写进 `characterRelations`。
- 无重要变化时允许只输出 `chapterDigest` 和空 patch。
- 每章每类最多输出有限条目：关系最多 3 条，知识设定最多 5 条，角色状态最多 5 条。
- 输出只包含新增或变化，不回传完整 memory。

## Normalization Pipeline

新增共享语义归一化层：

1. `normalizeKnowledgePatchV3(patch, context)`
   - 修正字段别名。
   - 填充稳定 id。
   - 丢弃空证据、空名称、低重要性条目。

2. `classifyRelationCandidate(candidate)`
   - 人物-人物且 relation kind 合法：保留为 `characterRelations`。
   - 人物-地点：转为 `characterStates.currentLocationId` 或丢弃。
   - 人物-概念/技能：转为 `characterStates.abilities` 或 `knowledgeFacts`。
   - 地点-地点：转为 `locationEdges`。
   - 一次性动作：写入 `droppedFacts`。

3. `mergeAiBookMemoryV3(previous, normalizedPatch)`
   - 按稳定 id 合并。
   - 同一人物对的相近关系合并为更高价值 label。
   - evidence 去重并限制数量。
   - 更新 `chapterDigests` 和 `summary`。

4. `selectAiBookDisplayMemory(memory)`
   - 关系页只读取 `characterRelations`。
   - 角色页组合 `characters + characterStates`。
   - 设定页读取 `knowledgeFacts`。
   - 地图页读取 `locations + locationEdges`。

## Migration Strategy

读取 memory 时执行惰性迁移，保存时落 V3。提供显式清洗入口用于已有书籍。

V2 到 V3：

- `worldFacts` / `worldview` → `knowledgeFacts`。
- `characters` → `characters` + `characterStates`。
- `locations` → `locations`。
- `mapState.edges` 中包含边 → `locationEdges`。
- `relationships`：
  - `targetKind=character` 且语义重要 → `characterRelations`。
  - `targetKind=location` 或 relation 为 身处/位于/就读/住在 → `characterStates` 或 `droppedFacts`。
  - relation 为 修炼/掌握/学习/使用 → `characterStates.abilities` + `knowledgeFacts`。
  - `targetKind=concept` → `knowledgeFacts` 或 ability。
  - 其他非法项 → `droppedFacts`。

迁移必须保留原始 evidence。无法安全迁移的条目不展示，但进入 `droppedFacts`。

## Performance Design

### Working Context

每章模型调用不再传完整 memory，而是传 `AiBookWorkingContext`：

```ts
interface AiBookWorkingContext {
  bookName?: string
  author?: string
  summaryCurrent: string
  recentChapterDigests: AiBookChapterDigest[]
  relevantCharacters: Array<{ id: string; name: string; status?: string; aliases?: string[] }>
  relevantRelations: Array<{ source: string; target: string; kind: string; label: string }>
  relevantKnowledgeFacts: Array<{ id: string; title: string; category: string; content: string }>
  relevantLocations: Array<{ id: string; name: string; parentName?: string; kind?: string }>
  schemaHint: 'KnowledgePatchV3'
}
```

选择规则：

- 最近 8 个 `chapterDigests`。
- 当前章节正文中提到的角色/地点/设定。
- high importance 条目优先。
- 每类设置硬上限，保证 prompt 稳定。

### Two-stage Catch-up

后台补齐改为两阶段：

1. Digest stage
   - 输入：章节标题和正文。
   - 输出：短 digest、关键事件、提到的角色/地点/概念。
   - token 上限：1024。

2. Patch stage
   - 输入：章节 digest、正文片段、working context。
   - 仅当 digest stage 判断有重要变化时调用。
   - 输出：`KnowledgePatchV3`。
   - token 上限：2048-3072。

水章或无重要变化章节只保存 digest，不跑 patch stage。

### Batch Window

旧章节补齐支持窗口模式：

- 默认 `windowSize=3`。
- 每 3 章先生成 digest batch。
- 若窗口内有重要变化，再生成一次 patch。
- 单章超过正文长度上限或实体过密时自动降到 `windowSize=1`。

第一版不做并发，保持章节顺序一致性。

### Token and Payload Limits

- 章节正文传入模型前从 12000 字符降为动态裁剪：默认 8000 字符。
- digest stage 使用完整前 8000 字符。
- patch stage 使用：digest + 与提到实体附近的片段 + working context。
- Responses/Gemini/Anthropic 输出上限按阶段区分，不再统一 8192。

### Observability

`catchupStats` 记录：

```ts
interface AiBookCatchupStats {
  totalModelCalls: number
  digestCalls: number
  patchCalls: number
  skippedPatchChapters: number
  totalInputBytes: number
  totalOutputBytes: number
  lastCallLatencyMs?: number
  averageCallLatencyMs?: number
  lastChapterIndex?: number
  updatedAt: number
}
```

任务状态接口返回最近统计，UI 显示“已跳过 patch N 章 / 平均耗时 X 秒”。

## Backend Responsibilities

- `src/service/ai_book_service.rs`
  - 接受 V3。
  - 保存前调用 V3 validator。
  - 惰性迁移 V1/V2 到 V3。

- `src/service/ai_book_catchup_service.rs`
  - 使用 digest/patch 两阶段。
  - 使用 working context，而不是完整 memory。
  - 记录 catchupStats。
  - 继续支持取消任务。

- 新增语义模块，例如 `src/service/ai_book_memory_v3.rs`
  - V3 类型无须先强 Rust struct 化完整 JSON；第一版可用 serde_json + 小型 helper，避免大重构。
  - 包含 normalize、migrate、validate、select working context。

## Frontend Responsibilities

- `frontend/src/types/index.ts`
  - 增加 V3 类型和 patch 类型。

- `frontend/src/utils/aiBookV3.ts`
  - V3 merge/migration/display selectors。
  - 与后端规则保持同名测试 fixture。

- `frontend/src/utils/aiBookGeneration.ts`
  - browser 模式输出 `KnowledgePatchV3`。
  - 不再让模型直接输出 legacy relationships。

- `frontend/src/views/AiBookView.vue`
  - 关系页只展示人物关系。
  - 角色卡增加能力/地点/阵营摘要。
  - 设置页可显示清洗统计，不展示 `droppedFacts` 详细内容，除非 debug 模式。

## Testing Strategy

### Fixture

建立《没钱修什么仙》小 fixture，至少包含：

- 张羽修炼健体三十六式。
- 张羽就读嵩阳高中。
- 赵天行怀疑/跟踪张羽。
- 白真真与张羽存在借贷互助。
- 苏海峰以老师身份诱导张羽签债务合同。
- 学校大屏幕出现人物名字。

### Required Assertions

- “张羽 修炼 健体三十六式” 不出现在关系页。
- “张羽 健体三十六式” 进入 ability 或 knowledgeFacts。
- “张羽 就读 嵩阳高中” 不出现在关系页。
- “张羽 白真真 借贷/互助” 出现在关系页。
- “苏海峰 张羽 诱导/债务/师生权力” 出现在关系页。
- “赵天行 学校大屏幕” 被丢弃或变成非关系状态，不展示在关系页。
- V2 脏 relationships 迁移后不会污染 `characterRelations`。
- working context 大小在 memory 增长后仍有上限。
- digest-only 章节不会调用 patch stage。

### Commands

- Rust focused tests: `cargo test ai_book`
- Frontend focused tests: `cd frontend && npm test -- aiBook`
- Sanity: `git diff --check`

## Rollout Plan

1. Add V3 types, normalizer, migration tests.
2. Add display selectors and UI compatibility, still allow reading V2.
3. Switch browser generation to V3 patch.
4. Switch backend catchup prompt to V3 patch and working context.
5. Add digest/patch two-stage processing and stats.
6. Add one-time cleanup action for existing memories.
7. Remove or hide legacy relationship display once migration tests pass.

## Risks

- Migration can accidentally hide useful relationship data. Mitigation: store hidden data in `droppedFacts` and test with real fixture.
- Two-stage calls can double model calls for important chapters. Mitigation: digest stage is small; patch only runs when needed.
- Browser and backend rules can drift. Mitigation: shared fixture names and mirrored tests.
- V3 schema could grow too broad. Mitigation: first implementation keeps relation ontology small and avoids graph database abstractions.

## Success Criteria

- In the test fixture, relationship UI shows only meaningful人物-人物关系。
- Skills, locations, school affiliation, and transient appearances no longer appear as relationship cards.
- Existing V2 memory opens without reset and migrates deterministically.
- A long catch-up task stops sending full memory every chapter.
- Catch-up status exposes model call counts, skipped patch chapters, and average latency.
- Focused Rust and frontend tests pass.
