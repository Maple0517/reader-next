# AI Book Memory V4 Design

> **V4 是 clean-slate redesign。** V4 不 patch Legacy JSON Memory，不迁移 Legacy JSON，不兼容 Legacy response shape。V4 使用新 database / new schema / new API contract。

## 1. Design Goals

将当前基于单 JSON blob 的 AI 资料系统重构为 claim-based、四层分离的架构。设计必须保证：

- Phase 1 建立的底座能被 Phase 2-6 完整复用，不需要推翻
- 后续阶段不得重定义 shared foundation 核心语义；允许 additive migrations（新增列、新增表）
- 所有阶段共享同一套 Claim → Canonical → Projection 的数据流
- V4 使用全新 API 契约，不兼容 Legacy response shape

### Phase 5 Freeze Update — Places and Map

Phase 5 已按 final verification report 冻结为 Product Ready：

- Backend / Frontend / Real AI smoke 均为 PASS，Phase 5 Freeze = YES，Ready for Phase 6 = YES。
- 地图系统落地为 additive extension：`place_details`、`place_edges`、`place_edge_sources`、`place_edge_conflicts`、`map_layout_snapshots`、generic `entity_links`。
- 核心事实边界已验证：AI 仍只产出 observations/claims；`place_reducer` 是唯一 canonical map writer；layout snapshot 仍是 projection，不是事实源。
- 严格缺口已补齐并有 named regression：direction inverse canonicalization、`inside/part_of -> contains` hierarchy canonicalization、existing-parent overwrite protection、real AI judge blank `conflict_type` normalization。
- 真实 AI smoke 使用 3 章 fixture，覆盖 place introduction、containment、directional edge、inverse duplicate、route edge、organization/place same-name link、distractors、conflict、hierarchy cycle；结果为 `places=5 active_edges=4 conflicts=1 edge_sources=5 false_positive_edges=0`。
- Phase 1/2/3/4 regression 均通过；Phase 5 没有重写 frozen foundation。

## 2. Architecture Overview

```text
Chapter Text
  → Segment + Source Spans (轻量切分)
  → Context Builder (精选上下文)
  → AI Extractor (observations)
  → Code Schema Validator
  → Candidate Retriever (代码召回)
  → AI Resolver (消歧 + 归一)
  → Risk Classifier (代码打分)
  → [Judge] (高风险才走)
  → Code Reducer (事务写入)
  → Projection Builder
  → View Model → Frontend
```

四层数据模型：

```text
Source Layer    chapters / chapter_segments / source_spans / ai_runs
Claim Layer     claims (事实账本，状态机驱动)
Canonical Layer entities / aliases / properties / relationships / knowledge / places
View Layer      materialized view models (character_card / relationship_graph / ...)
```

数据流向单一：Source → Claim → Canonical → View。AI 不直接写 Canonical，Reducer 是唯一入口。

## 3. Shared Foundation Design

以下设计在 Phase 1 建立，Phase 2-6 完整复用。

### 3.1 Source Span / Chapter Segment

**Source Span** 是证据引用单位。任何进入 canonical state 的事实都必须关联至少一个 source span。

**Chapter Segment** 是 AI 处理单位。每个 segment 对应一组 source spans。

切分策略（Phase 1 轻量）：

```text
- 普通章节（< 阈值字符数）：整章 = 1 segment，spans 按自然段生成
- 长章节（>= 阈值）：按自然段 / 对话块 / 句群聚合为多个 segment
- 不做复杂场景识别、不做 cross-segment retry
- 阈值可配置，建议 3000-5000 字
```

chapters.raw_text 仍保存全文。source_spans 存储该 span 的完整原文片段（text_excerpt），span 长度由切分控制，不做截断。

AI 处理时只接收：

- 当前 segment 的 source spans
- Context Builder 提供的精选上下文（候选实体、活跃人物、相关关系摘要）
- 不接收全书 source spans，不接收完整 memory

高风险 Judge（Phase 2+）可以额外读取邻近 segment 的 spans 作为判断依据。

Claim 支持关联多个 source spans（一条事实可能跨段落有多个证据）。

### 3.2 AI Runs

每次 AI 调用记录一个 ai_run：

```text
run_type:       extract / resolve / judge / map_extract / knowledge_extract
model:          实际调用的模型 ID
prompt_version: prompt 版本标记
schema_version: 输出 schema 版本
input_hash:     输入内容 hash
output_json:    完整输出
status:         success / failed / timeout / rejected
error:          失败原因
started_at / finished_at
```

用途：换模型后重跑、prompt regression 对比、追查脏数据来源、只重跑某版 prompt 影响的章节。

### 3.3 Claims

Claim 是事实账本，不是最终展示数据。Domain tables 才是 canonical state。

Claim 生命周期状态：

```text
proposed    → Extractor 产出，未经验证
accepted    → Reducer 事务成功后标记（Reducer 输入是 proposed claims，输出才标 accepted）
rejected    → 未通过验证或 Judge 拒绝
uncertain   → Extractor/Resolver 无法确定置信度，留待后续章节澄清
quarantined → 高风险 claim 未通过 judge，隔离等待 audit 或重跑
superseded  → 被后续 claim 替代（属性 replace、知识修正、reprocess 后新 claim 替代旧 claim）
contradicted → 与已有 canonical state 矛盾
reprocessed → 被 selective reprocess 标记为旧版本，canonical state 已回滚，新 claim 将重新生成
stale       → 由于 reprocess / hash / prompt / schema 变化而不再参与 canonical rebuild，保留审计
```

Claim 类型（覆盖所有阶段）：

```text
Phase 1:  entity_introduction / alias / property_update
          minor_event — 写 claims，reducer 不消费，不进入 canonical tables
          summary — 不写 claims，直接写 chapter_summaries
Phase 2:  relationship_update
Phase 3:  identity_reveal / entity_merge_candidate
Phase 4:  knowledge_assertion / knowledge_revision
Phase 5:  location_introduction / location_edge
```

Claim 关联 source span 和 ai_run：

```text
primary_source_span_id  — 主证据 source span
claim_source_spans      — 多对多，每条带 role
role: primary / supporting / contradiction / context
```

chapter_index 保留用于排序，但 evidence 以 source span 为准。

关键边界：claim 可以泛化（例如"赵天行修炼健体三十六式"），但 Reducer 决定是否进入 canonical tables 以及进入哪个表。claim 不等于最终展示。

### 3.4 Entities / Aliases

