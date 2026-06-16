use serde::{Deserialize, Serialize};

// ============================================
// World Map Spec V2 - 结构化地图规格书
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapSpec {
    pub metadata: WorldMapMetadata,
    pub entities: Vec<WorldMapEntity>,
    pub relations: Vec<WorldMapRelation>,
    pub routes: Vec<WorldMapRoute>,
    pub factions: Vec<WorldMapFaction>,
    pub constraints: WorldMapConstraints,
    pub conflicts: Vec<WorldMapConflict>,
    pub coordinates: Option<WorldMapCoordinates>,
    pub review_items: Vec<WorldMapReviewItem>,
    pub statistics: WorldMapStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldMapMetadata {
    pub novel_title: String,
    pub start_chapter: i32,
    pub end_chapter: i32,
    pub spec_version: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub total_entities: usize,
    pub total_relations: usize,
}

impl Default for WorldMapMetadata {
    fn default() -> Self {
        Self {
            novel_title: String::new(),
            start_chapter: 0,
            end_chapter: 0,
            spec_version: "2.0".to_string(),
            created_at: 0,
            updated_at: 0,
            total_entities: 0,
            total_relations: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapEntity {
    pub id: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub entity_type: EntityType,
    pub subtype: Option<String>,
    pub first_chapter: i32,
    pub evidence: Evidence,
    pub description: Option<String>,
    pub faction_id: Option<String>,
    pub related_entity_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Settlement,  // 聚落
    Region,      // 政治区域
    Terrain,     // 地形
    Water,       // 水系
    Transit,     // 交通节点
    Fantasy,     // 超自然区域
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub level: EvidenceLevel,
    pub chapter: i32,
    pub quote: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum EvidenceLevel {
    A,         // 原文直接说明
    B,         // 原文明显暗示
    C,         // 多条信息共同约束
    Unknown,   // 原文未说明
    Conflict,  // 原文存在冲突
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapRelation {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub relation_type: RelationType,
    pub direction: Option<Direction>,
    pub bidirectional: bool,
    pub evidence: Evidence,
    pub constraint_type: ConstraintType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RelationType {
    Direction,  // 方位关系
    Nearby,     // 邻接
    Contains,   // 包含
    Blocks,     // 阻隔
    Route,      // 路径
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    North,
    South,
    East,
    West,
    Northeast,
    Northwest,
    Southeast,
    Southwest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConstraintType {
    Hard,  // 不可违反
    Soft,  // 辅助布局
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapRoute {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub transport_mode: Option<String>,
    pub distance: Option<f64>,
    pub time: Option<String>,
    pub via: Vec<String>,
    pub blocks: Vec<String>,
    pub is_trade_route: bool,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapFaction {
    pub id: String,
    pub name: String,
    pub faction_type: String,
    pub core_entities: Vec<String>,
    pub controlled_entities: Vec<String>,
    pub influence_entities: Vec<String>,
    pub borders: Vec<String>,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapConstraints {
    pub hard: Vec<Constraint>,
    pub soft: Vec<Constraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Constraint {
    pub id: String,
    pub constraint_type: String,
    pub entities: Vec<String>,
    pub description: String,
    pub evidence: Evidence,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapConflict {
    pub id: String,
    pub entities: Vec<String>,
    pub info_a: String,
    pub info_b: String,
    pub evidence_a: Evidence,
    pub evidence_b: Evidence,
    pub resolution_hint: ResolutionHint,
    pub reason: String,
    pub status: ConflictStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionHint {
    PreferA,       // 倾向A
    PreferB,       // 倾向B
    IgnoreBoth,    // 忽略两者
    Unresolvable,  // 无法解决
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConflictStatus {
    Resolved,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapCoordinates {
    pub status: CoordinateStatus,
    pub reason: Option<String>,
    pub placed: Vec<PlacedEntity>,
    pub unplaced: Vec<UnplacedEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CoordinateStatus {
    Feasible,  // 可生成完整地图
    Partial,   // 部分地点可定位
    Blocked,   // 无法生成地图
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlacedEntity {
    pub entity_id: String,
    pub x: f64,
    pub y: f64,
    pub confidence: CoordinateConfidence,
    pub constraints_satisfied: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CoordinateConfidence {
    Fixed,       // 硬约束确定
    Relative,    // 相对位置
    Tentative,   // 临时布局
    Forbidden,   // 信息不足
    Unresolved,  // 存在冲突
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnplacedEntity {
    pub entity_id: String,
    pub reason: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapReviewItem {
    pub id: String,
    pub item_type: ReviewItemType,
    pub severity: Severity,
    pub entities: Vec<String>,
    pub issue: String,
    pub ai_suggestion: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewItemType {
    Conflict,           // 冲突
    UncertainPosition,  // 位置不确定
    CriticalError,      // 严重错误
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapStatistics {
    pub total_issues: usize,
    pub auto_resolved: usize,
    pub need_human: usize,
    pub automation_rate: f64,
}
