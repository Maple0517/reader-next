# World Map Spec - 实施 TODO

## ✅ 已完成

- [x] 核心数据结构 `src/model/world_map.rs`
- [x] 设计文档 `WORLD_MAP_DESIGN.md`
- [x] AI Prompt 模板 `prompts/world_map_spec_architect.md`

## 📋 待实施（按优先级）

### Phase 1: 存储层（1天）

```rust
// src/service/world_map_storage.rs
pub struct WorldMapStorage {
    storage_dir: PathBuf,
}

impl WorldMapStorage {
    // 核心方法
    pub async fn load(&self, user_ns: &str, book_key: &str) -> Result<Option<WorldMapSpec>>;
    pub async fn save(&self, user_ns: &str, book_key: &str, spec: &WorldMapSpec) -> Result<()>;
    pub async fn save_incremental(&self, user_ns: &str, book_key: &str, delta: &WorldMapDelta) -> Result<()>;
    
    // 辅助方法
    fn spec_dir(&self, user_ns: &str, book_key: &str) -> PathBuf;
    fn load_jsonl<T: DeserializeOwned>(&self, path: &Path) -> Result<Vec<T>>;
    fn save_jsonl<T: Serialize>(&self, path: &Path, items: &[T]) -> Result<()>;
}
```

**文件结构**：
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
└── review_items.json
```

**任务清单**：
- [ ] 实现 `WorldMapStorage` 结构体
- [ ] 实现 JSONL 读写工具函数
- [ ] 实现增量更新逻辑（append + dedup）
- [ ] 单元测试（load/save/incremental）
- [ ] 集成到 `AppState`

---

### Phase 2: 推理引擎（2天）

```rust
// src/service/world_map_inference.rs
pub struct WorldMapInferenceEngine;

impl WorldMapInferenceEngine {
    // 冲突解决
    pub fn resolve_conflict(&self, conflict: &WorldMapConflict, context: &InferenceContext) 
        -> ConflictResolution;
    
    // 位置推理
    pub fn infer_position(&self, entity: &WorldMapEntity, context: &InferenceContext)
        -> PositionInference;
    
    // 置信度计算
    pub fn compute_confidence(&self, evidence: &Evidence, strategy: &str)
        -> f64;
    
    // 传递性推理
    pub fn infer_transitive_relations(&self, relations: &[WorldMapRelation])
        -> Vec<WorldMapRelation>;
}

pub struct InferenceContext {
    pub entities: HashMap<String, WorldMapEntity>,
    pub relations: Vec<WorldMapRelation>,
    pub factions: Vec<WorldMapFaction>,
}

pub struct ConflictResolution {
    pub resolution_hint: ResolutionHint,
    pub reason: String,
    pub confidence: f64,
    pub need_human: bool,
}

pub struct PositionInference {
    pub entity_id: String,
    pub inferred_direction: Option<Direction>,
    pub confidence: f64,
    pub reasoning: Vec<ReasoningStep>,
}
```

**任务清单**：
- [ ] 实现冲突解决规则（8种，见设计文档）
- [ ] 实现位置推理策略（5种）
- [ ] 实现置信度加权算法
- [ ] 实现传递性推理（A→B→C）
- [ ] 单元测试（mock 数据）
- [ ] 集成测试（真实小说片段）

---

### Phase 3: 坐标优化器（2天）

```rust
// src/service/world_map_optimizer.rs
pub struct WorldMapOptimizer;

impl WorldMapOptimizer {
    // 生成坐标
    pub fn generate_coordinates(
        &self,
        spec: &WorldMapSpec,
        anchors: Option<Vec<Anchor>>
    ) -> Result<WorldMapCoordinates>;
    
    // 约束验证
    pub fn validate_constraints(
        &self,
        coords: &WorldMapCoordinates,
        constraints: &WorldMapConstraints
    ) -> Vec<ConstraintViolation>;
    
    // 自验证循环
    pub fn self_validate_and_fix(
        &self,
        spec: &mut WorldMapSpec,
        max_iterations: usize
    ) -> Vec<ValidationError>;
    