**Entities** 是统一实体表，entity_type 区分类型：

```text
character / organization / place / item / ability / realm / concept / event / unknown
```

不要一开始就设计几十种类型。后续根据真实小说数据扩展。

一个实体只能有一个 entity_type。"青云门"同时是组织和地点时，创建两个独立 entity（organization + place），用 entity_links 关联，不强迫单一 entity 承担两种语义。

每个 entity 有：canonical_name、display_name、short_summary、importance_score、first_seen_chapter、last_seen_chapter、status。

Canonical ID 由后端生成，推荐前缀 `char:` / `loc:` 等。AI 输出的 names/labels 只是输入，后端拥有最终 ID。

**Entity Aliases** 存储别名：改名、称号、伪装身份、翻译变体等不覆盖主名，都进入 alias 表。展示名由 Projection 决定。

Alias 类型：name / title / nickname / former_name / new_name / disguise / identity / honorific。

### 3.5 Property Dimensions

`property_dimensions` 是防止属性漂移的注册表。AI 不能自由创造字段，必须映射到已注册的维度。

Phase 1 内置 character dimensions：

```text
identity / life_status / affiliation / rank / occupation / realm /
ability / equipment / location / mental_state / goal / injury /
appearance / background
```

每个 dimension 定义：entity_type、dimension_key、display_name、value_type、merge_strategy、aliases_json、status、created_by。

- aliases_json：同义维度别名列表（如 realm ↔ cultivation_level ↔ 境界 ↔ 修为），Resolver 归一时使用
- status：active / deprecated，deprecated 的 dimension 不再接受新属性
- created_by：system / ai / migration，区分内置维度和 AI 建议维度

后续允许 AI 建议新 dimension，但必须经过 Resolver 归一到已有维度或显式创建新注册条目。

### 3.6 Properties / Current Properties

**entity_properties** 是属性历史表，每条记录是一个属性值的生命周期：

```text
entity_id / dimension_key / value_text / value_json
valid_from_chapter / valid_to_chapter
source_claim_id / confidence / status (active / superseded / contradicted / uncertain)
supersedes_property_id
```

**entity_current_properties** 是当前快照，每 (entity_id, dimension_key) 只有一条记录，用于快速查询当前状态。包含 value_text 和 value_json（复杂属性用 JSON 存储）。

属性更新策略（Phase 1 实现 replace + append）：

```text
replace: 旧 property.status = superseded, valid_to_chapter = current - 1
         新 property 进入 current_properties
         适用：境界、身份、位置、心理状态

append:  旧 property 保留，新 property 作为补充
         projection 聚合展示
         适用：外貌、背景、经历

timeline: (后续阶段) 职位变化、境界变化历史
set:      (后续阶段) 装备列表、能力列表
```

属性系统不能变成自由 EAV。每个 dimension 都必须在 registry 中注册。

### 3.7 Reading / Processing Progress

```text
max_read_chapter      = 用户读到的最远章节
max_processed_chapter = AI 处理到的最远章节
```

系统只能处理 chapter_index <= max_read_chapter。用户跳回旧章节不回退资料库。前端展示"资料已更新到第 N 章"。

Processing / Catchup Job 状态机：

```text
状态：idle / running / cancel_requested / failed / completed
字段：target_chapter / current_chapter / current_segment_id / last_error
```

同一本书同一时间只允许一个 running job。cancel_requested 为 cooperative cancellation，在安全点检查。

### 3.8 View Model Projection

Projection 从 Canonical tables 构建前端直接消费的 view model。

Phase 1 的 Projection 是 Character Card View：

```text
从 entities + entity_aliases + entity_current_properties 构建
包含：id、name、aliases、summary、importance、first/last_seen_chapter
current_states: { dimension_key: { label, value, updated_chapter, confidence } }
```

Projection 由 Reducer 在事务提交后触发，或由 Projection Builder 在章节处理完成后批量重建。

Phase 1 使用轻量 view_model_cache：章节处理完成后 invalidate + rebuild affected cache entries。Character Card 读取频繁，cache 比 on-read projection 更合适。后续阶段复用同一 cache 机制。

### 3.9 Chapter Processing Runs (幂等性)

同一章、同一 chapter_hash、同一 prompt_version + schema_version 已成功处理，不重复处理。

章节文本 hash 变化时，旧 run 标记 stale，允许重跑。

## 4. Shared AI Pipeline

### 4.1 Extractor

职责：从章节 segment 的 source spans 中抽取 observations。

不负责：最终入库、entity merge、relationship accept、contradiction 判断。

输出：`Vec<Observation>`，每条包含 type、subject_mention、value、evidence_span_ids、confidence。

Observation schema 是 typed union，每种 observation type 有固定的字段结构，不允许任意 JSON。具体字段定义留到 Implementation Contracts 阶段。

Extractor 规则：

- 只抽取对后续阅读有长期价值的信息
- 不抽取普通临时动作
- 不把属性当关系（"张三在青云门" → property_update，不是 relationship）
- 每条 observation 必须有 evidence span reference
- 不确定就标低 confidence 或 uncertain
- 每个阶段扩展 Extractor 的 observation 类型，不替换已有类型

#### Extractor Implementations

Phase 1 的 Extractor 只负责 character/property 相关 observation types：entity_introduction / alias / property_update / minor_event / summary。

Extractor 有两类实现：

1. **MockExtractor**
   - 用于 deterministic tests
   - 不调用真实 AI
   - 验证 pipeline / reducer / projection 行为
   - MockExtractor 通过只代表 engineering baseline

2. **RealAiExtractor**（Phase 1.1 待实现）
   - 接真实 AI provider
   - 执行真实章节抽取
   - 输出必须经过 parse / schema validation / evidence validation
   - RealAiExtractor + real novel smoke test 通过后，Phase 1 才能 product-ready

后续阶段可以扩展 Extractor observation types 和 prompt sections，但不能替换 Phase 1 的 Extractor framework、ai_run lifecycle、schema validation、ClaimWriter / Reducer 边界。

### 4.2 Candidate Retriever

代码负责召回候选实体，不做最终语义判断。

召回策略（按优先级）：

```text
1. alias exact match
2. normalized name match（runtime normalize：trim / 全角半角 / 大小写规范化，具体实现留到 Contracts）
3. recent active entities（最近 N 章出场）
4. high importance entities
5. same chapter co-occurrence
6. entity type compatibility
```

