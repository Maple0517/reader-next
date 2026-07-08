# V4 Console Redesign Spec

## 目标

将 V4 前端从现代 dashboard 风格重做为**克制暖色调数据库控制台**，对标 `recovered/v4-frontend-focus/visual-companion/index.html` 中七个 rail page 的具体设计。采用 **V4 独立设计系统**（`--v4-*` token），不污染 V3。

## Non-Goals

- 不碰 V3 页面。
- 不新增后端 API（当前 API 不变，后端补数据后可增量接入）。
- 不引入新 npm 依赖（graph library、motion library 等）。
- Identity 不作为一级 rail tab（已确认）。

---

## 1. V4 设计 Token 系统

新建 `frontend/src/styles/v4-console.css`，定义 V4 控制台专属变量。所有 V4 组件 CSS **只用 `--v4-*`**，不混用全局 `--color-*`。

### 1.1 色板

```css
:root {
  /* 基础色 */
  --v4-bg: #f5f1ea;           /* 页面底色：暖米 */
  --v4-surface: #fffdf9;      /* 卡片/面板：白纸 */
  --v4-line: #d8d0c3;         /* 边框线：暖灰 */
  --v4-ink: #181612;          /* 主文字：深墨 */
  --v4-muted: #655f55;        /* 次要文字：暖灰 */
  --v4-soft: #ebe4d8;         /* 软底色：浅暖 */
  --v4-inspector-bg: #fffaf2; /* 详情面板底色：微黄 */
  --v4-rail-bg: #eee6d8;      /* Rail 背景 */
  --v4-shell-top-bg: #f0ebe3; /* Shell 顶部条（surface 80% + soft 20%） */

  /* 功能色 */
  --v4-accent: #3657d6;       /* 主强调：蓝 */
  --v4-success: #26704f;      /* 成功/完成：绿 */
  --v4-warn: #9a6416;         /* 警告/注意：琥珀 */
  --v4-danger: #b83a3a;       /* 错误/失败：红 */
  --v4-purple: #6550b9;       /* 特殊标记：紫 */

  /* 间距 */
  --v4-radius: 0px;
  --v4-gap: 6px;
  --v4-pad: 8px;
  --v4-pad-lg: 10px;
  --v4-line-height: 1.45;
}
```

### 1.2 通用组件样式

**Pill**（status chip）：
```css
.v4-pill {
  display: inline-flex; align-items: center;
  min-height: 24px; padding: 0 9px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  color: var(--v4-muted);
  font-size: 12px; font-weight: 750; white-space: nowrap;
}
.v4-pill.blue { color: var(--v4-accent); border-color: color-mix(in srgb, var(--v4-accent) 34%, var(--v4-line)); }
.v4-pill.green { color: var(--v4-success); border-color: color-mix(in srgb, var(--v4-success) 34%, var(--v4-line)); }
.v4-pill.amber { color: var(--v4-warn); border-color: color-mix(in srgb, var(--v4-warn) 34%, var(--v4-line)); }
.v4-pill.red { color: var(--v4-danger); border-color: color-mix(in srgb, var(--v4-danger) 34%, var(--v4-line)); }
```

**Icon Button**：
```css
.v4-icon-btn {
  width: 28px; height: 24px;
  display: grid; place-items: center;
  border: 1px solid var(--v4-line); background: var(--v4-surface);
  color: var(--v4-muted); font-size: 12px; font-weight: 850;
  cursor: pointer;
}
```

**Action Button**：
```css
.v4-action-btn {
  border: 1px solid var(--v4-line); background: var(--v4-surface);
  min-height: 28px; padding: 0 9px;
  font-size: 11px; font-weight: 850; color: var(--v4-ink);
  cursor: pointer;
}
.v4-action-btn.primary {
  border-color: color-mix(in srgb, var(--v4-accent) 48%, var(--v4-line));
  color: var(--v4-accent);
  background: color-mix(in srgb, var(--v4-accent) 7%, var(--v4-surface));
}
```

**Evidence Block**：
```css
.v4-evidence {
  border-left: 3px solid var(--v4-success);
  background: color-mix(in srgb, var(--v4-success) 8%, var(--v4-surface));
  padding: 8px 9px; font-size: 11px; color: var(--v4-muted);
}
```

**Field Card**（inspector 内部字段卡）：
```css
.v4-field {
  border: 1px solid var(--v4-line); background: var(--v4-surface);
  padding: 8px; min-height: 54px;
}
.v4-field label { display: block; color: var(--v4-muted); font-size: 10px; margin-bottom: 3px; }
.v4-field strong { font-size: 12px; }
```

