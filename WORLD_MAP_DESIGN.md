# Reader World Map Spec - 设计文档

## 📋 概述

将 AI资料 模块的地图功能从简单的 `imageUrl + prompt` 升级为：
- **结构化地图规格书**（可追溯、可版本化）
- **自动化推理引擎**（85-90% 自动化率）
- **增量更新机制**（基于章节范围）
- **可视化 + 人工审查**（只审查 10-15% 的低置信度内容）

## 🎯 核心原则

1. **忠于原文** > 保留冲突 > 空间逻辑 > 地图美观
2. **证据分级**：A(原文直接) > B(明显暗示) > C(共同约束) > Unknown
3. **自动化优先**：AI 自己解决 85-90%，只在真正无法判断时才需要人工
4. **置信度驱动**：每个决策都有置信度分数，低于阈值才进入审查清单

## 📊 数据结构

### 核心实体

```rust
WorldMapSpec {
    metadata: {
        novel_title, start_chapter, end_chapter, 
        spec_version, timestamps, statistics
    },
    entities: Vec<WorldMapEntity>,      // 地理实体
    relations: Vec<WorldMapRelation>,   // 空间关系
    routes: Vec<WorldMapRoute>,         // 路线/交通
    factions: Vec<WorldMapFaction>,     // 势力范围
    constraints: {
        hard: Vec<Constraint>,          // 不可违反
        soft: Vec<Constraint>,          // 辅助布局
    },
    conflicts: Vec<WorldMapConflict>,   // 冲突记录
    coordinates: Option<WorldMapCoordinates>,  // 坐标草案
    review_items: Vec<WorldMapReviewItem>,     // 人工审查清单
    statistics: WorldMapStatistics,            // 自动化统计
}
```

### 证据系统

```rust
Evidence {
    level: EvidenceLevel,  // A, B, C, Unknown, Conflict
    chapter: i32,
    quote: String,
    context: Option<String>,
}
```

### 实体类型

```rust
enum EntityType {
    Settlement,  // 聚落（城市、村庄、港口）
    Region,      // 政治区域（帝国、王国、公国）
    Terrain,     // 地形（山脉、平原、沙漠）
    Water,       // 水系（河流、湖泊、海洋）
    Transit,     // 交通节点（关隘、渡口、传送阵）
    Fantasy,     // 超自然区域（禁区、秘境、遗迹）
}
```

### 空间关系

```rust
enum RelationType {
    Direction,  // A 在 B 东边（方位）
    Nearby,     // A 靠近 B（邻接）
    Contains,   // A 位于 B 内部（包含）
    Blocks,     // A 与 B 被山脉隔开（阻隔）
    Route,      // A 到 B 有路径（交通）
}

enum Direction {
    North, South, East, West,
    Northeast, Northwest, Southeast, Southwest,
}
```

### 约束系统

```rust
enum ConstraintType {
    Hard,  // 不可违反（evidence_level = A）
    Soft,  // 辅助布局（evidence_level = B/C）
}
```

### 冲突解决

```rust
WorldMapConflict {
    entities: Vec<String>,
    info_a: String,
    info_b: String,
    evidence_a: Evidence,
    evidence_b: Evidence,
    resolution_hint: ResolutionHint,  // PreferA, PreferB, IgnoreBoth, Unresolvable
    reason: String,                    // AI 给出的解决理由
    status: ConflictStatus,            // Resolved, Unresolved
}
```

### 坐标系统

```rust
WorldMapCoordinates {
    status: CoordinateStatus,  // Feasible, Partial, Blocked
    placed: Vec<PlacedEntity>,
    unplaced: Vec<UnplacedEntity>,
}

PlacedEntity {
    entity_id: String,
    x: f64, y: f64,
    confidence: CoordinateConfidence,  // Fixed, Relative, Tentative
    constraints_satisfied: Vec<String>,
}

enum CoordinateConfidence {
    Fixed,       // 硬约束确定，置信度 0.85-1.0
    Relative,    // 相对位置，置信度 0.60-0.84
    Tentative,   // 临时布局，置信度 0.40-0.59
    Forbidden,   // 信息不足，不允许放置
    Unresolved,  // 存在冲突，无法放置
}
```