不依赖 embedding（Phase 1）。后续可加入 FTS5 或向量搜索增强召回。

### 4.3 Resolver

负责：mention 消歧到具体 entity、决定是否新建 entity、是否新增 alias、属性维度归一。

代码只做：trim / 全角半角 / 大小写规范化 / 枚举校验 / exact alias match / schema validation。

AI 负责：语义消歧（"金丹"是境界还是丹药）、新 entity 是否需要创建、property_key 是否映射到已有 dimension。

后端不维护复杂语义词典。复杂语义归一交给 AI Resolver。

### 4.4 Risk Classifier

代码根据 claim 类型和影响范围打风险分：

```text
low:    普通属性补充、alias 补充、地点首次出现
medium: 重要人物状态变化、新建重要人物、组织归属变化、地点层级关系
high:   relationship_update / identity_reveal / entity_merge_candidate /
        death / resurrection / knowledge_revision / contradiction / map directional conflict
```

Risk Classifier 是纯代码逻辑，不调用 AI。

### 4.5 Judge

只处理 high risk claim。

Phase 1 不实现 Judge。高风险 claim 标记 quarantined 或 uncertain，不进入 canonical state。

Phase 2+ 的 Judge 结构：

```text
Phase 2: Relationship Judge
  → code structural gate: 类型检查 (character-character only)、enum 校验、evidence 存在、confidence 阈值
  → AI semantic judge: 判断是否为值得记录的长期或重要人物关系

Phase 3: Identity Judge
  → 高置信 merge / 中置信 possible_identity_link / 低置信 quarantine

Phase 4: Knowledge Revision Judge
  → 判断新信息是补充还是推翻旧认知

Phase 5: Map Conflict Judge
  → 判断地点拓扑关系是否与已有事实矛盾

Phase 6: Judge 持续优化、audit 发现的误判修正
```

### 4.6 Reducer

Reducer 是唯一能改 canonical state 的模块。AI 不能直接写最终表。

输入：validated/proposed claims + resolved entity IDs + resolved dimension keys + evidence spans。

Reducer 必须在事务内提交。任意关键步骤失败，整章 canonical state 不更新（rollback）。ai_runs 和失败日志可以保留。

每个 domain reducer 只消费 validated/proposed claims，commit 成功后才标 accepted：

```text
entity_reducer:     创建/更新 entity、alias
property_reducer:   按 merge_strategy 更新 properties + current_properties
relationship_reducer: (Phase 2) 创建/更新 relationship + event
knowledge_reducer:  (Phase 4) 创建/更新 knowledge_card + assertion
place_reducer:      (Phase 5) 创建/更新 place_detail + edge
```

Projection 由 Reducer 在事务提交后触发 rebuild。

### 4.7 Context Builder

不把完整 memory 塞给 AI。每章 context 只包含：

```text
1. 当前 segment 的 source spans
2. 上一章或最近几章短摘要（来自 chapter_summaries 缓存，独立于 claims）
3. 本章 mention 命中的候选实体（由 Candidate Retriever 产出）
4. 最近活跃的重要人物
5. 与候选人物相关的当前关系
6. 相关地点/组织/世界观摘要
7. 当前 schema brief（允许的 observation types、dimension keys、relation groups）
```

章节摘要是 Context Builder 的输入，不是 claim 产物。Phase 1 使用独立的 chapter_summaries 缓存（由 Extractor 产出或独立摘要服务生成），不依赖 summary claims 进入 canonical state。

## 5. Shared Data Model

### Source Layer

```text
chapters              (book_id, chapter_index, title, raw_text, text_hash)
chapter_segments      (book_id, chapter_id, chapter_hash, segment_index, segment_type,
                       start_span_id, end_span_id, start_offset, end_offset, text_hash,
                       status)
source_spans          (book_id, chapter_id, chapter_hash, segment_id, span_index,
                       start_offset, end_offset, text_excerpt, status)
ai_runs               (book_id, chapter_id, segment_id, run_type, model, prompt_version, schema_version,
                       input_hash, output_json, status, error)
                       -- segment_id: nullable，整章处理可为空；多 segment extract 必须记录
chapter_processing_runs (book_id, chapter_index, chapter_hash, prompt_version, schema_version, status) — 幂等性
chapter_summaries     (book_id, chapter_index, summary, key_points_json, updated_at) — Context Builder 输入，独立于 claims
```

### Claim Layer

```text
claims                (book_id, chapter_index, claim_type, subject_mention, object_mention,
                       subject_entity_id, object_entity_id,
                       predicate, value_json, value_text, primary_source_span_id, ai_run_id,
                       confidence, risk_level, status, supersedes_claim_id)
                       -- chapter_index 用于排序，evidence 以 source span 为准
claim_source_spans    (claim_id, source_span_id, role)  -- role: primary / supporting / contradiction / context
```

### Canonical Layer (Phase 1 建立的 shared foundation)

```text
entities              (book_id, entity_type, canonical_name, display_name, short_summary,
                       importance_score, first_seen_chapter, last_seen_chapter, status)
entity_aliases        (book_id, entity_id, alias, alias_type, first_seen_chapter, confidence, source_claim_id)
property_dimensions   (book_id, entity_type, dimension_key, display_name, value_type,
                       merge_strategy, importance, aliases_json, status, created_by)
entity_properties     (book_id, entity_id, dimension_key, value_text, value_json,
                       valid_from_chapter, valid_to_chapter, source_claim_id, confidence, status, supersedes_property_id)
entity_current_properties (book_id, entity_id, dimension_key, property_id,
                       value_text, value_json, updated_chapter, confidence)
```

### Canonical Layer (Phase 2+ 增量，按阶段新增)

```text
Phase 2: relationships / relationship_events
Phase 3: entity_identity_links / entity_links
Phase 4: knowledge_cards / knowledge_assertions
Phase 5: place_details / place_edges / map_layout_snapshots
Phase 6: quarantined_claims / user_corrections / quality_metrics
```

entity_links 关联不同 entity_type 的同源实体（如 organization "青云门" ↔ place "青云门"）。link_type 包含：organization_place_pair / based_at / headquarters_of / redirect（entity merge 后的 ID 重定向，Projection 读取时 resolve）/ related_entity。

### Progress

```text
reading_progress      (book_id, max_read_chapter)
processing_progress   (book_id, max_processed_chapter)
```

### View Model Cache

