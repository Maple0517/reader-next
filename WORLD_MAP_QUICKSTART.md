# World Map Spec - 快速上手

## 🎯 一句话概括

从小说原文自动生成结构化、可追溯、高置信度的世界地图规格书，85-90% 的工作由 AI 自动完成。

## 📁 核心文件（优先阅读顺序）

1. **WORLD_MAP_INTEGRATION_SUMMARY.md** - 集成总结（5分钟）
2. **WORLD_MAP_DESIGN.md** - 完整设计（30分钟）
3. **WORLD_MAP_TODO.md** - 实施计划（10分钟）
4. **prompts/world_map_spec_architect.md** - AI Prompt（15分钟）
5. **src/model/world_map.rs** - 数据结构（代码）

## 🚀 快速开始

### 选项 A：实现 MVP（推荐）

验证可行性，4-5天完成：

```bash
# 1. 实现存储层
touch src/service/world_map_storage.rs

# 2. 实现简化推理引擎（只要前3条冲突解决规则）
touch src/service/world_map_inference.rs

# 3. 实现简单坐标生成（启发式布局，无需优化器）
touch src/service/world_map_optimizer.rs

# 4. 实现基础 API（GET/POST spec）
touch src/api/handlers/world_map.rs

# 5. 前端简单展示（实体列表 + SVG 画布）
touch frontend/src/components/WorldMapCanvas.vue
```

**MVP 验证目标**：
- ✓ 能从真实小说提取结构化 spec
- ✓ 冲突解决规则有效
- ✓ 坐标生成产生合理布局
- ✓ 人工审查量 < 20%

### 选项 B：完整实现

按 Phase 1-7 依次实现，12天完成：
- Phase 1: 存储层（1天）
- Phase 2: 推理引擎（2天）
- Phase 3: 坐标优化器（2天）
- Phase 4: 构建服务（2天）
- Phase 5: API 层（1天）
- Phase 6: 前端（3天）
- Phase 7: 测试文档（1天）

## 🧩 关键概念

### 证据等级
```
A - 原文直接说明（"阿尔托位于黑暗山脉以南"）
B - 原文明显暗示（"他们向北走了三天到达..."）
C - 多条信息共同约束
Unknown - 原文未说明
Conflict - 原文存在冲突
```

### 约束类型
```
Hard - 不可违反（evidence_level = A）
Soft - 辅助布局（evidence_level = B/C）
```

### 置信度分级
```
Fixed (0.85-1.0) - 硬约束确定
Relative (0.60-0.84) - 相对位置
Tentative (0.40-0.59) - 临时布局
Forbidden - 信息不足，不放置
Unresolved - 存在冲突，无法放置
```

### 自动化率目标
```
实体提取: 98%
关系提取: 90%
冲突解决: 80%
位置推理: 70%
坐标生成: 85%
总体: 85-90%
```

## 🔄 典型工作流

```
用户请求"更新到第50章"
    ↓
1. Extract - AI 从章节提取实体和关系（JSONL）
    ↓
2. Resolve - AI 自动解决冲突（8种规则）
    ↓
3. Infer - AI 推理位置（5种策略，置信度加权）
    ↓
4. Coordinate - 约束优化器生成坐标
    ↓
5. Validate - 自验证循环（3轮迭代修正）
    ↓
6. Review - 生成审查清单（只包含低置信度项）
    ↓
返回 WorldMapSpec + ReviewItems（用户只需审查 10-15%）
```

## 💡 设计亮点

1. **忠于原文** - 每条信息都有原文引用
2. **保留冲突** - 不静默修复矛盾
3. **置信度驱动** - 用户清楚知道哪些可信
4. **自动化优先** - AI 自己解决 85-90%
5. **人工可介入** - 低置信度项目进入清单
6. **增量更新** - 新章节自动合并
7. **版本控制** - JSONL + Git

## 📊 数据结构速查

```rust
WorldMapSpec {
    metadata: WorldMapMetadata,           // 元数据
    entities: Vec<WorldMapEntity>,        // 实体（城市、山脉）
    relations: Vec<WorldMapRelation>,     // 关系（方位、邻接）
    routes: Vec<WorldMapRoute>,           // 路线（商路、航线）
    factions: Vec<WorldMapFaction>,       // 势力范围
    constraints: WorldMapConstraints,     // 约束（Hard/Soft）
    conflicts: Vec<WorldMapConflict>,     // 冲突记录
    coordinates: Option<WorldMapCoordinates>, // 坐标草案
    review_items: Vec<WorldMapReviewItem>,    // 审查清单
    statistics: WorldMapStatistics,           // 统计数据
}
```

## 🛠️ API 端点

```
GET  /reader3/ai/worldMap                  # 获取规格书
POST /reader3/ai/worldMap                  # 保存规格书
POST /reader3/ai/worldMap/update           # 增量更新
POST /reader3/ai/worldMap/generateCoordinates  # 生成坐标
GET  /reader3/ai/worldMap/reviewItems      # 获取审查清单
POST /reader3/ai/worldMap/resolve          # 人工修正
POST /reader3/ai/worldMap/render           # 绘制地图图片
```

## 🎨 前端组件

```
AiBookView.vue (地图 tab 升级)
    ├── WorldMapCanvas.vue       # 交互式 SVG 画布
    ├── MapEntityPanel.vue       # 实体列表
    └── MapReviewPanel.vue       # 审查面板
```

## 📝 第一个 PR

**推荐起点**：实现存储层

```bash
# 1. 创建文件
touch src/service/world_map_storage.rs

# 2. 实现 WorldMapStorage
# - load() / save() / save_incremental()
# - JSONL 读写工具函数
# - 单元测试

# 3. 集成到 AppState
# 修改 src/app/state.rs

# 4. 测试
cargo test world_map_storage

# 5. 提交
git add src/service/world_map_storage.rs
git commit -m "feat(world-map): 实现存储层"
```

## 🤔 常见问题

**Q: 为什么不直接让 AI 生成地图图片？**  
A: 图片无法追溯、修正、版本控制。先建立结构化规格书，再基于规格书生成图片。

**Q: 如果原文冲突很多怎么办？**  
A: 保留冲突记录，AI 给出解决建议（prefer_a/prefer_b），最终由用户决定。

**Q: 坐标生成失败怎么办？**  
A: 标记为 `Blocked` 状态，明确说明原因（信息不足/约束冲突），不强行生成。

**Q: 人工审查会不会很累？**  
A: 系统只会让你审查真正需要人工判断的 10-15%，且提供 AI 建议和原文证据。

**Q: 如何保证地图准确？**  
A: 不保证 100% 准确，但保证：1) 每条信息有原文出处 2) 置信度明确标记 3) 冲突不隐藏。

## 📚 延伸阅读

- **原始讨论** - `/Users/maple/Downloads/world_map_spec_architect_prompt.md`
- **Reader 架构** - `AGENTS.md`
- **AI资料现状** - `src/model/ai_book.rs`

## 🎉 开始实施

```bash
# 阅读设计文档
cat WORLD_MAP_DESIGN.md

# 查看任务清单
cat WORLD_MAP_TODO.md

# 开始第一个 PR
touch src/service/world_map_storage.rs
```

Good luck! 🚀
