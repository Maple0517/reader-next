# World Map Spec 集成完成总结

## 🎯 任务目标

将我们从头到尾讨论的小说世界地图架构设计，完整集成进 reader 项目的 AI资料模块。

## ✅ 已完成工作

### 1. 核心数据结构（src/model/world_map.rs）

创建了完整的类型系统，包括：

**主结构**：
- `WorldMapSpec` - 地图规格书主体
- `WorldMapMetadata` - 元数据（版本、章节范围、统计）
- `WorldMapEntity` - 地理实体（城市、山脉、河流等）
- `WorldMapRelation` - 空间关系（方位、邻接、包含、阻隔）
- `WorldMapRoute` - 路线/交通
- `WorldMapFaction` - 势力范围
- `WorldMapConstraints` - 约束系统（Hard/Soft）
- `WorldMapConflict` - 冲突记录
- `WorldMapCoordinates` - 坐标草案
- `WorldMapReviewItem` - 人工审查清单
- `WorldMapStatistics` - 自动化统计

**证据系统**：
- `Evidence` - 原文证据
- `EvidenceLevel` - 证据等级（A/B/C/Unknown/Conflict）

**枚举类型**：
- `EntityType` - 实体类型（Settlement/Region/Terrain/Water/Transit/Fantasy）
- `RelationType` - 关系类型（Direction/Nearby/Contains/Blocks/Route）
- `Direction` - 方向（8方位）
- `ConstraintType` - 约束类型（Hard/Soft）
- `ResolutionHint` - 冲突解决建议
- `CoordinateConfidence` - 坐标置信度
- 等

### 2. 设计文档（WORLD_MAP_DESIGN.md）

完整的技术设计文档，包含：

**核心原则**：
- 忠于原文 > 保留冲突 > 空间逻辑 > 地图美观
- 证据分级系统（A/B/C/Unknown/Conflict）
- 自动化优先（85-90% 自动化率目标）
- 置信度驱动（低于阈值才需要人工）

**数据结构详解**：
- 所有结构体的字段说明
- 枚举类型的使用场景
- 证据系统的运作机制

**工作流程（6个阶段）**：
1. Extract - 提取实体和关系
2. Resolve - 冲突自动解决
3. Infer - 位置推理
4. Coordinate - 坐标生成
5. Validate - 自验证循环
6. Review - 生成审查清单

**技术实现**：
- 存储方案（JSONL + Git vs SQLite）
- API 设计（7个端点）
- Service 架构（4个核心服务）
- 前端设计（组件划分）

**AI Prompt 模板**：
- 提取实体 Prompt
- 提取关系 Prompt
- 冲突解决 Prompt

**预期效果**：
- 各模块自动化率
- 不同小说类型的地图质量
- 人工工作量估算

### 3. AI Prompt 规格书（prompts/world_map_spec_architect.md）

完整的 AI 系统 Prompt，定义了：

**第一原则**：
- 忠于原文 > 保留冲突与未知 > 空间逻辑一致
- 禁止脑补、修改、补全原文

**证据等级规则**：
- A级：原文直接说明
- B级：原文明显暗示
- C级：多条信息共同约束
- Unknown/Conflict：必须保留

**工作流程（8步）**：
1. 提取地理实体
2. 提取空间关系
3. 检测冲突
4. 位置推理
5. 生成约束
6. 坐标生成
7. 生成审查清单
8. 生成统计数据

**自动解决规则**：
- 后文优先（置信度 0.75）
- 详细优先（置信度 0.70）
- 主角视角优先（置信度 0.80）
- 精确方位优先（置信度 0.75）
- 距离一致性（置信度 0.65）
- 无法判断 → 人工审查

**输出格式**：
- JSONL 格式示例
- 完整 JSON 对象结构

### 4. 实施计划（WORLD_MAP_TODO.md）

详细的开发任务清单，包括：

**7个 Phase**（共 12 天）：
- Phase 1: 存储层（1天）
- Phase 2: 推理引擎（2天）
- Phase 3: 坐标优化器（2天）
- Phase 4: 构建服务（2天）
- Phase 5: API 层（1天）
- Phase 6: 前端（3天）
- Phase 7: 测试文档（1天）

**MVP 方案**：
- 最小可行产品范围
- 4-5 天快速验证
- 验证目标明确

