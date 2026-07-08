# AI Book V4 Frontend Development Guide

本文档给没有上下文的 agent 快速接手 AI 资料 V4 前端使用。它描述当前代码里的目标形态、组件边界、API 约束和 UI 规则。

配套后端文档见 `docs/architecture/ai-book-v4-backend.md`。前端只消费后端 projection/view model，不在浏览器里修事实。

## 一屏结论

V4 前端目标是一个独立的 AI 资料控制台：

- Route-level shell: `frontend/src/views/AiBookV4View.vue`
- API client: `frontend/src/api/v4/book.ts`
- API transport: `frontend/src/api/v4/http.ts`
- Shared V4 types: `frontend/src/types/v4.ts`
- Domain panels: `frontend/src/components/reader/V4*Panel.vue`
- Shared UI primitives: `frontend/src/components/reader/v4/*.vue`
- Shared async panel state: `frontend/src/composables/useV4PanelState.ts`

新功能优先挂到 `AiBookV4View.vue` + V4 panels，不要继续塞进旧 `AiBookView.vue`。旧页面里仍有 V3/V4 混合痕迹，属于迁移残留，不是新开发模板。

## 当前控制台结构

`AiBookV4View.vue` 是 composition surface，负责：

- 从 route query 读取 `bookUrl` / `chapterIndex`。
- 拉取书籍信息、V4 memory status、catchup status。
- 提供顶部 action：
  - 刷新状态
  - 生成当前章节
  - 补齐到当前阅读
  - 重置 V4 资料
- 维护 rail 当前选中 domain。
- 挂载各 domain panel。

当前 rail：

| Key | Label | Component | 说明 |
| --- | --- | --- | --- |
| `overview` | 总览 | `V4BookOverviewPanel.vue` | 书级概览、统计入口。 |
| `task` | 任务 | `V4TaskProgressPanel.vue` | 用现有 status/catchup 数据可视化任务进度。 |
| `characters` | 角色 | `V4CharacterPanel.vue` | 角色目录、人物卡、当前状态。 |
| `relationships` | 关系 | `V4RelationshipPanel.vue` | 关系列表、网络 cluster、inspector。 |
| `knowledge` | 知识 | `V4KnowledgePanel.vue` | 知识卡/分类，不要做无限 fact stream。 |
| `map` | 地点 | `V4MapPanel.vue` | 地点、空间关系、地图冲突。 |
| `quality` | 质量 | `V4QualityPanel.vue` | quarantine、audit、correction、reprocess 等质量工作台。 |

身份 Identity 不再作为主 rail tab。它可以作为角色详情、质量排查或 debug 子视图出现，但不是读者第一层导航。

## 数据边界

前端只展示后端 V4 projection/API 返回的数据：

- 不在前端 merge identity。
- 不在前端 dedupe relationship facts。
- 不在前端 normalize relationship group/label。
- 不在前端把 knowledge fact 改写成另一个事实。
- 不在前端修 place/map conflict。
- 不把 V3 memory shape 当成 V4 canonical truth。

允许的前端 reshape：

- 排序、筛选、分组。
- 视觉上折叠/展开。
- score 显示格式化。
- 名称 fallback。
- 空状态、错误状态、loading 状态。
- 将后端返回的 view model 转成组件局部展示结构。

如果发现数据“不舒服”，先判断是哪一类：

- 后端 projection 已经返回但展示方式差：前端可改 UI。
- 后端 projection 缺字段：补 V4 API/type，不在前端猜。
- canonical 事实错：走后端 decision/reducer/quality，不在前端修。
- 任务进度缺阶段细节：先用现有 status 可视化，后端补 API 后再接入更细粒度 timeline。

## API 约定

所有 V4 前端接口集中在 `frontend/src/api/v4/book.ts`，底层 axios instance 是 `frontend/src/api/v4/http.ts`。

当前 base URL：

```ts
baseURL: '/api/books/v4'
```

`v4Http` 负责：

- 注入 `Authorization` 和 `X-Secure-Key`。
- 解包后端 `ApiResponse`。
- 对 `NEED_LOGIN` dispatch `need-login` event。
- 把错误统一转成 `Error`。