### 人工审查清单

```rust
WorldMapReviewItem {
    id: String,
    item_type: ReviewItemType,  // Conflict, UncertainPosition, CriticalError
    severity: Severity,          // High, Medium, Low
    entities: Vec<String>,
    issue: String,
    ai_suggestion: String,
    confidence: f64,
}
```

## 🔄 工作流程

### Phase 1: 提取（Extract）

```
输入：小说章节 1-N
输出：entities.jsonl + relations.jsonl

步骤：
1. 提取所有地理实体（只要原文出现）
2. 提取空间关系（方位、邻接、包含、阻隔、路线）
3. 每条记录必须有 evidence（chapter + quote）
4. 证据等级自动判定（A/B/C）
```

### Phase 2: 冲突解决（Resolve）

```
输入：relations.jsonl
输出：conflicts.jsonl + resolved_relations.jsonl

自动解决规则（按优先级）：
1. 后文优先（chapter_B > chapter_A + 50）→ confidence 0.75
2. 详细描述优先（len(quote_B) > 2 * len(quote_A)）→ confidence 0.70
3. 主角视角优先 → confidence 0.80
4. 精确方位优先（"东北" > "东方"）→ confidence 0.75
5. 距离一致性（与周边地点距离相符）→ confidence 0.65

无法解决 → need_human = true
```

### Phase 3: 位置推理（Infer）

```
输入：entities.jsonl + resolved_relations.jsonl
输出：inferred_positions.jsonl

推理策略（confidence 加权）：
1. 名称暗示（"东境"包含"东"）→ 0.50
2. 文化圈聚类（同势力城市地理集中）→ 0.60
3. 气候推理（"雪山"倾向北方）→ 0.55
4. 传递性推理（A→B→C，已知A和C推B）→ 0.70
5. 贸易网络（商路节点倾向地理中心）→ 0.65

综合置信度 < 0.60 → need_human = true
```

### Phase 4: 坐标生成（Coordinate）

```
输入：entities + relations + constraints
输出：coordinates.json

方法：
1. 将约束转化为优化问题
2. 目标函数：minimize(约束违反程度)
3. 约束：
   - Hard: A.x > B.x（A在B东边）
   - Hard: distance(A, B) ≈ expected_distance
   - Soft: 势力范围内地点聚集
4. 求解器：L-BFGS-B 优化

输出：
- placed: 可定位的实体（confidence >= 0.60）
- unplaced: 无法定位的实体（信息不足或冲突）
```

### Phase 5: 自验证（Validate）

```
输入：完整 spec
输出：validated_spec + errors

检查项（3轮迭代）：
1. 空间一致性（方位是否违反）
2. 传递性（A东B，B东C → A东C？）
3. 距离一致性（"遥远"是否画得太近）
4. 地形阻隔（是否穿越山脉）
5. 势力边界（是否错误连接隔绝区域）

自动修正率：60-70%
剩余错误 → review_items
```

### Phase 6: 生成审查清单（Review）

```
输入：spec + errors
输出：review_items.json

只有以下情况进入审查清单：
1. 冲突无法自动解决（完全相反的描述）
2. 关键地点定位失败（首都、主角出生地）
3. 严重约束冲突（满足A就违反B）
4. 地图完全无法闭合

预计人工审查量：10-15% 的总问题数
```

## 🛠️ 技术实现

### 存储方案

#### 方案 A：JSONL + Git（推荐）

