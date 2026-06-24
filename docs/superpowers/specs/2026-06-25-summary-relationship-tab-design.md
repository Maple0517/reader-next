# AI 摘要窗口：人物关系 tab 设计

## 目标

在现有 AI 摘要窗口里新增一个 tab：`人物关系`。它不是完整 AI资料页，也不是人物列表；它只回答读者阅读当前章节时最需要的问题：主角现在和谁有关，这些关系最近是什么状态。

## MVP 范围

采用「图 + 说明列表」方案：

- 图：主角居中，周围展示与主角关系最强/最近的角色。
- 线：按关系类型着色，同一角色与主角有多种关系时聚合成一条线。
- 列表：补充图上放不下的语义，例如关系类型、最近动态、关系摘要。

第一版只做主角的一跳关系，不做全书关系图、多主角切换、拖拽、缩放、搜索、后端 schema 改动。

## 数据来源

复用现有 AI资料数据：

- `AiBookMemoryViewModel.characters`
- `AiBookMemoryViewModel.relationships`
- 之后如需要，可补用 `AiBookChapterMemoryViewModel.digest` 做近期增强，但 MVP 先不强依赖。

主角识别不新增字段：

1. 统计每个角色出现在关系中的次数。
2. 分数加权：关系次数优先，其次 `importance: high`，再次 `lastSeenChapterIndex` 接近当前章节。
3. 分数最高者视为主角。

这符合当前约束：出现最多的就是主角。后续如果后端提供 `protagonistId`，再替换 selector，不改 UI。

## 角色筛选

只展示与主角直接相关的角色。

排序规则：

1. 最近更新章节更接近当前章节。
2. 关系强度更高：`critical > strong > moderate > weak > unknown`。
3. 关系状态更值得关注：`developing` 优先。
4. evidence/history/currentDynamics 越多，越靠前。

默认取 6 个，最多 8 个。低频亲属/路人不会出现，除非最近有关系变化或关系强度较高。

## 关系聚合

同一个角色与主角之间只显示一个边。

聚合方式：

- label：优先使用 `facets` 的 kind/subtype，再退到 relationship `label` / `kind`。
- summary：优先使用 `currentDynamics[0]`，再退到 `summary`。
- polarity：决定线色倾向；mixed/unknown 使用灰色。
- 多关系合并为短标签，例如：`家族 / 保护`、`敌对 / 临时合作`。

关系明细放在说明列表，不塞进 SVG 线标签。

## UI 结构

在 AI 摘要窗口 tab 区新增：`人物关系`。

内容：

1. 空状态
   - 无 AI资料：提示「暂无人物关系资料，可先生成 AI资料」。
   - 有资料但关系不足：提示「人物关系不足，继续阅读后会补全」。

2. 主角中心图
   - 中心节点：主角，视觉更大。
   - 周围节点：最多 6-8 个相关角色，圆形放射布局。
   - 连线颜色：
     - family：蓝
     - romance：粉
     - friendship/alliance：绿
     - conflict/rivalry：红
     - affiliation/supervision：紫
     - unknown/mixed：灰

3. 说明列表
   - 每行一个相关角色：`角色名｜聚合关系｜最近动态`
   - 示例：`李青｜盟友｜最近：共同调查遗迹`
   - 示例：`城主府｜压力来源｜最近：开始关注张羽`

## 实现边界

最小实现：

- 新增 selector utility：把 AI资料 memory 转成关系图 view model。
- `ReaderView.vue` 加载 `getAiBookMemory(bookUrl)`。
- AI 摘要窗口新增 `人物关系` tab。
- 用原生 SVG + HTML 列表渲染，不引入图表库。
- inline / continuous / side 三处摘要面板都要有同一个 tab。

不做：

- 后端改动
- 图布局库
- 拖拽/缩放
- 点击展开详情
- 全人物关系图
- AI 重新生成文案

## 验证

最小测试：

- selector 能识别关系数最多的主角。
- selector 能筛出与主角直接相关的角色。
- selector 能把同一对角色的多关系聚合为一条边。
- selector 会按最近/强度/变化排序，限制数量。

命令：

```bash
cd frontend && npm test -- --run aiBookRelationshipGraph
cd frontend && npm run build
```

## 风险

- 主角识别是启发式，不是强语义；但符合 MVP 和当前数据限制。
- 如果 AI资料本身缺关系，图会空；空状态必须明确引导生成/补齐 AI资料。
- 关系类型颜色需要克制，避免破坏阅读页原有风格。
