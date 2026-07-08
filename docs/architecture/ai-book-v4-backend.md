# AI Book V4 Backend Architecture

本文档给后续 agent 快速理解 AI 资料 V4 后端链路使用。它描述的是当前代码里的实际边界和约束，不是理想化草图。

## 一屏结论

V4 后端按五个硬边界组织：

| Layer | 中文职责 | 输入 | 输出 | 能写什么 |
| --- | --- | --- | --- | --- |
| 1. AI Output Adapter | 把模型原始输出变成 typed observation | raw AI output / prompt context | `Observation` | 不写 DB |
| 2. Claim Ledger | 记录模型说过什么和证据来自哪里 | `Observation` | `claims` / source spans / ai run provenance | 只写 ledger / provenance |
| 3. Domain Decision / Command Materializer | 判断 claim 是否可写，并把可写内容变成命令 | `ClaimRecord` + resolver/gate/judge | `Invalid` / `NoWrite` / `Quarantine` / `Write(Command)` | 可读 canonical，可更新 claim lifecycle，不写 canonical |
| 4. Domain Reducer | 执行 canonical 写入 | typed `*WriteCommand` | reduction result | 唯一 canonical writer |
| 5. Projection | 给前端组装 view model/cache | canonical tables | view model / cache | 只写 `view_model_cache` |

核心原则：

- Judge 已经内收进 Domain Decision，不是独立架构层。
- Decision 对外只能输出 `Invalid` / `NoWrite` / `Quarantine` / `Write(DomainWriteCommand)`。
- Reducer 只能吃 typed command，不应依赖 judge raw output 或 reducer-critical `claim.value_json`。
- Projection 只能做展示 reshape、排序、cache，不修 canonical 事实。
- CorrectionApplier 不允许绕过 reducer 直接做正向 canonical mutation。

## 端到端链路

入口在 `src/service/v4/pipeline.rs` 的 `process_chapter` / `process_chapter_with_all_judges`。

当前主流程：

1. upsert chapter，检查 idempotency，创建 `chapter_processing_run`。
2. 切 segment，写 `source_spans`。
3. 为 segment 创建 `ai_run`，构造上下文。
4. 调 `Extractor::extract` 得到 typed observations。
5. summary 走 `summary_processor` side path，不进入 claim ledger。
6. `claim_writer::write_claims` 把 observation 写成 proposed claims。
7. Pipeline 按 `claim_type` 分 lane，并调用各 domain processor：
   - Character / property
   - Identity
   - Relationship
   - Knowledge
   - Place / map
8. Domain processor 负责 resolver / gate / judge / decision / reducer apply / claim lifecycle。
9. Pipeline invalidate + rebuild projection cache。
10. 标记 processing run success，推进 `processing_progress`。

Pipeline 允许做 lane split 和编排顺序，但不能做 canonical 判断。

## GitNexus Verification Map

最近一次用 GitNexus 校验 V4 backend 边界时，索引显示以下关键符号仍是当前链路里的实际锚点：

| 边界/约束 | GitNexus 命中的符号 | 文件 | 说明 |
| --- | --- | --- | --- |
| Pipeline orchestration | `process_chapter` / `process_chapter_with_all_judges` | `src/service/v4/pipeline.rs` | 章节级编排入口。负责 segment、ai_run、claim ledger、domain lane dispatch、projection cache rebuild。 |
| Relationship sample processor | `process_relationship_claims_for_segment` | `src/service/v4/relationship_processor.rs` | Relationship 样板线：resolve/gate/judge/decision/reducer/lifecycle。 |
| Relationship lifecycle | `apply_relationship_decision_lifecycle` | `src/service/v4/relationship_processor.rs` | 把 `RelationshipDecision` 四态转成 claim lifecycle 和 reducer apply。 |
| Character lifecycle | `apply_character_decision_lifecycle` | `src/service/v4/character_processor.rs` | Character / property 的 decision lifecycle。 |
| Identity lifecycle | `apply_identity_decision_lifecycle` | `src/service/v4/identity_processor.rs` | Identity 的 decision lifecycle；包含 ledger enrichment 例外。 |
| Knowledge lifecycle | `apply_knowledge_decision_lifecycle` | `src/service/v4/knowledge_processor.rs` | Knowledge 的 decision lifecycle。 |
| Place lifecycle | `apply_place_decision_lifecycle` | `src/service/v4/place_processor.rs` | Place/map 的 decision lifecycle。 |
| Quarantine proof | `identity_quarantine_decision_marks_claim_quarantined` / `knowledge_quarantine_decision_marks_claim_quarantined` / `place_quarantine_decision_marks_claim_quarantined` | `src/service/v4/*_processor.rs` | Quarantine 分支应标记 `quarantined`，不是 `uncertain`。 |
| Correction boundary proof | `correction_applier_does_not_embed_direct_canonical_status_update_sql` | `src/service/v4/correction_applier.rs` | CorrectionApplier 不应内嵌 direct canonical status update SQL，应通过 reducer helper。 |
| Projection API flow | `get_v4_character_card` -> `project_character_card` | `src/api/handlers/v4.rs` / `src/service/v4/projection.rs` | API handler 消费 projection view model，不应修 canonical。 |
| Relationship projection flow | `project_relationship_edges_for_chapter` | `src/service/v4/relationship_projection.rs` | 章节关系 view model 组装入口之一；只展示 canonical relationship/events。 |