---

## 2. Shell 结构

### 2.1 V4ConsoleShell

**现状**：有 border + 8px 圆角 + 内阴影，header 和 body 之间有 gap。

**目标**：纯 layout 容器，无装饰。结构：

```
┌─────────────────────────────────────────────────────┐
│ 《书名》· V4 Memory Console    ⟳  ▶  ⋯             │ ← v4-top-bar
│ 已读 N 章 · 已处理 N 章 · 当前 N 章    [action btns] │ ← status + actions
├──────────┬──────────────────────────────────────────┤
│ 总览     │                                          │
│ 任务 ·…  │   ← #default slot (panel content)       │
│ 角色 ·…  │                                          │
│ 关系 ·…  │                                          │
│ 知识 ·…  │                                          │
│ 地点 ·…  │                                          │
│ 质量 ·…  │                                          │
└──────────┴──────────────────────────────────────────┘
```

CSS：
- `.v4-console`：`display: grid; grid-template-rows: auto 1fr;`，无 border/radius/background
- `.v4-top-bar`：`display: grid; grid-template-columns: 1fr auto; align-items: center; padding: 10px; border-bottom: 1px solid var(--v4-line); background: var(--v4-shell-top-bg);`
- `.v4-console-body`：`display: grid; grid-template-columns: 116px 1fr; min-height: 0;`
- Rail 列：`border-right: 1px solid var(--v4-line); background: var(--v4-rail-bg); padding: 8px;`

Top-bar 内容由 `AiBookV4View` 通过 `#header` slot 注入，包含：
- 左侧：书名标题 + subtitle（作者 + 处理状态描述）
- 右侧：status pills + action buttons（刷新状态/生成当前章节/补齐到当前阅读/重置）

**合并当前 masthead**：AiBookV4View 的 `.v4-console-masthead` 区域移入 shell 的 header slot，样式改为 top-bar 平铺（不再是独立 card）。

### 2.2 V4DomainRail

**现状**：支持 `meta` 但 AiBookV4View 没传 meta。

**目标**：
- Rail item 两行：label（12px, font-weight: 800）+ meta（10px, var(--v4-muted)）
- Active 状态：`border-color: var(--v4-accent); color: var(--v4-accent); font-weight: 850;`
- AiBookV4View 传入带 meta 的 railItems：

```ts
const railItems = [
  { key: 'overview', label: '总览', meta: 'health' },
  { key: 'task', label: '任务', meta: computed(() => catchupStatus.value?.status === 'running' ? `running · ${progressPercent}%` : 'idle') },
  { key: 'characters', label: '角色', meta: computed(() => `${characterCount} canonical`) },
  { key: 'relationships', label: '关系', meta: computed(() => `${edgeCount} edges`) },
  { key: 'knowledge', label: '知识', meta: computed(() => `${factCount} facts`) },
  { key: 'map', label: '地点', meta: computed(() => `${placeCount} places`) },
  { key: 'quality', label: '质量', meta: computed(() => `${queueCount} queues`) },
]
```

Rail 宽度 `116px`，背景 `var(--v4-rail-bg)`。移动端（<820px）横向滚动。

### 2.3 V4PanelShell

**现状**：有 border + 8px 圆角 + background，每个 panel 被包了一层 card。

**目标**：去掉外层装饰，变成纯 layout 容器。只负责：
1. Panel header（title + subtitle + toolbar slot），带 `border-bottom: 1px solid var(--v4-line)`
2. Loading / Error / Empty 状态切换
3. Default slot

去掉 `.v4-panel-shell` 的 border/radius/background。Panel header 样式匹配设计稿的 `screen-header`：`display: grid; grid-template-columns: 1fr auto;`。

---

## 3. 七面板设计

所有面板使用 `<script setup lang="ts">`，遵循 Vue 最佳实践：
- `shallowRef` 管理 primitive 状态
- `ref` 用于需要整体替换的对象
- `computed` 派生所有展示数据
- Watcher 只做 side effect
- Props down, events up

### 3.1 总览 / Overview

对标设计稿 Rail Pages → 总览/Overview 区域（index.html L2260-2352）。

**布局**：三段垂直堆叠

1. **Live Core**（进度卡）：
   - 两行 core-row：左 big number（如 `42%`）+ 描述文字；右 pill（如 `Domain Decision`）
   - 中间 progress-track + progress-fill（蓝紫渐变 + shimmer 动画）
   - 底 core-row：最近完成信息 + 失败数
   - 仅在有运行中或最近任务时显示