    // 内部方法
    fn build_constraint_matrix(&self, constraints: &WorldMapConstraints) -> ConstraintMatrix;
    fn solve_layout(&self, matrix: &ConstraintMatrix) -> Result<Vec<(f64, f64)>>;
    fn compute_layout_error(&self, coords: &[(f64, f64)], constraints: &WorldMapConstraints) -> f64;
}

pub struct Anchor {
    pub entity_id: String,
    pub x: f64,
    pub y: f64,
}
```

**依赖**：
- `nalgebra` 或 `ndarray`（矩阵运算）
- `argmin` 或自实现梯度下降

**任务清单**：
- [ ] 选择约束求解算法（L-BFGS-B vs 简单梯度下降）
- [ ] 实现约束转矩阵
- [ ] 实现布局优化器
- [ ] 实现空间一致性检查（5项，见设计文档）
- [ ] 实现自验证循环（3轮迭代）
- [ ] 单元测试
- [ ] 性能优化（大规模实体）

---

### Phase 4: 构建服务（2天）

```rust
// src/service/world_map_builder.rs
pub struct WorldMapBuilderService {
    ai_proxy: Arc<AiProxyService>,
    storage: WorldMapStorage,
    inference: WorldMapInferenceEngine,
    optimizer: WorldMapOptimizer,
}

impl WorldMapBuilderService {
    // 从章节构建
    pub async fn build_from_chapters(
        &self,
        user_ns: &str,
        book_url: &str,
        chapters: &[ChapterContent]
    ) -> Result<WorldMapSpec>;
    
    // 增量更新
    pub async fn update_incremental(
        &self,
        user_ns: &str,
        book_url: &str,
        existing: WorldMapSpec,
        new_chapters: &[ChapterContent]
    ) -> Result<WorldMapSpec>;
    
    // 内部流程
    async fn extract_entities(&self, chapters: &[ChapterContent]) -> Result<Vec<WorldMapEntity>>;
    async fn extract_relations(&self, entities: &[WorldMapEntity], chapters: &[ChapterContent]) -> Result<Vec<WorldMapRelation>>;
    fn detect_conflicts(&self, relations: &[WorldMapRelation]) -> Vec<WorldMapConflict>;
    fn build_constraints(&self, relations: &[WorldMapRelation]) -> WorldMapConstraints;
    fn generate_review_items(&self, spec: &WorldMapSpec) -> Vec<WorldMapReviewItem>;
}
```

**任务清单**：
- [ ] 实现 `WorldMapBuilderService` 结构体
- [ ] 实现 AI 调用逻辑（使用 prompt 模板）
- [ ] 实现完整构建流程（8步）
- [ ] 实现增量更新逻辑（delta merge）
- [ ] 错误处理和重试
- [ ] 单元测试 + 集成测试

---

### Phase 5: API 层（1天）

```rust
// src/api/handlers/world_map.rs
use axum::{extract::{Query, State}, Json};

// 获取地图规格书
pub async fn get_world_map_spec(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<WorldMapRequest>,
) -> Result<Json<ApiResponse<WorldMapSpec>>, AppError>;

// 保存地图规格书
pub async fn save_world_map_spec(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(spec): Json<WorldMapSpec>,
) -> Result<Json<ApiResponse<WorldMapSpec>>, AppError>;

// 增量更新（新章节）
pub async fn update_world_map(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<UpdateWorldMapRequest>,
) -> Result<Json<ApiResponse<UpdateWorldMapResponse>>, AppError>;

// 生成坐标
pub async fn generate_coordinates(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<GenerateCoordinatesRequest>,
) -> Result<Json<ApiResponse<WorldMapCoordinates>>, AppError>;

// 获取审查清单
pub async fn get_review_items(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(q): Query<WorldMapRequest>,
) -> Result<Json<ApiResponse<Vec<WorldMapReviewItem>>>, AppError>;