```
storage/data/{user_ns}/world-maps/{book_key}/
├── metadata.json
├── entities.jsonl
├── relations.jsonl
├── routes.jsonl
├── factions.jsonl
├── constraints/
│   ├── hard.jsonl
│   └── soft.jsonl
├── conflicts.jsonl
├── coordinates.json
├── review_items.json
└── .git/
```

优点：
- 轻量级，零部署成本
- 增量更新友好（追加行）
- Git 版本控制
- 可导出为单个 JSON

缺点：
- 查询性能一般（需全扫描）

#### 方案 B：SQLite（可选升级）

```sql
CREATE TABLE world_map_entities (
    id TEXT PRIMARY KEY,
    user_ns TEXT NOT NULL,
    book_key TEXT NOT NULL,
    canonical_name TEXT,
    entity_type TEXT,
    data JSON,
    evidence_level TEXT,
    confidence REAL,
    UNIQUE(user_ns, book_key, canonical_name)
);

CREATE INDEX idx_entities_book ON world_map_entities(user_ns, book_key);
CREATE INDEX idx_entities_confidence ON world_map_entities(confidence);

-- 类似结构：relations, routes, factions, constraints, conflicts
```

优点：
- 查询性能高
- 支持复杂过滤（WHERE confidence < 0.6）
- 支持 FTS5 全文搜索

缺点：
- 需要 migration

### API 设计

```rust
// 获取地图规格书
GET  /reader3/ai/worldMap?bookUrl={url}
Response: WorldMapSpec

// 保存地图规格书
POST /reader3/ai/worldMap
Body: WorldMapSpec

// 增量更新（新章节）
POST /reader3/ai/worldMap/update
Body: { bookUrl, newChapters: [51, 52, ...], mergeStrategy }
Response: { added, modified, conflicts }

// 生成坐标
POST /reader3/ai/worldMap/generateCoordinates
Body: { bookUrl, anchorPoints?: [...] }
Response: WorldMapCoordinates

// 获取审查清单
GET  /reader3/ai/worldMap/reviewItems?bookUrl={url}
Response: Vec<WorldMapReviewItem>

// 人工修正
POST /reader3/ai/worldMap/resolve
Body: { bookUrl, reviewItemId, resolution }
Response: { updated: true }

// 导出地图图片（调用 AI 绘图）
POST /reader3/ai/worldMap/render
Body: { bookUrl, imageSize, style }
Response: { imageUrl, prompt }
```

### Service 架构

```rust
// 1. 地图构建服务
pub struct WorldMapBuilderService {
    ai_proxy: AiProxyService,
    storage: WorldMapStorage,
}

impl WorldMapBuilderService {
    pub async fn build_from_chapters(&self, book_url: &str, chapters: &[String]) 
        -> Result<WorldMapSpec>;
    
    pub async fn update_incremental(&self, existing: WorldMapSpec, new_chapters: &[String])
        -> Result<WorldMapSpec>;
}

// 2. 推理引擎
pub struct WorldMapInferenceEngine;

impl WorldMapInferenceEngine {
    pub fn resolve_conflict(&self, conflict: &WorldMapConflict) 
        -> ConflictResolution;
    
    pub fn infer_position(&self, entity: &WorldMapEntity, context: &InferenceContext)
        -> PositionInference;
    
    pub fn compute_confidence(&self, evidence: &Evidence, context: &InferenceContext)
        -> f64;
}

// 3. 坐标优化器
pub struct WorldMapOptimizer;

impl WorldMapOptimizer {
    pub fn generate_coordinates(&self, spec: &WorldMapSpec, anchors: Option<Vec<Anchor>>)
        -> Result<WorldMapCoordinates>;
    
    pub fn validate_constraints(&self, coords: &WorldMapCoordinates, constraints: &WorldMapConstraints)
        -> Vec<ConstraintViolation>;
    
    pub fn self_validate_and_fix(&self, spec: &mut WorldMapSpec, max_iterations: usize)
        -> Vec<ValidationError>;
}

// 4. 存储层
pub struct WorldMapStorage {
    storage_dir: PathBuf,
}

impl WorldMapStorage {
    pub async fn load(&self, user_ns: &str, book_key: &str) 
        -> Result<Option<WorldMapSpec>>;
    
    pub async fn save(&self, user_ns: &str, book_key: &str, spec: &WorldMapSpec)
        -> Result<()>;
    
    pub async fn save_incremental(&self, user_ns: &str, book_key: &str, delta: &WorldMapDelta)
        -> Result<()>;
}
```