2. **overview-grid**（`grid-template-columns: 1.3fr 0.9fr`）：
   - 左：**Domain Health** panel
     - panel-title：`<h3>Domain Health</h3>` + pill `canonical ready`
     - domain-health-grid（4 格 grid）：角色/关系/知识/地点，每格含 `<span>标签</span>` + `<strong>数字</strong>` + `<span>状态描述</span>` + `spark` 装饰线
   - 右：**需要注意** panel
     - panel-title：`<h3>需要注意</h3>` + pill `N items`
     - attention-list：每条 attention-row 含 status-dot（蓝/红/amber/绿）+ 文字 + chip tag

**数据源**：不变（`getV4Memory` + `getV4MemoryStatus` + `getV4CatchupStatus` + `getV4Map` + `getV4Quality`）

**变更清单**：
- 替换圆形状态指示器 → live-core 进度条样式
- 替换 5 格 MetricCard → 4 格 domain-card + spark 装饰
- 替换 `<li>` 文字列表 → attention-row 带状态圆点
- 去掉质量格（质量在 rail 有独立入口）

### 3.2 任务 / Task Progress

对标设计稿 Rail Pages → 任务/Task Progress + 推荐混合方案（index.html L2120-2252, L2354-2440）。

**布局**：三段垂直堆叠

1. **task-hero**（`grid-template-columns: 1fr 260px`）：
   - 左 task-hero-main：h2 标题 + p 描述 + action-row 按钮（刷新状态/取消任务/查看 claims）
   - 右 task-hero-meter：label + big number + progress-track + 更新时间，带 scan 动画

2. **task-main-grid**（`grid-template-columns: minmax(420px, 1fr) 340px`）：
   - 左：**Pipeline Timeline**
     - h3 标题
     - stage-large 行列表（每行：18px 圆点 + 名称描述 + 状态标签）
     - 5 推导阶段：状态读取 → AI 输出解析 → Domain Decision → Canonical 写入 → Projection 更新
     - 状态样式：done=绿点、active=蓝点+脉冲、failed=红点
   - 右：**Stage Inspector**（error-inspector 样式）
     - 红色边框面板
     - detail-line 字段（阶段/状态/失败历史等）
     - 代码块（隔离项信息或错误详情）
     - action-row 操作按钮

3. **heatmap-wide**（底部）：
   - h3 `Compressed Chapter Navigator`
   - 章节色彩矩阵（32 列 grid）：绿成功/蓝当前/红失败/灰等待
   - 底部说明文字
   - 当后端无 per-chapter API 时，用 processed/current/target 边界推导范围可视化

**数据源**：不变

**变更清单**：
- 替换圆环进度 → task-hero 双栏布局
- 替换 V4InspectorSection 折叠 → 右侧常驻 error-inspector
- 新增底部 mini heatmap
- Pipeline 阶段视觉改为 stage-large 样式

### 3.3 角色 / Characters

对标设计稿 Rail Pages → 角色/Characters（index.html L2442-2524）。

**布局**：directory-layout（`grid-template-columns: minmax(270px, 34%) 1fr`）

1. 左 list-panel：
   - **搜索栏**：`<div class="search">搜索角色 / 别名 / 状态</div>` + icon-btn `⌕`。前端本地 filter，匹配 name/aliases。
   - 行列表：每行 row 含 row-main（名字 + chip 可见度/身份线索）+ row-meta（别数/首见/最近/证据）
   - Active 行：`border-color: var(--v4-accent); background: blue 8%`
   - Progressive disclosure：默认显示前 12 个，"显示其余 N 位角色" toggle

2. 右 detail-panel（`background: var(--v4-inspector-bg)`）：
   - inspector-title：名字 + 状态标签行（canonical/visibility/accepted）+ chip 别名 + chip 身份稳定
   - field-grid（2x2）：当前状态/章节范围/关系摘要/治理状态
   - **identity-thread**（`border: 1px solid purple 42%; background: purple 7%`）：
     - h3 `身份线索`
     - detail-line：稳定链接 / 待确认 / 处理入口
   - evidence block（绿色左边框）

**数据源**：不变

**变更清单**：
- **新增搜索栏**（前端本地过滤）
- 替换大型圆角 avatar card → 紧凑 row 样式
- 替换圆角紫调详情卡 → 暖黄底 + 锐利边 detail-panel
- **新增 identity-thread section**
- 保留状态矩阵分组（已匹配设计）

### 3.4 关系 / Relationships