// 人工修正
pub async fn resolve_review_item(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<ResolveReviewRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError>;

// 导出地图图片
pub async fn render_map_image(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<RenderMapRequest>,
) -> Result<Json<ApiResponse<RenderMapResponse>>, AppError>;
```

**路由注册**：
```rust
// src/api/routes.rs
pub fn world_map_routes() -> Router<AppState> {
    Router::new()
        .route("/worldMap", get(get_world_map_spec).post(save_world_map_spec))
        .route("/worldMap/update", post(update_world_map))
        .route("/worldMap/generateCoordinates", post(generate_coordinates))
        .route("/worldMap/reviewItems", get(get_review_items))
        .route("/worldMap/resolve", post(resolve_review_item))
        .route("/worldMap/render", post(render_map_image))
}
```

**任务清单**：
- [ ] 实现所有 API handlers
- [ ] 定义 Request/Response 结构体
- [ ] 权限验证（复用 `ensure_shelf_book`）
- [ ] 错误处理
- [ ] API 测试（Postman / curl）

---

### Phase 6: 前端（3天）

#### 6.1 状态管理

```typescript
// frontend/src/stores/worldMapStore.ts
import { defineStore } from 'pinia'
import type { WorldMapSpec, WorldMapReviewItem } from '@/types/worldMap'

export const useWorldMapStore = defineStore('worldMap', {
  state: () => ({
    spec: null as WorldMapSpec | null,
    loading: false,
    selectedEntityId: null as string | null,
    sidebarTab: 'entities' as 'entities' | 'review',
  }),
  
  actions: {
    async loadSpec(bookUrl: string) { /* ... */ },
    async updateToCurrentProgress(bookUrl: string, currentChapter: number) { /* ... */ },
    async generateCoordinates(bookUrl: string, anchors?: Anchor[]) { /* ... */ },
    async renderImage(bookUrl: string, style?: string) { /* ... */ },
    async resolveReviewItem(itemId: string, resolution: any) { /* ... */ },
  },
  
  getters: {
    placedEntities: (state) => state.spec?.coordinates?.placed || [],
    unplacedEntities: (state) => state.spec?.coordinates?.unplaced || [],
    reviewItems: (state) => state.spec?.review_items || [],
    automationRate: (state) => state.spec?.statistics?.automation_rate || 0,
  }
})
```

#### 6.2 组件

**WorldMapCanvas.vue**（交互式画布）

```vue
<template>
  <svg :viewBox="`0 0 ${width} ${height}`" class="world-map-canvas">
    <!-- 地点节点 -->
    <g v-for="entity in placedEntities" :key="entity.entity_id">
      <circle 
        :cx="entity.x" 
        :cy="entity.y" 
        :r="10"
        :class="`confidence-${entity.confidence}`"
        @click="selectEntity(entity.entity_id)"
      />
      <text :x="entity.x" :y="entity.y - 15">
        {{ getEntityName(entity.entity_id) }}
      </text>
    </g>
    
    <!-- 关系连线 -->
    <g v-for="rel in visibleRelations" :key="rel.id">
      <line 
        :x1="getEntityX(rel.from_id)" 
        :y1="getEntityY(rel.from_id)"
        :x2="getEntityX(rel.to_id)"
        :y2="getEntityY(rel.to_id)"
        :class="`relation-${rel.relation_type}`"
      />
    </g>
  </svg>
</template>

<style scoped>
.confidence-fixed { fill: #10b981; }
.confidence-relative { fill: #f59e0b; }
.confidence-tentative { fill: #ef4444; }
</style>
```

**MapReviewPanel.vue**（审查面板）

```vue
<template>
  <div class="review-panel">
    <div class="review-summary">
      需要审查 {{ items.length }} 项
      <span>（预计 {{ estimatedTime }} 分钟）</span>
    </div>
    
    <div v-for="item in sortedItems" :key="item.id" class="review-item">
      <div class="review-header">
        <span :class="`severity-${item.severity}`">
          {{ severityLabel(item.severity) }}
        </span>
        <span>{{ itemTypeLabel(item.item_type) }}</span>
      </div>
      
      <div class="review-body">
        <strong>{{ item.issue }}</strong>
        <p class="ai-suggestion">
          AI 建议：{{ item.ai_suggestion }}
        </p>
        <span class="confidence">
          置信度: {{ (item.confidence * 100).toFixed(0) }}%
        </span>
      </div>
      
      <div class="review-actions">
        <button @click="acceptAI(item)">采纳</button>
        <button @click="manualEdit(item)">修正</button>
        <button @click="skip(item)">跳过</button>
      </div>
    </div>
  </div>
</template>
```

**任务清单**：
- [ ] 创建 `types/worldMap.ts`（TypeScript 类型定义）
- [ ] 实现 `worldMapStore.ts`
- [ ] 实现 `WorldMapCanvas.vue`
- [ ] 实现 `MapReviewPanel.vue`
- [ ] 实现 `MapEntityPanel.vue`
- [ ] 升级 `AiBookView.vue` 地图 tab
- [ ] CSS 样式（置信度着色、交互高亮）
- [ ] 响应式适配

---

### Phase 7: 测试 & 文档（1天）

**任务清单**：
- [ ] 端到端测试（真实小说章节）
- [ ] 性能测试（100+ 实体规模）
- [ ] 错误场景测试（冲突严重、信息缺失）
- [ ] API 文档（OpenAPI spec）
- [ ] 用户使用指南
- [ ] 开发者文档（架构、扩展点）

---

## 🔧 技术选型待定

### 1. 坐标求解器

**选项 A：Rust 纯实现**
- 使用 `nalgebra` + 手写梯度下降
- 优点：零依赖、部署简单
- 缺点：需要自己实现优化算法

**选项 B：argmin crate**
- 使用 `argmin` + `argmin-math`
- 优点：成熟的优化库
- 缺点：依赖较重

**选项 C：Python scipy（通过 PyO3）**
- 调用 Python `scipy.optimize.minimize`
- 优点：算法成熟、效果好
- 缺点：需要 Python 运行时

**推荐**：先用选项 A 实现简单梯度下降，效果不好再升级到 B 或 C。

### 2. 向量检索（可选）

如果需要语义搜索（如"靠近山的城市"），可以加入：
- `sentence-transformers` 生成 embedding
- `faiss` 或 `hnswlib` 做近似最近邻搜索

**当前**：不实现，优先保证结构化查询。

### 3. 地图绘制

**选项 A：AI 图片生成**
- 用 DALL-E / Stable Diffusion 生成艺术风格地图
- 优点：美观
- 缺点：不精确、成本高

**选项 B：程序化生成（SVG）**
- 基于坐标直接渲染 SVG
- 优点：精确、可交互
- 缺点：不够美观

**推荐**：两者结合
1. 程序化生成 SVG（用于内部验证）
2. 用 SVG 作为 AI 的参考，生成美化版图片

---

## 📅 时间估算

| Phase | 工作量 | 依赖 |
|-------|-------|-----|
| Phase 1: 存储层 | 1天 | - |
| Phase 2: 推理引擎 | 2天 | Phase 1 |
| Phase 3: 坐标优化器 | 2天 | Phase 2 |
| Phase 4: 构建服务 | 2天 | Phase 1-3 |
| Phase 5: API 层 | 1天 | Phase 4 |
| Phase 6: 前端 | 3天 | Phase 5 |
| Phase 7: 测试文档 | 1天 | Phase 6 |
| **总计** | **12天** | - |

---

## 🚀 快速启动（Minimal Viable Product）

如果要快速验证方案可行性，可以先实现 MVP：

### MVP 范围
1. **数据结构**（✅ 已完成）
2. **存储层**（JSONL 读写）
3. **简化推理引擎**（只实现冲突解决前 3 条规则）
4. **简单坐标生成**（基于方位约束的启发式布局）
5. **基础 API**（GET/POST spec）
6. **前端简单展示**（只显示实体列表 + SVG 画布）

### MVP 工作量
约 4-5 天

### MVP 验证目标
- 能否从真实小说章节提取出结构化 spec？
- 冲突解决规则是否有效？
- 坐标生成是否能产生合理布局？
- 人工审查量是否控制在 10-20%？

---

## 📝 下一步行动

**推荐顺序**：

1. **先实现 MVP**（4-5天）验证可行性
2. 如果 MVP 效果好，再完成 Phase 1-7
3. 如果 MVP 发现问题，调整设计后再继续

**第一个 PR**：
- Phase 1 存储层 + 单元测试
- 预计 1 天完成

开始？