**技术选型待定**：
- 坐标求解器（Rust vs argmin vs scipy）
- 向量检索（可选）
- 地图绘制（AI vs 程序化）

**下一步行动**：
- 推荐先实现 MVP 验证可行性
- 第一个 PR：存储层 + 单元测试

## 📊 架构特点

### 1. 结构化 + 证据驱动

每条信息都有：
- 原文引用（quote）
- 章节位置（chapter）
- 证据等级（A/B/C/Unknown/Conflict）
- 置信度分数（0.0-1.0）

### 2. 自动化 + 人工审查

**自动化（85-90%）**：
- 冲突解决：8种规则自动判断
- 位置推理：5种策略加权计算
- 坐标生成：约束优化求解
- 自验证：3轮迭代修正

**人工介入（10-15%）**：
- 只审查低置信度项（< 0.60）
- 明确的审查清单（issue + AI建议）
- 优先级排序（High/Medium/Low）

### 3. 增量更新 + 版本控制

- JSONL 格式支持增量追加
- Git 版本控制变更历史
- Delta merge 策略
- 冲突检测和合并

### 4. 可扩展 + 模块化

**数据层**：
- JSONL 文件（初期）
- 可升级到 SQLite（性能优化）
- 可加入向量检索（语义搜索）

**Service 层**：
- WorldMapStorage - 存储抽象
- WorldMapInferenceEngine - 推理引擎
- WorldMapOptimizer - 坐标优化
- WorldMapBuilderService - 构建服务

*- RESTful 设计
- 7个端点覆盖完整流程
- 与现有 AiBookMemory 集成

**前端层**：
- 交互式 SVG 画布
- 置信度可视化（颜色标记）
- 人工审查面板
- 增量更新 UI

## 🔄 与现有代码的集成点

### 1. 数据模型扩展

现有：
```rust
pub struct AiBookMemory {
    pub map: Option<AiBookMap>,  // 简单的 imageUrl + prompt
    pub map_dirty: bool,
}
```

升级后：
```rust
pub struct AiBookMemory {
    pub map: Option<AiBookMapLegacy>,  // 保留向后兼容
    pub world_map_spec: Option<WorldMapSpec>,  // 新增结构化规格书
    pub map_dirty: bool,
}
```

### 2. Service 层扩展

现有：
```rust
pub struct AiBookService {
    pool: SqlitePool,
    storage_dir: PathBuf,
}
```

新增：
```rust
pub struct WorldMapBuilderService {
    ai_proxy: Arc<AiProxyService>,  // 复用现有 AI 调用
    storage: WorldMapStorage,
    inference: WorldMapInferenceEngine,
    optimizer: WorldMapOptimizer,
}
```

### 3. API 扩展

现有路由：
```
GET  /reader3/ai/bookMemory
POST /reader3/ai/bookMemory
```

新增路由：
```
GET  /reader3/ai/worldMap
POST /reader3/ai/worldMap
POST /reader3/ai/worldMap/update
POST /reader3/ai/worldMap/generateCoordinates
GET  /reader3/ai/worldMap/reviewItems
POST /reader3/ai/worldMap/resolve
POST /reader3/ai/worldMap/render
```

### 4. 前端扩展

现有：
```vue
<section v-if="activeTab === 'map'" class="map-panel">
  <img v-if="memory.map?.imageUrl" :src="memory.map.imageUrl" />
  <!-- 简单的图片展示 -->
</section>
```

升级后：
```vue
<section v-if="activeTab === 'map'" class="map-panel-v2">
  <!-- 工具栏 -->
  <div class="map-toolbar">
    <div class="map-stats">...</div>
    <div class="map-actions">...</div>
  </div>
  
  <!-- 交互式画布 -->
  <WorldMapCanvas :spec="spec" />
  
  <!-- 侧边栏 -->
  <aside class="map-sidebar">
    <MapEntityPanel />
    <MapReviewPanel />
  </aside>
</section>
```

## 📈 预期收益

### 对用户

1. **更准确的地图**：基于原文证据，而非 AI 脑补
2. **可追溯性**：每个地点都能看到原文出处
3. **增量更新**：看新章节后自动更新地图
4. **冲突提示**：明确告知哪些地方原文矛盾
5. **可信度标记**：颜色区分高/中/低置信度