```text
view_model_cache      (book_id, view_type, scope_id, max_chapter, payload_json, updated_at)
                       -- view_type: character_card / character_list / memory_overview / relationship_graph / knowledge / map
                       -- scope_id: book-level 用 "__book__"，entity-level 用 entity_id
```

Phase 1 view_type：`character_card` / `character_list` / `memory_overview`

Phase 1 使用轻量 cache，章节处理后 invalidate + rebuild affected entries。后续阶段复用同一 cache 机制。

## 6. Phase 1 Design: Stable Foundation + Character Cards

### 目标

用户打开资料面板，能看到人物列表和人物当前状态。

Phase 1 分为两个完成层级：
- **Engineering Baseline**：MockExtractor E2E 验证代码链路（Observation → Claim → Canonical → Projection）
- **Product Ready**：RealAiExtractor + real novel smoke test 验证真实抽取能力

### 新增能力

- Source Layer 完整建立：chapters、chapter_segments、source_spans、ai_runs、chapter_processing_runs
- Claim Layer 建立：claims、claim_source_spans
- Entity Layer 建立：entities、entity_aliases
- Property Layer 建立：property_dimensions、entity_properties、entity_current_properties
- Progress：reading_progress、processing_progress
- AI Pipeline：Extractor（人物 + 属性 observations）、Candidate Retriever、Resolver、Risk Classifier
- Reducer：entity_reducer + property_reducer
- Claim 状态处理：低风险 claim 经 Reducer 事务成功后标 accepted；高风险 claim（death / resurrection / identity_reveal / entity_merge_candidate / major life_status change）只能标 quarantined 或 uncertain，不进入 canonical state
- Projection：Character Card View Model（轻量 cache，章节处理后 invalidate + rebuild）
- Context Builder（精选上下文构建）

### 新增数据模型

即 Shared Data Model 中 Phase 1 scope 的全部表。

### 新增 AI Stage

- Extractor prompt：输出 entity_introduction / alias / property_update / minor_event / summary 类型的 observations
- Resolver prompt：消歧 mention → entity，归一 dimension_key

### 新增 Reducer

- **entity_reducer**：消费 entity_introduction / alias claims → 创建/更新 entity + alias
- **property_reducer**：消费 property_update claims → 按 merge_strategy 更新 properties + current_properties
- minor_event claims 只保留在 claims 表，reducer 不消费
- summary observation 写 chapter_summaries，不创建 claim

### 新增 Projection

- **Character Card Projection**：从 entities + aliases + current_properties 构建 view model
- Projection 在 Reducer 事务提交后触发

### 复用

无（Phase 1 建立全部 shared foundation）。

### 不做什么

- relationships / relationship_events（Phase 2）
- entity_identity_links（Phase 3）
- knowledge_cards / knowledge_assertions（Phase 4）
- place_details / place_edges（Phase 5）
- quarantined_claims 管理界面（Phase 6）—— 但 quarantined claim 状态本身存在
- AI Judge（Phase 1 不实现 Judge，但高风险 facts 必须拦截：death / resurrection / identity_reveal / entity_merge_candidate / major life_status change 只能标 quarantined 或 uncertain，不进入 canonical state）
- 复杂场景切分
- 用户纠错

### 与 Phase 2 的衔接

Phase 1 的 Extractor 不提取 relationship_update。当 Extractor 未来提取关系类 observations 时，Risk Classifier 会将其标记为 high risk，Phase 2 的 Relationship Judge 接管处理。Phase 1 的 claims 表已经预留 claim_type = relationship_update 的存储空间。

---

## 7. Phase 2 Design: Relationship System

### 目标

关系页不出现人物-技能、人物-地点、临时互动，只有干净的人物关系。

### 新增能力

- Canonical Layer：relationships、relationship_events
- AI Pipeline：Extractor 扩展（关系 observation 类型）、Relationship Judge = code structural gate + AI semantic judge
- Reducer：relationship_reducer
- Projection：Relationship Graph View Model

### 新增数据模型

```text
relationships         (book_id, subject_character_id, object_character_id,
                       relation_group, relation_label, directionality, current_state,
                       strength, polarity, confidence, importance_score,
                       first_seen_chapter, last_changed_chapter, last_seen_chapter, status)

relationship_events   (book_id, relationship_id, event_type, relation_group, relation_label,
                       state_after, strength_after, polarity_after,
                       chapter_index, source_claim_id, confidence)
```

relation_group 是稳定 enum：family / romance / friendship / mentorship / hierarchy / alliance / rivalry / hostility / debt_obligation / contract / acquaintance / other_social / unknown_significant。

relation_label 是自然语言，保留小说语义细节。

同一对人物可以有多种不同 relation_group 的关系共存（如"师徒 + 对手"）。匹配规则：same pair + same group → 更新 existing relationship；same pair + different group → 创建新 relationship。

### 新增 AI Stage

**Extractor 扩展**：新增 relationship_update 类型 observation，包含 subject_mention、object_mention、relation_hint、confidence。

**Relationship Judge**（两阶段）：

```text
Stage 1: Code Structural Gate
  - 必须是 character-character（subject 和 object 都解析为 character entity）
  - relation_group 必须来自允许的 enum
  - 必须有 evidence span
  - confidence >= 阈值

Stage 2: AI Semantic Judge
  - 输入：resolved characters + relation hint + source spans + 当前关系状态
  - 判断：是否为值得进入人物关系页的长期或重要人物关系
  - 输出：accept / reject / redirect
  - redirect 目标：property_update（归属/位置/装备）、minor_event（临时互动）
```

拒绝场景：

- "张三在青云门" → redirect to affiliation property
- "张三修炼健体三十六式" → redirect to ability property
- "张三和李四同处一室" → reject (minor_event)
- "张三攻击李四一次" → reject（除非关系发生了持续变化）

### 新增 Reducer

**relationship_reducer**：消费 validated/proposed relationship_update claims，commit 成功后标 accepted。

流程：

```text
1. 找 existing relationship（same subject + object + compatible group）
2. 不存在则 create relationship
3. insert relationship_event
4. update relationship current_state / strength / polarity
5. update relationship last_changed_chapter
6. invalidate relationship graph view
```

### 新增 Projection

**Relationship Graph Projection**：从 relationships 构建 graph view model。

支持：关系分组过滤、importance 排序、低重要度关系折叠。

### 复用

- entities / entity_aliases（Phase 1）用于 subject_character_id / object_character_id 引用
- claims 表（Phase 1）存储 relationship_update claims
- ai_runs（Phase 1）记录 Relationship Judge 调用
- source_spans（Phase 1）提供证据