对标设计稿 Rail Pages → 关系/Relationships（index.html L2526-2596）。

**布局**：relationship-layout（`grid-template-columns: 1fr 330px`）

1. 左 graph-panel（`min-height: 420px`）：
   - **可视化关系图**：CSS 背景网格线 + 绝对定位 node + edge-line + edge-label
   - node：`border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 8px 10px;` + `box-shadow`
   - edge-line：绝对定位 2px 高度条，旋转角度连接两点
   - edge-label：蓝色文字小标签，显示关系类型和置信度
   - 布局用确定性前端计算（无需 graph library）

2. 右 detail-panel：
   - panel-title：`<h3>选中关系</h3>` + pill（关系类型）
   - field：Edge（源→目标）+ 语义
   - field-grid：置信度 + 事件数
   - evidence block
   - fact-stack：family 组提示 + 可操作链接

**数据源**：不变

**变更清单**：
- 替换 node-chip cloud → 可视化关系图（绝对定位 node + CSS line）
- Group filter pills 和边列表保留
- 右侧 inspector 保留，视觉微调

### 3.5 知识 / Knowledge

对标设计稿 Rail Pages → 知识/Knowledge（index.html L2598-2686）。

**布局**：knowledge-layout（`grid-template-columns: 1fr 330px`）

1. 左 wide-panel：
   - panel-title：`<h3>Knowledge Atlas</h3>` + pill `N categories`
   - domain-health-grid（4 格）：修炼体系/势力结构/世界规则/秘密预言，每格含主题名/数量/描述/spark
   - fact-stack：高价值 topic cards（主题摘要 + 状态描述）
   - **Category nav** 按钮行保留（全部 + 各分类）

2. 右 detail-panel：
   - panel-title：`<h3>Topic Inspector</h3>` + pill（分类）
   - field：主题 + 当前摘要
   - field-grid：断言数 + 最近更新
   - evidence block
   - fact-stack：断言时间线（按章节，每条含状态描述）

**数据源**：不变

**变更清单**：
- 分类 nav 前加 4 格 category overview cards
- Topic Inspector 视觉微调（去掉圆角/阴影 → 锐利边暖黄底）

### 3.6 地点 / Places

对标设计稿 Rail Pages → 地点/Places（index.html L2688-2756）。

**布局**：place-layout（`grid-template-columns: 1fr 330px`）

1. 左 map-panel（`min-height: 420px`）：
   - 空间示意：背景渐变 + 绝对定位 place-pin + route-line + map-label
   - place-pin：14px 圆形，`border: 3px solid var(--v4-surface); box-shadow`
   - route-line：2px 高度条，连接地点
   - map-label：边框标签显示地名
   - 保留当前 SVG 拓扑图作为默认视图，设计稿空间示意作为增强

2. 右 detail-panel：
   - panel-title：`<h3>地点 Inspector</h3>` + pill
   - field：地点名
   - field-grid：出现章节 + 关联事实
   - fact-stack：空间规则 + 最近事件 + 关联角色
   - evidence block（Projection 刷新摘要）

**数据源**：不变

**变更清单**：
- 拓扑图保留，增加空间视觉元素（place-pin + route-line 样式）
- Place Inspector 视觉微调

### 3.7 质量 / Quality

对标设计稿 Rail Pages → 质量/Quality（index.html L2758-2876）。

**布局**：quality-layout（`grid-template-columns: 220px 1fr 330px`）

1. 左 queue-panel（`background: var(--v4-inspector-bg)`）：
   - panel-title：`<h3>Queues</h3>` + pill `N open`
   - queue-row 列表：名字 + chip 计数 + meta 描述
   - Active 行：红色边框 + 红底

2. 中 triage-list：
   - panel-title：h3 队列名 + pill `needs review`
   - triage-row 列表：severity badge（H/M/L 颜色区分）+ triage-copy（标题+描述）+ chip domain
   - Active 行：红色边框

3. 右 quality-inspector（`border: 1px solid red 38%; background: red 5%`）：
   - error-title：h3 `Review Inspector` + pill `quarantined`
   - field：问题描述
   - field-grid：Domain + 章节
   - detail-line：Decision + 原因
   - error-code block（claim/judge/action/canonical_write）
   - evidence block
   - fact-stack：边界提示（修正必须走后端 API）

**数据源**：不变

**变更清单**：
- 结构已对齐，主要是视觉微调（去掉圆角/阴影 → 锐利边暖色调）

---

## 4. 组件架构

### 4.1 组件职责表