## 🎨 前端设计

### 地图 Tab 升级

```vue
<!-- frontend/src/views/AiBookView.vue -->
<section v-if="activeTab === 'map'" class="map-panel-v2">
  <!-- 工具栏 -->
  <div class="map-toolbar">
    <div class="map-stats">
      <span>{{ spec.entities.length }} 个地点</span>
      <span>{{ spec.relations.length }} 条关系</span>
      <span>自动化率: {{ spec.statistics.automation_rate }}%</span>
    </div>
    <div class="map-actions">
      <button @click="updateMapSpec">更新到当前进度</button>
      <button @click="generateCoordinates">生成坐标</button>
      <button @click="renderImage">绘制地图</button>
    </div>
  </div>

  <!-- 主画布区 -->
  <div class="map-main">
    <!-- 交互式 SVG 画布 -->
    <WorldMapCanvas 
      :spec="spec"
      :selected-entity="selectedEntity"
      @select="onSelectEntity"
      @move="onMoveEntity"
    />
    
    <!-- 生成的地图图片 -->
    <img v-if="spec.coordinates?.imageUrl" :src="spec.coordinates.imageUrl" />
  </div>

  <!-- 侧边栏 -->
  <aside class="map-sidebar">
    <!-- Tab 切换 -->
    <div class="sidebar-tabs">
      <button :class="{active: sidebarTab === 'entities'}">地点列表</button>
      <button :class="{active: sidebarTab === 'review'}">
        待审查 <span class="badge">{{ spec.review_items.length }}</span>
      </button>
    </div>

    <!-- 地点列表 -->
    <MapEntityPanel v-if="sidebarTab === 'entities'"
      :entities="spec.entities"
      :selected="selectedEntity"
      @select="onSelectEntity"
    />

    <!-- 审查清单 -->
    <MapReviewPanel v-else
      :items="spec.review_items"
      @resolve="onResolveReview"
    />
  </aside>
</section>
```

### 新增组件

#### 1. WorldMapCanvas.vue（交互式画布）

```vue
<template>
  <svg :viewBox="`0 0 ${width} ${height}`" class="world-map-canvas">
    <!-- 地点节点 -->
    <g v-for="entity in placedEntities" :key="entity.id">
      <circle 
        :cx="entity.x" 
        :cy="entity.y" 
        :r="entityRadius(entity)"
        :class="confidenceClass(entity.confidence)"
        @click="$emit('select', entity)"
      />
      <text :x="entity.x" :y="entity.y - 15">{{ entity.canonical_name }}</text>
    </g>

    <!-- 关系连线 -->
    <g v-for="rel in relations" :key="rel.id">
      <line 
        :x1="getEntityX(rel.from_id)" 
        :y1="getEntityY(rel.from_id)"
        :x2="getEntityX(rel.to_id)"
        :y2="getEntityY(rel.to_id)"
        :class="relationClass(rel.relation_type)"
      />
    </g>
  </svg>
</template>

<script setup>
// 按置信度着色
const confidenceClass = (confidence) => {
  if (confidence === 'fixed') return 'confidence-high'  // 绿色
  if (confidence === 'relative') return 'confidence-medium'  // 黄色
  return 'confidence-low'  // 红色
}
</script>

<style>
.confidence-high { fill: #10b981; }
.confidence-medium { fill: #f59e0b; }
.confidence-low { fill: #ef4444; }
</style>
```