### 不做什么

- identity reveal / merge（Phase 3）
- knowledge system（Phase 4）
- place / map（Phase 5）
- 用户纠错

### 与 Phase 3 的衔接

Phase 3 的实体合并会影响 relationship 的 subject/object 引用。Phase 2 的 relationship 表使用 entity ID，合并时需要 rebuild affected relationships。

### 与 Phase 1 的衔接

Phase 1 的 Extractor 不产出 relationship_update。Phase 2 启用后，Extractor prompt 扩展 observation types，Risk Classifier 对 relationship_update 标记 high risk，进入 Relationship Judge。

---

## 8. Phase 3 Design: Identity Reveal / Merge / Split

### 目标

处理改名、马甲、称号、隐藏身份、同名不同人、误合并。

### 新增能力

- Canonical Layer：entity_identity_links
- AI Pipeline：Identity Judge
- Entity Merge 流程
- Relationship Migration（实体合并后 rebuild affected relationships）
- User Correction：用户标记"不是同一个人"

### 新增数据模型

```text
entity_identity_links (book_id, entity_a_id, entity_b_id,
                       link_type,  -- same_identity / possible_same_identity / disguise / mistaken_identity
                       confidence, source_claim_id, status)
```

### 新增 AI Stage

**Extractor 扩展**：新增 identity_reveal / entity_merge_candidate 类型 observation。

**Identity Judge**：

```text
输入：两个 entity 的 aliases + properties + source spans
判断：
  - 高置信 (>= 阈值)：link_type = same_identity → 触发 merge
  - 中置信：link_type = possible_same_identity → 创建 link，不 merge
  - 低置信：标记 quarantined/uncertain
```

不因为名字相似直接 merge。必须有 source span 证据支持。

### Entity Merge 流程

Claim ledger 尽量 append-only。merge 时不静默改写历史 claims 的 subject/object entity_id。

```text
1. 确定 survivor entity（保留哪个）
2. 合并 aliases：victim 的 aliases 转移到 survivor
3. 合并 properties：按 dimension_key 合并。无冲突时直接迁移；冲突时不能静默丢弃 victim 的值 — 保留为历史记录（status = superseded）或标记 merge_conflict 留待 audit
4. 迁移 relationships：victim 作为 subject/object 的 relationship 改引用 survivor
5. 重建 affected relationship projection
6. 新增 entity_links(link_type=redirect, entity_a_id=victim_id, entity_b_id=survivor_id) 记录，历史 claims 保持原样不改写
7. Projection 读取时 resolve redirect，确保展示正确
8. 标记 victim entity status = merged
```

### Relationship Migration

实体合并后，受影响的 relationship projection 必须 rebuild：

```text
1. 找到所有 subject_character_id = victim_id 或 object_character_id = victim_id 的 canonical relationships
2. 改引用为 survivor_id
3. 检查是否与 survivor 已有的 relationship 冲突
4. 冲突时保留 survivor 的关系，victim 的关系标记为 merged
5. 重建 relationship graph projection
```

历史 claims 不改写（append-only）。relationship_events 保留历史记录不变，只迁移 canonical relationships 的引用。

### User Correction

用户可以标记"不是同一个人"，阻止未来自动 merge：

```text
correction_type = not_same_person
target: entity_a_id + entity_b_id
效果：后续 Identity Judge 对这对 entity 不再产出 same_identity claim
```

### 复用

- entities / entity_aliases（Phase 1）是合并的主体
- entity_properties / entity_current_properties（Phase 1）需要迁移
- relationships / relationship_events（Phase 2）需要 migration
- claims（Phase 1）存储 identity_reveal / entity_merge_candidate

### 不做什么

- knowledge system（Phase 4）
- place / map（Phase 5）
- 批量审计
- 用户手动合并 UI

### 与 Phase 2 的衔接

Phase 2 的 relationship 使用 entity ID。Phase 3 的 merge 必须维护引用一致性。

### 与 Phase 4 的衔接

Phase 4 的 knowledge assertions 关联 entity。merge 时需要更新 assertion 引用（Phase 3 scope 内处理 entity 相关引用，knowledge 引用在 Phase 4 建立后通过 referenced_entity_ids 补全）。

---

## 9. Phase 4 Design: Knowledge System

### 目标

展示修炼体系、势力结构、历史、规则、秘密，并支持后期修正。

### 新增能力

- Canonical Layer：knowledge_cards、knowledge_assertions
- AI Pipeline：Knowledge Extractor、Knowledge Revision Judge
- Reducer：knowledge_reducer
- Projection：Knowledge View Model

### 新增数据模型

```text
knowledge_cards       (book_id, category, topic, current_summary,
                       confidence, importance_score,
                       first_seen_chapter, last_updated_chapter, status)

knowledge_assertions  (book_id, card_id, claim_id,
                       assertion_text, status,  -- active / rumor / uncertain / revised / contradicted / false_in_world
                       chapter_index, supersedes_assertion_id,
                       referenced_entity_ids)  -- 知识断言引用的实体 ID 列表，merge 时需要更新
```

category：power_system / faction_structure / world_rule / history / secret / prophecy / politics / geography / custom

knowledge topic 需要经过 Resolver 归一：AI 可能用不同表述描述同一 topic（如"修炼体系"vs"境界系统"），Resolver 须映射到已有 card 或显式创建新 card。

### 新增 AI Stage

**Extractor 扩展**：新增 knowledge_assertion 类型 observation，包含 topic、category、assertion_text、confidence。

**Knowledge Revision Judge**：

```text
输入：新 assertion + 同 topic 的已有 assertions + source spans
判断：
  - 补充信息：新 assertion.status = active，旧不动
  - 修正旧认知：新 assertion.status = active，旧 assertion.status = revised
  - 推翻旧认知：新 assertion.status = active，旧 assertion.status = contradicted
  - 传闻/不确定：新 assertion.status = rumor / uncertain
```

### 新增 Reducer

**knowledge_reducer**：消费 validated/proposed knowledge_assertion claims，commit 成功后标 accepted。

流程：

```text
1. 找 existing knowledge_card（same topic + category）
2. 不存在则 create knowledge_card
3. insert knowledge_assertion
4. 如果 Revision Judge 判定为 revised/contradicted，更新旧 assertion status
5. 更新 knowledge_card.current_summary
6. update knowledge_card.last_updated_chapter
7. invalidate knowledge view model
```