| 文件 | 职责 | 变更类型 |
| ---| --- | --- |
| `views/AiBookV4View.vue` | Route orchestration + shell composition | 重构：masthead 移入 shell header slot，railItems 加 meta |
| `v4/V4ConsoleShell.vue` | 顶层 layout（header + rail + main） | 重构：去掉装饰，改为纯 layout |
| `v4/V4DomainRail.vue` | Rail 渲染 + active 选中 | 重写样式 |
| `v4/V4PanelShell.vue` | Panel 容器 + 状态切换 | 重构：去掉外层 border/radius |
| `v4/V4MetricCard.vue` | 指标卡 | 重写样式 |
| `v4/V4ConfidenceBadge.vue` | 置信度显示 | 微调 |
| `v4/V4InspectorSection.vue` | Inspector 折叠区 | 不变 |
| `V4BookOverviewPanel.vue` | 总览面板 | 重构布局 |
| `V4TaskProgressPanel.vue` | 任务面板 | 重构布局 |
| `V4CharacterPanel.vue` | 角色面板 | 重构布局 + 新增搜索/identity-thread |
| `V4RelationshipPanel.vue` | 关系面板 | 重构：新增可视化图 |
| `V4KnowledgePanel.vue` | 知识面板 | 微调：新增 category overview cards |
| `V4MapPanel.vue` | 地点面板 | 微调 |
| `V4QualityPanel.vue` | 质量面板 | 微调 |
| `styles/v4-console.css` | V4 设计 token | **新建** |

### 4.2 数据流规则（Vue Best Practices）

- `shallowRef` 用于 primitive 状态（`activeDomain`、`selectedCharacterId`、`loading` 等）
- `ref` 用于需要整体替换的对象（`detail`、`memoryStatus` 等）
- `computed` 派生所有展示数据（`groupedStateSections`、`filteredCharacters`、`railItems` 等）
- Watcher 只做 side effect（`bookUrl` 变化 → reload），不做数据转换
- Panel 通过 `bookUrl` prop 拉取自己的数据，不修改父状态
- 搜索过滤在 panel 内用 `computed` 实现，不新增 API

### 4.3 Rail Meta 数据流

Rail meta 需要从各 panel 的数据中聚合。方案：
- AiBookV4View 加载 `memoryStatus` 和 `catchupStatus` 后，用 `computed` 生成 railItems（带 meta）
- Panel 数据（角色数/关系数等）通过面板 load 后 emit 一个 `counts` 事件，或通过共享 composable 提供
- 第一版简化：rail meta 只用 AiBookV4View 已有的 `memoryStatus` 数据（角色数/关系数/知识数/地点数），质量队列数用 quality API 一次获取

---

## 5. 动画

使用纯 CSS 动画（设计稿已定义），不引入 motion 依赖：

- `@keyframes shimmer`：进度条填充的光效
- `@keyframes scan`：hero meter 的扫描线
- `@keyframes pulse`：active 阶段圆点的呼吸效果
- `@keyframes rotate`：（仅 Mission Control 的雷达指针，本轮不实现）

---

## 6. 测试策略

每个面板的 test 验证：
1. Rail 不再渲染 Identity 作为一级 item
2. Rail 显示 meta 状态摘要
3. 概览渲染 live-core 进度条 + domain health 4 格 + attention list 带状态圆点
4. 任务渲染 hero + pipeline stages + error inspector + mini heatmap
5. 角色渲染搜索栏 + 列表 + detail with state matrix + identity-thread
6. 关系渲染可视化图（node + edge-line）+ edge inspector
7. 知识渲染 category overview + topic cards + inspector
8. 地点渲染拓扑图 + place inspector
9. 质量渲染 queue + triage + inspector

验证命令：
```bash
cd frontend && npm test -- V4
cd frontend && npm run build
```

---

## 7. 实施顺序

1. 新建 `v4-console.css` token 系统
2. 重构 V4ConsoleShell（去掉装饰 → 纯 layout）
3. 重构 V4DomainRail（样式 + meta）
4. 重构 V4PanelShell（去掉外层 card）
5. 重构 AiBookV4View（masthead 合并 + railItems 加 meta）
6. 重写 V4BookOverviewPanel
7. 重写 V4TaskProgressPanel
8. 重写 V4CharacterPanel（+ 搜索 + identity-thread）
9. 重写 V4RelationshipPanel（+ 可视化图）
10. 微调 V4KnowledgePanel（+ category overview）
11. 微调 V4MapPanel
12. 微调 V4QualityPanel
13. 跑测试 + build 验证