新增 V4 API 时：

1. 先在 `frontend/src/types/v4.ts` 增加 response/request type。
2. 在 `frontend/src/api/v4/book.ts` 增加函数。
3. 参数名沿用现有风格：`bookUrl` 放 query params 或 request body。
4. 路径参数必须 `encodeURIComponent`。
5. 给 API wrapper 补 `frontend/src/api/v4/*.test.ts` 或现有测试。

不要在 panel 里直接写 axios。Panel 只调用 `book.ts` 导出的 typed function。

## 类型约定

`frontend/src/types/v4.ts` 是 V4 API response shape 的前端契约。

约束：

- 类型名统一用 `V4` 前缀。
- Response type 尽量映射后端 projection/view model。
- 前端局部展示类型可以放组件内部，例如 panel 的 `DisplayItem`。
- 不要把后端缺失字段用 optional any 糊过去；先确认 API 是否应该补字段。
- 避免 `unknown[]` 扩散到业务 UI。旧字段可保留，但新 UI 应尽快用明确类型。

常见 domain type：

- `V4MemoryResponse`
- `V4MemoryStatusResponse`
- `V4CatchupStatusResponse`
- `V4CharacterListItem`
- `V4CharacterCardView`
- `V4RelationshipGraphView`
- `V4KnowledgeOverviewView`
- `V4MapOverviewView`
- `V4QualityOverviewView`

## 组件边界

使用 Vue 3 Composition API + `<script setup lang="ts">`。

Route view：

- `AiBookV4View.vue` 只做 shell/orchestration。
- 不把 domain UI 直接写在 route view 里。
- 新 rail tab = 更新 rail item + 增加一个 domain panel。

Domain panel：

- 放在 `frontend/src/components/reader/V4*Panel.vue`。
- 只负责一个 domain。
- 通过 `bookUrl` prop 拉取自己的数据。
- 使用 `V4PanelShell` 承接 loading/error/empty/retry。
- 复杂 section 拆到 `frontend/src/components/reader/v4/` 或 domain-specific child component。

Shared V4 primitives：

- `V4ConsoleShell.vue`: 顶层 console layout，header + rail + main。
- `V4DomainRail.vue`: 左侧/移动端 rail。
- `V4PanelShell.vue`: panel 容器、toolbar、loading/error/empty。
- `V4MetricCard.vue`: 指标卡。
- `V4InspectorSection.vue`: inspector 区块和折叠。
- `V4ConfidenceBadge.vue`: 置信度展示。
- `V4EmptyState.vue` / `V4ErrorState.vue` / `V4LoadingState.vue`: 状态组件。

数据流：

- Props down, events up。
- Panel 不修改父组件 state，必要时 emit。
- 不用 component ref 做常规通信。
- `v-model` 只用于真正的双向输入控件。

## 异步状态

简单 panel 数据加载优先用 `useV4PanelState`：

```ts
const state = useV4PanelState(
  () => getV4Characters(props.bookUrl),
  computed(() => props.bookUrl),
  { emptyCheck: (data) => data.characters.length === 0 },
)
```

它提供：

- `loading`
- `error`
- `data`
- `empty`
- `reload`

它也用 `requestId` 防止旧请求覆盖新请求。复杂场景，比如 master/detail 的角色详情，可以在 panel 内另开 detail request id。

注意：

- derived data 用 `computed`。
- watcher 只做 side effect，例如 bookUrl 变化后 reload。
- 不在 template 里写复杂 filter/sort/map。
- 原始 response state 尽量少，展示结构用 computed 派生。

## UI / UX 原则

V4 AI 资料是小说阅读辅助控制台，不是营销页。

优先级：

1. 读者现在需要记住什么。
2. 当前章节附近的角色状态、关系变化、地点和知识。
3. 可追溯证据和质量风险。
4. Debug/运维信息。

页面形态：