### 新增 Projection

**Knowledge Projection**：从 knowledge_cards + knowledge_assertions 构建 view model。

前端展示：当前已知 (current_summary) + 早期说法 (revised/contradicted assertions) + 修正历史。

### 复用

- claims（Phase 1）存储 knowledge_assertion / knowledge_revision claims
- ai_runs（Phase 1）记录 Knowledge Revision Judge 调用
- source_spans（Phase 1）提供证据

### 不做什么

- place / map（Phase 5）
- 批量审计
- 用户纠错扩展

### 与 Phase 3 的衔接

Phase 3 的实体合并可能影响 knowledge_assertions 关联的 entity 引用。如果 knowledge assertions 引用 entity ID，merge 时需要更新。

---

## 10. Phase 5 Design: Places and Map

### 目标

结构化地点层级和拓扑地图。

### 新增能力

- Canonical Layer：place_details、place_edges
- AI Pipeline：Map Extraction
- Reducer：place_reducer
- Projection：Map View Model、layout snapshots

### 新增数据模型

```text
place_details         (entity_id, book_id, place_type, parent_place_id, scale_level,
                       importance_score, map_visible)

place_edges           (book_id, from_place_id, to_place_id,
                       edge_type,  -- contains / near / adjacent_to / route_to / north_of / ...
                       direction_hint, distance_hint,
                       confidence, source_claim_id, first_seen_chapter, status)
```

地点本身是 entities (entity_type = place)。place_details 是扩展信息。

place_type：world / region / country / city / sect / building / room / mountain / river / secret_realm / route / unknown

map_layout_snapshots 存储地图布局快照：

```text
map_layout_snapshots   (book_id, max_chapter, layout_json, created_at)
```

布局从 place_edges 投影生成，可以自动布局和美化。事实层（place_edges）必须忠于原文，展示层（layout_snapshot）可以美化。

### 新增 AI Stage

**Extractor 扩展**：新增 location_introduction / location_edge 类型 observation。

**Map Conflict Judge**：

```text
输入：新 location_edge + 已有 place_edges + source spans
判断：
  - 接受：不与已有拓扑矛盾
  - 冲突：与已有拓扑矛盾，标记 conflict
  - 不确定：证据不足，标记 uncertain
```

### 新增 Reducer

**place_reducer**：消费 validated/proposed location_introduction / location_edge claims，commit 成功后标 accepted。

流程：

```text
1. 创建/更新 entity (type=place) + place_detail
2. 创建/更新 place_edge（用 ID 引用，不用名字）
3. 检查拓扑一致性
4. invalidate map view model
```

### 新增 Projection

**Map Projection**：从 place_details + place_edges 构建拓扑 view model。

**Layout Snapshot**：从 place_edges 投影生成自动布局。地图事实来自原文证据，不做 AI 图片事实源。

### 复用

- entities（Phase 1）存储 place 类型实体
- entity_aliases（Phase 1）存储地点别名
- claims（Phase 1）存储 location claims
- source_spans（Phase 1）提供证据

### 不做什么

- 批量审计
- 复杂用户纠错
- AI 图片地图作为事实源

---

## 11. Phase 6 Design: Quality / Audit / Reprocess

### 目标

长期维护几百章小说资料不崩。

### 新增能力

- Quarantine Workflow：quarantined_claims 管理界面
- Batch Audit：扫描低置信事实、重复实体、关系污染
- Duplicate Entity Detection
- Relationship Pollution Audit
- Selective Reprocess
- Prompt Regression 测试框架
- Quality Metrics
- User Correction 扩展

### 新增数据模型

```text
quarantined_claims    (book_id, claim_id, reason_code, reason_text, suggested_action)
user_corrections      (book_id, target_type, target_id, correction_type, correction_json, status)
quality_metrics       (book_id, metric_type, metric_value, measured_at)
```

### 新增能力详情

**Quarantine Workflow**：

- 管理界面展示 quarantined claims，按 reason_code 分类
- 支持批量处理：accept / reject / retry / reclassify
- 分类重跑：只重跑某个 reason_code 影响的 claims

**Batch Audit**：

- 重复实体检测：canonical_name 相似度 + alias 交叉 → merge 建议
- 关系噪音检测：低 confidence + 长期未更新 + 无 event history → 降权建议
- 低置信事实扫描：confidence < 阈值的 accepted claims → review 建议

**Selective Reprocess**：

- 重跑单章：标记 chapter_processing_run stale → 重新处理
- 重跑章节范围：批量标记 stale → 顺序重跑
- 重跑某类 claim：只标记特定 claim_type 的 claims 为 stale → 重跑
- 重跑后标记旧 claims 为 stale/superseded/reprocessed，不物理删除，保持 claim ledger 可审计

**Prompt Regression**：

- 固定测试集（15 个核心场景），每次改 prompt 都跑
- 对比不同 prompt_version 的 ai_runs 输出
- 自动检测 regression

**User Correction 扩展**：

- 隐藏关系
- 修改展示名
- 标记非同一人（Phase 3 基础能力扩展）
- 手动合并实体
- 修正属性
- 修正地点关系

### 复用

- 全部 shared foundation
- 全部 Phase 1-5 的 domain tables
- ai_runs 用于 prompt regression 对比
- claims 状态机用于 quarantine workflow

### 不做什么

- 新的数据类型
- 新的 AI pipeline 阶段
- 跨书 ontology

---

## 12. V4 API and Projection Contract

V4 使用全新 API 契约，不兼容 Legacy response shape。

V4 external API follows existing reader API style:
- routes use `/api/books/v4/...`
- book identity is passed via `bookUrl` query/body field
- internal V4 storage uses `book_id = md5_hex(book_url)`

This differs from the conceptual `/api/books/:book_id/v4/...` notation used in early design discussions. Future phases must follow the implemented route convention unless intentionally migrated.

V4 routes are separate from Legacy V3 routes; implement as new V4 handlers/modules, do not patch Legacy V3 handlers unless only for routing registration.

Phase 1 接口：