如果这些符号变动，优先重新跑 GitNexus `query` / `context` / `detect_changes`，再更新本文档。不要只靠文件名推断边界仍然成立。

## Layer 1: AI Output Adapter

主要文件：

- `src/service/v4/extractor.rs`

职责：

- 把 raw AI output parse 成 typed `Observation`。
- 做 schema/enum/shape 级解析和轻量输入防御。
- 把模型字段映射成 source observation 的原始语义。

禁止：

- 不写 DB。
- 不决定 canonical 是否可写。
- 不做 resolver / gate / judge。
- 不把 judge-normalized 字段塞进 observation/ledger payload。

代码证据：

- `Extractor::extract` 返回 observations。
- `relationship_decision.rs` 里有 `ledger_relationship_claim_rejects_judge_normalized_fields`，防止 `normalized_relation_group` / `judge_confidence` 等字段污染 ledger payload。
- `knowledge_decision.rs` / `place_decision.rs` 也拒绝 `judge_decision`、`card_action`、`assertion_status`、`normalized_edge_type` 等 decision/judge 字段进入 ledger claim。

## Layer 2: Claim Ledger

主要文件：

- `src/service/v4/claim_writer.rs`
- `src/storage/db/v4/claim_repo.rs`
- `src/storage/db/v4/ai_run_repo.rs`

职责：

- 把 observations 写成 claim ledger。
- 记录 `book_id`、`chapter_index`、`claim_type`、mention、predicate、`value_text`、`value_json`、source span、ai run、confidence、risk。
- 所有 domain claim 初始状态都是 `proposed`。
- `Summary` 不建 claim，走 summary side path。
- `MinorEvent` 可作为 ledger-only claim 保留，但不要求进入 canonical。

允许：

- `classify_risk(obs)` 作为 ledger metadata。
- 写 `claim_source_spans`。

禁止：

- 不做 domain routing 决策。
- 不做 risk routing。
- 不决定 canonical intent。
- 不根据 observation kind 直接写 canonical。

注意：

- Pipeline 会按 `claim_type` 分 lane，这属于 orchestration，不属于 ClaimWriter。
- `identity_processor.rs` 会在 identity reference resolve 后更新 claim 的 `subject_entity_id` / `object_entity_id`。这是 ledger enrichment 例外，不是 canonical write。

## Layer 3: Domain Decision / Command Materializer

主要文件：

- `src/service/v4/character_decision.rs`
- `src/service/v4/relationship_decision.rs`
- `src/service/v4/identity_decision.rs`
- `src/service/v4/knowledge_decision.rs`
- `src/service/v4/place_decision.rs`
- Processor:
  - `character_processor.rs`
  - `relationship_processor.rs`
  - `identity_processor.rs`
  - `knowledge_processor.rs`
  - `place_processor.rs`

职责：

- 从 `ClaimRecord` 建立 domain-specific ledger claim。
- 解析 ledger payload，但只能用于 decision/materialization。
- 运行 resolver、structural gate、domain judge。
- 把 judge output 收敛成外部四态：
  - `Invalid`
  - `NoWrite`
  - `Quarantine`
  - `Write(DomainWriteCommand)`
- Processor 负责把 decision 结果映射到 claim lifecycle。

禁止：

- Judge raw output 不得传给 reducer。
- Decision 不直接写 canonical。
- NoWrite / Invalid / Quarantine 不得写 canonical。
- 不把 judge output 回写到 `claim.value_json`。

当前 domain 状态：

| Domain | 当前状态 | 说明 |
| --- | --- | --- |
| Character / property | PASS | `CharacterDecision` 四态；`Write(CharacterWriteCommand)` 后由 reducer 写 entities/aliases/properties。 |
| Relationship | PASS | `RelationshipDecision` 四态；judge output 在 decision 内转成 `RelationshipWriteCommand`。 |
| Identity | PASS with ledger enrichment | `IdentityDecision` 四态；processor 会补 claim subject/object entity id，这是 ledger enrichment 例外。 |
| Knowledge | PASS with in-memory enrichment | `KnowledgeDecision` 四态；`resolve_knowledge_claim_references` 会在内存 clone 上补 `resolved_entity_id`，不持久化回 claim。 |
| Place / map | PASS with gate adapter | `PlaceDecision` 四态；place processor 会构造 gate 所需的 resolved claim/value_json，这是 decision/gate 输入适配，不应扩散到 reducer。 |