- 工作台式、信息密度适中。
- 左 rail 只放一级 domain。
- 每个 panel 内部使用 master/detail、inspector、可折叠 section。
- 大列表要 progressive disclosure，不要一口气 dump 1000 条。
- 任务进度用 pipeline/stage/timeline/错误 inspector，而不是只给百分比。

视觉规则：

- 使用现有 CSS tokens：`--color-bg`、`--color-bg-sunken`、`--color-text`、`--color-border`、`--color-primary`、`--color-danger` 等。
- 卡片半径控制在 8px 左右；局部 metric 可以更圆，但不要卡片套卡片。
- 不做 landing page/hero。
- 不用大面积装饰渐变、orb、bokeh。
- 文本不能溢出按钮/卡片。
- 移动端 rail 横向滚动，panel 内容单列。
- 保持可访问性：`aria-label`、`role="tabpanel"`、loading 用 `role="status"`，错误用 `role="alert"`。

## Domain 展示指南

### Overview

展示书级状态和入口，不要重复每个 domain 的完整内容。

适合：

- 已读/已处理章节。
- processing/error summary。
- character/relationship/knowledge/place/quality 计数。
- 最近风险或下一步 action。

### Task Progress

当前只能基于现有接口：

- `getV4MemoryStatus`
- `getV4CatchupStatus`

可展示：

- running/failed/cancelled/completed/idle。
- current / target / maxProcessed。
- lastError。
- 推导出的 stage。

后端未来补数据后再接入：

- per chapter timeline。
- adapter parsed claims count。
- decision write/no-write/quarantine count。
- reducer accepted/rejected/skipped count。
- projection refreshed cache keys。

不要为 1000 章画全量 heatmap；用范围、搜索、分页、窗口化时间线。

### Characters

角色页应突出“当前状态”：

- 身份/阵营/职业/等级。
- 境界/技能/装备。
- 位置/目标/心理/生死状态。
- firstSeen/lastSeen/importance。

当前 `V4CharacterPanel.vue` 已从 `currentStates` 分组展示状态。新增状态维度时优先让后端 projection 返回 `currentStates`，前端只做分组和标签展示。

### Relationships

关系页适合 cluster/network + edge inspector。

展示：

- group/label/directionality。
- currentState。
- strength/polarity/confidence/importance。
- eventCount/latestSourceClaimId。

不要在前端把 family inverse 合并成另一条事实。后端 projection 返回什么关系，前端就展示什么关系。

### Knowledge

知识不适合无限 fact stream。小说知识会很多，应该按 card/category/topic 管理。

推荐：

- category overview。
- topic cards。
- card detail 内按 assertion status 分组。
- 支持搜索/筛选/重要度排序。

避免：

- 全书所有 fact 一条长流。
- 把临时事件和长期知识混在一起。
- 前端自行判断 contradiction/revise。

### Map / Place

地点页适合空间结构和冲突检查：

- place list / hierarchy。
- graph/layout。
- selected place detail。
- conflicts inspector。

不要在前端修 contains/east_of 等边的冲突或方向。

### Quality

质量页是工作台，不是普通读者主内容：

- quarantine。
- audit runs/findings。
- corrections。
- reprocess jobs。
- prompt regression。

质量 action 必须走 `book.ts` API，不能在前端改本地状态假装成功。成功后 reload 对应数据。

## 和 V3 的关系

当前 repo 仍有旧 `AiBookView.vue`，里面保留 V3 memory 和部分 V4 panel 切换。这是历史过渡页面。

新 V4 前端开发规则：

- 新 V4 控制台功能加到 `AiBookV4View.vue`。
- 新 V4 API/type/panel 不依赖 V3 `memoryView`。
- 不把 V3 的 `characters` / `relationships` / `knowledgeFacts` 当作 V4 数据源。
- 不为了兼容旧 V4 UI 牺牲新控制台结构；V4 尚未正式上线，允许 breaking change。

## 测试规则

常用命令：

```bash
cd frontend && npm test
cd frontend && npm run build
```

针对性测试：

- API wrapper: `frontend/src/api/v4/*.test.ts`
- Route shell: `frontend/src/views/AiBookV4View.test.ts`
- Domain panel: `frontend/src/components/reader/V4*Panel.test.ts`
- Shared primitives: `frontend/src/components/reader/v4/*.test.ts`