```text
GET  /api/books/v4/memory          — 从 view_model_cache 读取，miss 时 on-read projection
GET  /api/books/v4/characters      — book-level character list
GET  /api/books/v4/characters/:id  — entity-level character card
GET  /api/books/v4/chapter-memory  — 章节级 memory view
GET  /api/books/v4/memory/status   — { max_read_chapter, max_processed_chapter, processing, last_error }
POST /api/books/v4/memory/reset    — 清空 canonical state / claims / cache / processing state
POST /api/books/v4/enabled         — 启用/禁用
POST /api/books/v4/chapter-memory/generate — 触发单章处理（调用真实 pipeline）
POST /api/books/v4/catchup/start   — 触发批量处理（调用真实 pipeline）
GET  /api/books/v4/catchup/status  — processing_progress + job state
POST /api/books/v4/catchup/cancel  — 设置 cancel_requested
```

后续阶段新增：

```text
Phase 2: GET /api/books/:book_id/v4/relationships
Phase 4: GET /api/books/:book_id/v4/knowledge
Phase 5: GET /api/books/:book_id/v4/map
Phase 6: POST /api/books/:book_id/v4/reprocess, POST /api/books/:book_id/v4/corrections
```

Projection 层负责将 canonical tables 映射到 V4 API 响应格式。

## 13. Clean-slate DB Initialization / Reset Design

V4 使用全新数据库，不迁移 Legacy JSON Memory。

初始化：

- 创建所有 Phase 1 表 + 索引
- 初始化 property_dimensions 内置 character dimensions
- Legacy `ai_book_memories` 表和 `data/{user_ns}/ai-books/` 文件不迁移、不读取、不自动 reset
- 用户需要重新触发 catchup 重建资料

Reset（用户触发）：

- 清空该 book 的：claims / claim_source_spans / entities / entity_aliases / entity_properties / entity_current_properties / view_model_cache / chapter_summaries / chapter_processing_runs / processing_progress（重置为 idle）
- 保留并复用：chapters / chapter_segments / source_spans（章节 hash 未变化则保留，变化则重建）
- 保留：ai_runs（审计保留，不参与当前 projection）
- 保留：reading_progress
- property_dimensions 保留（注册表不随 reset 清除）

## 14. Failure Handling and Idempotency

**失败不能污染资料库**（以下均指 V4 AI output 处理失败，不涉及 Legacy JSON Memory）：

```text
AI output JSON parse fail  → repair once → 再失败 reject claim
AI output schema fail      → repair once → 再失败 reject claim
evidence missing           → reject claim
dimension_key invalid      → reject claim
confidence out of range    → reject claim
resolver uncertain         → uncertain claim
judge reject               → reject claim
AI timeout                 → retry with backoff (max 2)
chapter hash change        → old run stale, 允许重跑
```

RealAiExtractor output parse/schema/evidence validation failed:
- repair once
- repair still failed → ai_run failed/rejected
- chapter_processing_run failed
- 不写 canonical state

**事务规则**：

```text
Reducer 必须在 transaction 内提交
任意关键步骤失败，整章 canonical state 不更新 (rollback)
ai_runs 和失败日志可以保留（不回滚审计记录）
```

**幂等性**：

```text
chapter_processing_runs: (book_id, chapter_index, chapter_hash, prompt_version, schema_version) UNIQUE
同一组合已成功 → skip
hash 变化 → 允许重跑
```

**并发控制**：

```text
同一本书同一时间只允许一个 processing worker
新进度可以更新 queue target
旧任务不能处理超过 max_read_chapter
```

## 15. Cross-phase Extension Rules

Phase 1 建立 shared foundation 后，后续阶段必须遵守：

1. **语义兼容**：保持 shared foundation 语义兼容。允许 additive migrations（新增列、新增表），不允许重定义核心语义
2. **Claim type 扩展**：每个阶段新增自己的 claim_type，不删除已有类型
3. **Extractor 扩展**：每个阶段扩展 Extractor 的 observation types，不替换已有类型。只能 additive 扩展 observation types / prompt sections / validators。不得替换 Extractor framework。不得绕过 parse / schema validation / evidence validation。不得让 AI 直接写 canonical tables
4. **Reducer 独立**：每个阶段的 domain reducer 只消费自己的 claim types
5. **Projection 独立**：每个阶段的 projection 只读取自己的 domain tables + shared entities
6. **Judge 按需引入**：Phase 1 无 Judge，Phase 2+ 按需引入，每个 Judge 只处理自己的 high risk claims
7. **Risk Classifier 共享**：所有阶段共用同一个 Risk Classifier，新增 claim_type 时更新分类规则
8. **Context Builder 共享**：所有阶段共用同一个 Context Builder，新增 domain 时扩展 context 内容
9. **ai_runs 共享**：所有阶段的 AI 调用都记录到同一个 ai_runs 表
10. **V4 API Stability**：Projection 层保证 V4 API 响应格式一致，后续阶段不破坏已发布的 V4 接口

## 16. Design Risks

**Phase 1 底座风险**：

- 风险：Phase 1 的表结构设计不周，导致后续阶段需要 ALTER TABLE
- 缓解：Shared foundation 设计已考虑全阶段 claim types、entity types、dimension keys

**AI 调用链路风险**：

- 风险：Extractor + Resolver 两次 AI 调用增加延迟和成本
- 缓解：Context Builder 控制 token 用量；Phase 1 验证成本增量后再优化

**Claim 膨胀风险**：

- 风险：几百章后 claims 表数据量大，查询变慢
- 缓解：按 book_id + chapter_index 分区索引；SQLite 单表百万级没问题

**Projection 一致性风险**：

- 风险：canonical state 变更后 projection 未及时 rebuild
- 缓解：Reducer 事务提交后同步触发 projection rebuild

**Judge 准确率风险**：

- 风险：Phase 2 的 AI semantic judge 可能误判
- 缓解：Phase 6 通过 audit + prompt regression 持续优化；judge 的 accept/reject 决策可追溯到 ai_runs

**V4 API / 前端集成风险**：

- 风险：全新 API 契约需要前端同步改造，旧前端无法直接对接
- 缓解：V4 API 设计已考虑前端消费模式，view model 结构清晰；前端改造与后端同步进行

**Real AI extraction readiness risk**：

- 风险：MockExtractor E2E 通过但真实模型输出 schema drift、evidence_span_ids 错误或抽取质量差
- 缓解：RealAiExtractor 使用 strict JSON schema、parse/validation/repair once、RUN_REAL_AI_TESTS=1 smoke test、prompt_version/schema_version 记录到 ai_runs

## Phase 2 Finalized Relationship Architecture

> Phase 2 已 Product Ready Frozen。以下记录最终实现架构，供 Phase 3+ 参考。

### Ownership