Claim lifecycle helper 在 `src/service/v4/claim_lifecycle.rs`：

- `accept_claim`
- `reject_claim`
- `mark_uncertain_claim`
- `quarantine_claim`
- `redirect_claim`

`quarantine_claim` 会把 claim status 设为 `quarantined`，并 upsert `quarantined_claims` workflow。

## Layer 4: Domain Reducer

主要文件：

- `src/service/v4/reducer.rs`
- `src/service/v4/place_reducer.rs`

职责：

- 接收 typed write command。
- 写 canonical tables。
- 返回 reduction result，让 processor 决定 claim lifecycle。
- 维护 canonical 写入时需要的 cache invalidation。

核心 command：

- `CharacterWriteCommand`
  - `IntroduceEntity`
  - `AddAlias`
  - `UpdateProperty`
- `RelationshipWriteCommand`
- `RelationshipStatusWriteCommand`
- `IdentityWriteCommand`
  - `Merge`
  - `Link`
- `KnowledgeWriteCommand`
- `KnowledgeAssertionStatusWriteCommand`
- `PlaceWriteCommand`
  - `UpsertPlace`
  - `UpsertEdge`
  - `RecordConflict`
- `PlaceEdgeStatusWriteCommand`

禁止：

- Reducer 不应重新 parse judge raw string。
- Reducer 不应从 `claim.value_json` 读取 reducer-critical 字段。
- Reducer 不负责把 claim 标成 accepted/rejected/quarantined。

当前测试意图：

- `reducer.rs` 有 `*_reducer_accepts_typed_command_without_claim_value_json` 类测试，证明 reducer 可以只靠 typed command 工作。
- `rel_reducer_reports_accepted_claim_without_mutating_ledger_status` 证明 relationship reducer 返回 accepted claim，但不直接改 ledger status。
- `place_reducer.rs` 有 place typed command without `claim.value_json` 测试。

## Layer 5: Projection

主要文件：

- `src/service/v4/projection.rs`
- `src/service/v4/relationship_projection.rs`
- `src/service/v4/knowledge_projection.rs`
- `src/service/v4/place_projection.rs`

职责：

- canonical tables -> frontend view model。
- cache read/write。
- 展示排序、字段 reshape、视图过滤。

允许：

- sort。
- score clamp 到展示安全范围。
- 组装 node/edge/card/list。
- cache invalidation/rebuild。

禁止：

- 不修 canonical 事实。
- 不合并 canonical identity。
- 不把 missing fact 补成另一个 fact。
- 不做 relationship inverse/family semantic merge 这类事实层操作。

当前残留风险：

- `projection.rs` / `relationship_projection.rs` 仍有 `preferred_character_name`。它会把泛化角色名优先展示为更具体 alias。这是展示层美化，不是 canonical repair，但容易掩盖上游命名质量问题。
- `relationship_projection.rs` 的 `sort_relationship_edges` 和 `build_edge_for_relationship` 会做展示排序、event_count、latest source claim 组装；这属于 projection。
- `relationship_projection.rs` 当前测试强调“preserves canonical family edges”，说明 family/inverse dedupe 不应再由 projection 做事实合并。
- `projection.rs` 和 `relationship_projection.rs` 测试里存在 `UPDATE entities SET status = 'merged'` fixture，用来构造 merged entity 场景。不要把这种测试 fixture 当成 projection 可写 canonical 的模式。

## Domain Lanes

V4 当前不是按“relationship/identity/knowledge/place 是架构层”划分，而是按五层架构穿过多个 domain lane。

| Lane | 相关 claim_type | 说明 |
| --- | --- | --- |
| Character / property | `entity_introduction` / `alias` / `property_update` | 角色实体、别名、当前状态、境界、技能等属性。 |
| Identity | `identity_reveal` / `entity_merge_candidate` / `entity_split_candidate` / `not_same_identity` | 身份关系、合并、排除同一人。Identity 是跨 domain 的 canonical identity 能力，但当前作为独立 lane 处理。 |
| Relationship | `relationship_update` | 人物之间的长期显著关系。人物-地点/物品/技能等会被 gate/decision redirect 或 no-write。 |
| Knowledge | `knowledge_assertion` | 世界观、规则、背景事实、主题化知识卡。 |
| Place / map | `location_introduction` / `location_edge` | 地点实体、地理/层级边、地图冲突。 |
| Ledger-only | `minor_event` 等 | 只保留证据，不进入 canonical。 |