#### 2. MapReviewPanel.vue（审查面板）

```vue
<template>
  <div class="review-panel">
    <div class="review-summary">
      需要审查 {{ items.length }} 项（预计 {{ estimatedTime }} 分钟）
    </div>

    <div v-for="item in sortedItems" :key="item.id" class="review-item">
      <div class="review-header">
        <span :class="`severity-${item.severity}`">{{ severityLabel(item.severity) }}</span>
        <span>{{ itemTypeLabel(item.item_type) }}</span>
      </div>

      <div class="review-body">
        <strong>{{ item.issue }}</strong>
        <p class="ai-suggestion">AI 建议：{{ item.ai_suggestion }}</p>
        <span class="confidence">置信度: {{ (item.confidence * 100).toFixed(0) }}%</span>
      </div>

      <div class="review-actions">
        <button @click="acceptAI(item)">采纳 AI 建议</button>
        <button @click="manualEdit(item)">手动修正</button>
        <button @click="skip(item)">跳过</button>
      </div>
    </div>
  </div>
</template>
```

## 📦 AI Prompt 模板

### 1. 提取实体 Prompt

```markdown
# 任务：从小说章节提取地理实体

## 输入
章节范围：{{ start_chapter }}-{{ end_chapter }}
小说内容：
"""
{{ chapters_text }}
"""

## 输出格式
JSONL格式，每行一个实体：
{"id":"E001","canonical_name":"阿尔托","aliases":[],"entity_type":"settlement","subtype":"city","first_chapter":3,"evidence":{"level":"A","chapter":3,"quote":"主角到达了阿尔托城","context":null},"description":null,"faction_id":null,"related_entity_ids":[]}

## 规则
1. 只提取原文明确出现的地名
2. 禁止脑补、推测、补全
3. 每个实体必须有原文证据（quote）
4. evidence_level 判定：
   - A: 原文直接说明（"城市名叫X"）
   - B: 原文明显暗示（"他们到达了X"）
   - C: 多处信息共同约束
   - Unknown: 不确定
5. entity_type 只能是：settlement, region, terrain, water, transit, fantasy
6. 不确定的信息留空（null）

## 输出
[直接输出JSONL，不要markdown代码块]
```

### 2. 提取关系 Prompt

```markdown
# 任务：提取地理实体间的空间关系

## 输入
已提取实体：
{{ entities_json }}

小说内容：
{{ chapters_text }}

## 输出格式
JSONL格式，每行一条关系：
{"id":"R001","from_id":"E001","to_id":"E002","relation_type":"nearby","direction":null,"bidirectional":false,"evidence":{"level":"A","chapter":5,"quote":"阿尔托靠近黑暗山脉","context":null},"constraint_type":"hard"}

## 规则
1. 只提取原文明确的方位、邻接、包含、阻隔、路径关系
2. relation_type 只能是：direction, nearby, contains, blocks, route
3. direction 只能是：north, south, east, west, northeast, northwest, southeast, southwest, null
4. constraint_type 判定：
   - hard: evidence_level = A（原文直接说明）
   - soft: evidence_level = B/C（原文暗示或推导）
5. 禁止推测方位（"A是港口"不代表"A在海边"）
6. 禁止推测距离（"走了三天"需要记录交通方式）

## 输出
[直接输出JSONL]
```

### 3. 冲突解决 Prompt