- Relationship Judge（Structural Gate + RealAiRelationshipJudge）属于 Phase 2，不属于 Phase 3。
- Phase 3 处理 identity merge / descriptive names / same-name-different-person / relationship remap。

### Pipeline Architecture

1. **relationship_update 是 special high-risk path**：status=proposed，不走 generic quarantine。
2. **Two-phase resolution**：entity/property first-pass → relationship re-resolve 后处理。
3. **Structural Gate（纯代码）→ RealAiRelationshipJudge（LLM 调用）**：Gate 先做 redirect/reject，Pass 后才调 AI Judge。
4. **Resolver 对 object 使用 best-effort resolution**：object 可以 unresolved / non-character。只有 relationship_reducer 写 canonical relationship 前才要求 subject/object 都是 character。
5. **Redirect derived property_update** 通过 second-pass property_reducer 消费。
6. **relationship_reducer 使用独立 transaction**，不回滚 Phase 1 character/property canonical state。

### Constraints

- Phase 2 directionality 只支持 `directed` / `undirected`。
- Relationship graph/list 默认只展示 `status='active'` relationships。
- AI model 可能对数值字段返回字符串（如 strength="strong"）。`normalize_judge_json` 负责 string→f64 转换。

### Phase 3 Deferred

- identity merge 后 relationship remap
- relationship direction normalization / reverse-edge folding
- same-name-different-person disambiguation
- descriptive character names 合并

## Phase 3 Final Freeze Status

> Phase 3 已 Final Frozen。冻结范围是 Identity Reveal / Merge / Split safety，不包含 Phase 4 knowledge、map/place system、batch audit dashboard、cross-book identity ontology 或 manual correction UI。

### Frozen Outcome

- Backend: PASS
- Frontend: PASS
- Real AI Smoke: PASS
- Phase 1 Regression: PASS
- Phase 2 Relationship Regression: PASS
- Product Readiness: PASS
- Phase 3 Freeze: YES
- Ready for Phase 4 planning: YES

### Frozen Architecture

1. Phase 3 只做 additive extension，不重定义 Phase 1/2 shared foundation。
2. Identity Judge 只输出 decision，不直接写 canonical state。
3. Identity Reducer / Entity Merge Reducer 是唯一合法 merge 写入口。
4. `entity_identity_links` 保存 semantic identity links 和 reducer-created redirect links。
5. `entity_merge_operations` 是 merge ledger，`entity_merge_conflicts` 是 conflict audit。
6. Active `not_same_identity` link 会阻止未来 automatic merge。
7. Redirect link 保持 directional：victim -> survivor。
8. Projection/API 默认 resolve redirect，merged victim 不作为活跃人物暴露。
9. Relationship migration 先 compute remapped pair，再处理 self-edge / duplicate / update，避免 UNIQUE collision。
10. Frontend 只提供 lightweight debug surface，不提供 manual merge / split / correction actions。

### Freeze Evidence

- Final report: `PHASE_3_FINAL_VERIFICATION_REPORT.md`
- `cargo test --lib`: 465 passed
- `cargo test v4 --lib`: 281 passed
- `npm run build`: passed
- Frontend identity focused tests: 8 files / 36 tests passed
- `RUN_REAL_AI_TESTS=1` identity fixture smoke with `gpt-5.4`: passed
- Admin namespace real source smoke on 《奥术神座》 first chapters: passed as real-book pipeline coverage; those early chapters did not produce an accepted identity merge.

### Known Deferred Work

- Complex user correction UI remains non-scope.
- Broad automatic split remains non-scope; unsafe split stays audit-first.
- Knowledge system and map/place system remain Phase 4+ work.
- If Phase 4 wants stronger real-book identity-merge evidence, use later real-book chapters or a curated real-book excerpt containing explicit disguise/title reveal.

## Phase 4 Final Freeze Status

> Phase 4 已 Final Frozen。冻结范围是 Knowledge System：knowledge cards / assertions / revision history / referenced entities / projection / API / lightweight frontend panel。不包含 Phase 5 map/place system、user correction UI、batch audit dashboard、vector search、full-text search 或 cross-book ontology。

### Frozen Outcome

- Backend: PASS
- Frontend: PASS
- Real AI Smoke: PASS
- Phase 1 Regression: PASS
- Phase 2 Relationship Regression: PASS
- Phase 3 Identity Regression: PASS
- Product Readiness: PASS
- Phase 4 Freeze: YES
- Ready for Phase 5: YES

### Frozen Architecture

1. Phase 4 只做 additive extension，不重定义 Phase 1/2/3 shared foundation。
2. `knowledge_assertion` 是 Phase 4 special path，不因 risk level 绕过 Knowledge Revision Judge。
3. Topic Resolver 只在 same book + same category 范围内做 deterministic candidate retrieval。
4. Knowledge Structural Gate 过滤 character property、relationship、identity reveal、map/place edge、chapter summary、minor event 等非知识边界。
5. RealAiKnowledgeRevisionJudge 只输出 decision，不直接写 canonical knowledge tables。
6. `knowledge_reducer` 是唯一合法 canonical knowledge 写入口，且使用独立 transaction。
7. accepted claim 只能在 reducer 成功写入 canonical state 后标记。
8. rumor / uncertain / false_in_world 可以保留为 assertions，但不得污染 factual `current_summary`。
9. `knowledge_assertion_links` 支持 one-to-many revision / contradiction history。
10. referenced entities 写入前 resolve Phase 3 redirect；projection/API fallback 再次 resolve redirect。
11. Frontend 只提供 lightweight Knowledge Panel，不提供 manual correction、map UI 或 batch audit UI。

### Freeze Evidence

- Final report: `PHASE_4_FINAL_VERIFICATION_REPORT.md`
- `cargo test --lib`: 529 passed
- `cargo test v4 --lib`: 346 passed
- `cargo test knowledge --lib`: 60 passed
- Frontend knowledge focused tests: 3 files / 7 tests passed
- `npm run build`: passed
- `RUN_REAL_AI_TESTS=1` strict Phase 4 freeze fixture with `gpt-5.4`: passed
- `RUN_REAL_AI_TESTS=1` admin namespace 《奥术神座》 real-source knowledge smoke with `gpt-5.4`: passed

### Known Deferred Work

- Map/place system remains Phase 5 work.
- User correction UI remains later-phase work.
- Batch audit dashboard remains later-phase work.
- Vector/full-text search and cross-book ontology remain non-scope.
- Topic resolver aliases and `custom` category rules may need tuning after broader real-book testing.