Identity 为什么看起来和其他类别并列：

- 架构上它不是第六层，只是一条 domain lane。
- 业务上它会影响其他 lane 的 canonical 引用，所以 reducer 会做 redirect、merge、relationship/event migration、knowledge entity remap 等 canonical 维护。
- Layer 1/2 仍然照常 parse 和记录 identity claims。

## Human Correction Path

主要文件：

- `src/service/v4/correction.rs`
- `src/service/v4/correction_applier.rs`
- `src/service/v4/quarantine_workflow.rs`

约束：

- `reject_claim` 可以只改 claim status。
- `rebuild_projection` 只能 invalidate cache。
- 正向 canonical correction 不能在 `CorrectionApplier` 里直接 UPDATE canonical tables。
- 目前允许的 canonical status correction 会路由到 reducer helper：
  - `apply_relationship_status_write_with_conn`
  - `apply_knowledge_assertion_status_write_with_conn`
  - `apply_place_edge_status_write_with_conn`
- 未实现 reducer route 的正向 correction 会失败，错误为 `positive canonical correction requires domain reducer implementation`。

这意味着人工链路也要遵守五层边界：人工决定可以创建 correction intent，但 canonical mutation 仍要经过 reducer。

## 常见反模式

不要这样做：

- 在 extractor/parser 中直接输出 `judge_decision`、`normalized_relation_group`、`assertion_status`。
- 在 claim writer 中按 risk 或 claim_type 决定是否写 canonical。
- 在 pipeline 中直接 `update_claim_status` 或直接写 canonical。
- 在 reducer 中读取 `claim.value_json` 来决定要写什么。
- 在 projection 中“顺手”修复 identity redirect、relationship inverse、knowledge contradiction、place conflict。
- 在 CorrectionApplier 中直接 `UPDATE relationships/entities/knowledge_assertions/place_edges` 完成正向修正。

应该这样做：

- 新字段先进入 observation/ledger，保持 source observation 语义。
- 可写判断放到 domain decision。
- judge raw output 在 decision 内转成 typed command。
- reducer 只处理 command。
- projection 只展示 canonical。
- 新人工修正路径先定义 correction command，再接到 reducer command。

## 编辑 V4 后端前的 Agent Checklist

1. 先确认自己改的是哪一层：Adapter / Ledger / Decision / Reducer / Projection。
2. 如果要改函数或类型，先按项目规则跑 GitNexus impact analysis。
3. 行为改动先写 RED proof，再实现，再跑 focused tests。
4. 不碰 V3。
5. 不把 judge 重新拆成独立架构边界。
6. 不新增大型 workflow engine 或 ClaimDecisionRouter，除非用户明确要求。
7. 如果必须 breaking change，V4 当前未上线，优先保持边界干净，不为旧 V4 内部兼容牺牲结构。
8. 完成后至少跑：
   - focused test
   - `cargo test v4 --lib` 或更大范围的后端测试
   - `cargo build`
   - GitNexus `detect_changes`

## 快速找代码

| 你想看 | 文件 |
| --- | --- |
| 主编排 | `src/service/v4/pipeline.rs` |
| 模型输出解析 | `src/service/v4/extractor.rs` |
| claim ledger 写入 | `src/service/v4/claim_writer.rs` |
| claim lifecycle | `src/service/v4/claim_lifecycle.rs` |
| relationship decision 样板 | `src/service/v4/relationship_decision.rs` |
| relationship processor 样板 | `src/service/v4/relationship_processor.rs` |
| character decision | `src/service/v4/character_decision.rs` |
| identity decision | `src/service/v4/identity_decision.rs` |
| knowledge decision | `src/service/v4/knowledge_decision.rs` |
| place decision | `src/service/v4/place_decision.rs` |
| main reducers | `src/service/v4/reducer.rs` |
| place reducer | `src/service/v4/place_reducer.rs` |
| projection | `src/service/v4/projection.rs` / `relationship_projection.rs` / `knowledge_projection.rs` / `place_projection.rs` |
| correction apply | `src/service/v4/correction_applier.rs` |

## 当前结论

Backend V4 boundary convergence 当前可以按 `PASS with documented residual risks` 理解：

- 五层硬边界已经在主要链路落地。
- Relationship 已经是完整样板线。
- Character / Identity / Knowledge / Place 已按同样模式收敛。
- 旧 bridge 思路不应再扩展。
- 主要剩余风险不在“缺少 normalize”，而在少数展示层美化、decision 阶段 enrichment、测试 fixture 可能被未来 agent 误读为生产模式。

后续优化的方向不是增加更多层，而是把每个 domain 的输入/输出 typed contract 写得更硬，并逐步清理 projection 里的展示型语义补偿。