```markdown
# 任务：自动解决空间关系冲突

## 输入
冲突记录：
{{ conflict_json }}

相关实体：
{{ entities_json }}

## 输出格式
JSON对象：
{
  "resolution_hint": "prefer_b",
  "reason": "第30章描述更详细（字数为第10章的3倍），且与周边距离描述一致",
  "confidence": 0.75,
  "need_human": false
}

## 解决规则（按优先级）
1. 后文优先：如果 chapter_B > chapter_A + 50 → prefer_B, confidence=0.75
2. 详细优先：如果 len(quote_B) > 2 * len(quote_A) → prefer_B, confidence=0.70
3. 主角视角优先：如果 B 是主角视角，A 是旁白 → prefer_B, confidence=0.80
4. 精确方位优先：如果 B 是"东北"，A 是"东方" → prefer_B, confidence=0.75
5. 距离一致性：计算与周边地点距离，选择更一致的 → confidence=0.65
6. 无法判断：完全相反的描述 → need_human=true, confidence<0.50

## 输出
[直接输出JSON对象]
```

## 📈 预期效果

### 自动化率

| 模块 | 自动化率 | 人工介入场景 |
|------|---------|-------------|
| 实体提取 | 98% | 极度模糊的地名 |
| 关系提取 | 90% | 隐含关系 |
| 冲突解决 | 80% | 完全相反的描述 |
| 位置推理 | 70% | 零线索的地点 |
| 坐标生成 | 85% | 欠定问题 + 严重冲突 |
| 最终验证 | 90% | 逻辑矛盾 |
| **总体** | **85-90%** | 10-15% 需要人工 |

### 地图质量

| 小说类型 | 可信度 | 说明 |
|---------|-------|------|
| 世界观完整（冰与火之歌） | 70-80% | 主要地点位置准确，细节需人工调整 |
| 一般网文（起点玄幻） | 50-60% | 核心区域可信，边缘区域需推测 |
| 爽文流水文 | <30% | 只能提取实体列表，无法生成一致地图 |

### 人工工作量

- **零干预模式**：直接使用 AI 生成结果（准确率 50-70%）
- **轻度审查**：只审查高优先级问题（15分钟，准确率提升到 65-75%）
- **完整审查**：审查所有低置信度项（1-2小时，准确率 75-85%）

## 🚀 实施计划

### Phase 1: 核心数据结构（已完成）
- [x] `world_map.rs` 数据模型
- [x] 添加到 `mod.rs`

### Phase 2: 存储层（1天）
- [ ] `WorldMapStorage` 实现（JSONL 格式）
- [ ] 增量更新逻辑
- [ ] 单元测试

### Phase 3: 推理引擎（2天）
- [ ] `WorldMapInferenceEngine` 实现
- [ ] 冲突解决规则（8种）
- [ ] 位置推理策略（5种）
- [ ] 置信度计算

### Phase 4: 坐标优化器（2天）
- [ ] `WorldMapOptimizer` 实现
- [ ] 约束求解器（基于 `nalgebra`/`argmin`）
- [ ] 自验证循环
- [ ] 布局算法

### Phase 5: Service + API（2天）
- [ ] `WorldMapBuilderService`
- [ ] API handlers（GET/POST/update/render）
- [ ] 与 `AiProxyService` 集成

### Phase 6: 前端（3天）
- [ ] `WorldMapCanvas.vue` 交互式画布
- [ ] `MapReviewPanel.vue` 审查面板
- [ ] `worldMapStore.ts` 状态管理
- [ ] 升级 `AiBookView.vue` 地图 tab

### Phase 7: AI Prompt + 测试（2天）
- [ ] 完整 prompt 模板
- [ ] 端到端测试（真实小说章节）
- [ ] 性能优化

## 📝 待定问题

1. **坐标求解器选择**：使用 Rust 生态的 `argmin` 还是调用 Python scipy？
2. **向量检索**：是否需要 embedding 支持语义搜索？（如"靠近山的城市"）
3. **地图绘制**：使用 AI 图片生成还是基于坐标的程序化生成？
4. **多书对比**：是否支持多本小说的世界观对比（如同人 vs 原著）？

## 🔗 参考资料

- JSON Lines 规范：https://jsonlines.org/
- 约束优化：https://docs.rs/argmin/latest/argmin/
- 图算法：https://docs.rs/petgraph/latest/petgraph/
