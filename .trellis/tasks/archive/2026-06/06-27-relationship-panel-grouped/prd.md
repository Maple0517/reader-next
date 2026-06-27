# 优化摘要人物关系面板：扩展角色数量与关系分组

## 背景

当前摘要的人物关系面板固定显示 5 个角色（`summaryRelationshipGraph.ts:67` 的 `limit ?? 5`），所有关系以扁平列表展示。当角色数量增多后，扁平列表缺乏结构性，用户难以快速定位特定类型的关系。

## 需求

1. **扩展角色上限**：默认从 5 个提升到 15 个（可配置）
2. **按关系类型分组展示**：在下方 rows 区域，将角色按 `tone`（家族/情感/盟友/冲突/阵营/中立）聚合到分组标题下
3. **分组排序**：每组内按关系强度（critical > strong > moderate > weak > unknown）排序
4. **空组隐藏**：没有角色的分组不显示
5. **图谱区域保持不变**：SVG 圆形图谱仍展示所有角色节点，不做分组

## 数据模型

已有分组依据：
- `SummaryRelationshipTone`: `family | romance | ally | conflict | affiliation | neutral`
- `AiBookRelationStrength`: `critical | strong | moderate | weak | unknown`

无需修改类型定义。

## 改动范围

### `frontend/src/utils/summaryRelationshipGraph.ts`

- `buildSummaryRelationshipGraph` 的 `limit` 默认值改为 15
- 返回类型 `SummaryRelationshipGraphView` 新增 `groupedRows` 字段：
  ```ts
  groupedRows: Array<{
    tone: SummaryRelationshipTone
    label: string  // e.g. '家族', '盟友'
    rows: Array<{ id; name; label; summary; tone; strength }>
  }>
  ```
- 生成逻辑：将 `links` 按 `tone` 分组，组内按 `strength` 排序，空组跳过
- 组顺序：family → romance → ally → conflict → affiliation → neutral

### `frontend/src/components/reader/ChapterSummaryRelationshipPanel.vue`

- 下方 rows 区域改为分组渲染：每组显示 `tone` 对应的标题（如「盟友」「冲突」），标题下方是该组的角色卡片
- 保留原有 `.relationship-row` 样式，新增 `.relationship-group` 和 `.relationship-group-title` 样式
- 组标题带对应 tone 颜色圆点标识

### `frontend/src/views/ReaderView.vue`

- 调用处无需改动（`limit` 不传则使用新默认值 15）

## 验收标准

- [ ] 默认显示最多 15 个角色
- [ ] rows 区域按关系类型分组，每组有标题
- [ ] 组内按关系强度排序
- [ ] 空组不显示
- [ ] SVG 图谱区域行为不变
- [ ] 移动端布局正常