测试重点：

- API path / query / body 正确。
- loading/error/empty 状态。
- rail 切换。
- action disabled/busy 状态。
- failed task 显示具体错误。
- master/detail 选择和 reload。
- 大量数据时不要溢出/遮挡。

视觉相关改动完成后，优先用真实浏览器或截图检查：

- desktop。
- mobile/narrow viewport。
- rail 不跑到 header 外。
- 文本不溢出。
- panel 内容没有卡片套卡片或错位。

## 新增一个 V4 panel 的步骤

1. 确认后端已有 projection/API；没有就先补后端契约。
2. 在 `frontend/src/types/v4.ts` 添加 typed response。
3. 在 `frontend/src/api/v4/book.ts` 添加 API wrapper 和测试。
4. 新建 `frontend/src/components/reader/V4XxxPanel.vue`。
5. 用 `V4PanelShell` 和 `useV4PanelState` 处理 loading/error/empty。
6. 把复杂 UI 拆成 shared primitive 或 panel-local child。
7. 在 `AiBookV4View.vue` 添加 rail item 和 panel mount。
8. 添加 panel test。
9. 跑 focused tests，再跑 `cd frontend && npm run build`。

## 常见反模式

不要这样做：

- 在 `.vue` 里直接 axios 请求 V4 API。
- 在 template 里写大段 filter/sort/map。
- 把 route view 写成巨型页面。
- 复用 V3 store 当 V4 truth。
- 为缺失后端字段在前端猜事实。
- 在 quality action 后只本地 splice 数据，不重新拉取真实状态。
- 为 1000 章渲染 1000 个复杂节点。
- 新增大面积营销式 hero 或解释性文案。
- 让左 rail 脱离 console shell/header。

应该这样做：

- API/type 先行。
- Panel 自己拉自己的 projection 数据。
- 复杂展示用 computed 派生。
- 事实问题回后端，展示问题留前端。
- 大数据用分页、筛选、窗口化、分层 drill-down。
- 每个新行为有测试。

## 快速找代码

| 你想看 | 文件 |
| --- | --- |
| V4 控制台入口 | `frontend/src/views/AiBookV4View.vue` |
| V4 API wrappers | `frontend/src/api/v4/book.ts` |
| V4 axios instance | `frontend/src/api/v4/http.ts` |
| V4 shared types | `frontend/src/types/v4.ts` |
| 通用 panel state | `frontend/src/composables/useV4PanelState.ts` |
| Console shell | `frontend/src/components/reader/v4/V4ConsoleShell.vue` |
| Rail | `frontend/src/components/reader/v4/V4DomainRail.vue` |
| Panel shell | `frontend/src/components/reader/v4/V4PanelShell.vue` |
| Overview panel | `frontend/src/components/reader/V4BookOverviewPanel.vue` |
| Task panel | `frontend/src/components/reader/V4TaskProgressPanel.vue` |
| Character panel | `frontend/src/components/reader/V4CharacterPanel.vue` |
| Relationship panel | `frontend/src/components/reader/V4RelationshipPanel.vue` |
| Knowledge panel | `frontend/src/components/reader/V4KnowledgePanel.vue` |
| Map panel | `frontend/src/components/reader/V4MapPanel.vue` |
| Quality panel | `frontend/src/components/reader/V4QualityPanel.vue` |
| Legacy mixed AI page | `frontend/src/views/AiBookView.vue` |

## Agent Checklist

开始前：

- 读本文件和 `docs/architecture/ai-book-v4-backend.md`。
- 确认改动属于 V4 frontend，不碰 V3 旧链路，除非用户明确要求。
- 用 GitNexus 查相关符号/组件影响面。
- 对 Vue 改动遵守 Composition API、typed props/emits、props down/events up。

提交前：

- focused tests 通过。
- `cd frontend && npm run build` 通过。
- 如果是视觉改动，做截图/浏览器检查。
- `git diff --check` 通过。
- GitNexus `detect_changes` 确认影响范围符合预期。