### 对开发者

1. **结构化数据**：JSON 格式，易于扩展和查询
2. **版本控制**：Git 管理，可回溯历史
3. **模块化设计**：各层职责清晰，易于维护
4. **测试友好**：纯函数为主，易于单元测试
5. **性能可控**：JSONL 格式，可按需加载

### 对 AI 生态

1. **可复用**：规格书可被其他 AI 工具使用
2. **可验证**：明确的证据链和置信度
3. **可协作**：多个 AI 可基于同一规格书工作
4. **可改进**：人工修正后反馈给 AI

## 🚀 下一步建议

### 立即可做

1. **验证编译**（✅ 已完成）
   ```bash
   cd /Users/maple/Documents/reader
   cargo check --lib
   # ✓ Finished `dev` profile
   ```

2. **阅读完整设计**
   - `WORLD_MAP_DESIGN.md` - 技术设计
   - `WORLD_MAP_TODO.md` - 任务清单
   - `prompts/world_map_spec_architect.md` - AI Prompt

3. **实现 MVP**（4-5天）
   - Phase 1: 存储层
   - 简化推理引擎（前3条规则）
   - 简单坐标生成
   - 基础 API
   - 前端简单展示

4. **真实测试**
   - 用真实小说章节测试
   - 验证自动化率是否达到 80%+
   - 验证地图质量是否可接受

### 中期规划

1. **完整实现**（Phase 1-7）
2. **性能优化**（大规模实体）
3. **用户体验优化**（交互、动画）
4. **文档完善**（API 文档、用户指南）

### 长期展望

1. **多书对比**（同人 vs 原著）
2. **时间线地图**（不同章节范围的地图对比）
3. **3D 地图**（地形高程）
4. **VR 体验**（沉浸式世界探索）

## 📚 文件清单

已创建的文件：

```
/Users/maple/Documents/reader/
├── src/model/
│   ├── world_map.rs                          # ✅ 核心数据结构
│   └── mod.rs                                # ✅ 已添加 world_map
├── WORLD_MAP_DESIGN.md                       # ✅ 设计文档
├── WORLD_MAP_TODO.md                         # ✅ 任务清单
├── WORLD_MAP_INTEGRATION_SUMMARY.md          # ✅ 本文件
└── prompts/
    └── world_map_spec_architect.md           # ✅ AI Prompt
```

待创建的文件（按 TODO 顺序）：

```
src/service/
├── world_map_storage.rs       # Phase 1
├── world_map_inference.rs     # Phase 2
├── world_map_optimizer.rs     # Phase 3
└── world_map_builder.rs       # Phase 4

src/api/handlers/
└── world_map.rs               # Phase 5

frontend/src/
├── stores/
│   └── worldMapStore.ts       # Phase 6
├── components/
│   ├── WorldMapCanvas.vue     # Phase 6
│   ├── MapEntityPanel.vue     # Phase 6
│   └── MapReviewPanel.vue     # Phase 6
└── types/
    └── worldMap.ts            # Phase 6
```

## 🎉 总结

我们成功将**完整的小说世界地图架构设计**集成进了 reader 项目：

1. ✅ **数据结构完备**：覆盖实体、关系、约束、冲突、坐标、审查的完整生命周期
2. ✅ **设计文档详尽**：从原则到实现的每个环节都有明确指导
3. ✅ **AI Prompt 规范**：确保 AI 生成的规格书可信、可用
4. ✅ **实施计划清晰**：12天完整实现或4天MVP验证
5. ✅ **编译通过验证**：代码结构正确，可立即开始开发

**核心创新点**：
- **证据驱动**：每条信息都有原文出处
- **自动化优先**：85-90% 的工作 AI 自己完成
- **置信度标记**：用户清楚知道哪些信息可信
- **人工可介入**：低置信度项目进入审查清单
- **增量更新**：新章节自动合并，冲突自动检测

**与现有项目的完美契合**：
- 复用 `AiProxyService` 调用 AI
- 复用 `AiBookService` 的用户权限和存储结构
- 扩展 `AiBookMemory` 而非替换
- 前端无缝集成到现有的地图 tab

开始实施？建议先跑 MVP 验证效果！
