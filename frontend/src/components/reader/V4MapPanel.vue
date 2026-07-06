<template>
  <V4PanelShell
    title="地图"
    :subtitle="overviewText"
    :loading="listState.loading.value"
    :error="listState.error.value"
    :empty="listState.empty.value"
    empty-title="暂无地图资料"
    empty-message="当前书籍还没有可展示的地图数据。"
    @retry="listState.reload()"
  >
    <template #toolbar>
      <button class="v4-map-refresh" type="button" :disabled="listState.loading.value" @click="listState.reload()">
        刷新
      </button>
    </template>

    <div class="map-stats">
      <span>地点 {{ mapData?.overview.placeCount ?? 0 }}</span>
      <span>层级关系 {{ flatHierarchy.length }}</span>
      <span>拓扑边 {{ mapData?.overview.activeEdgeCount ?? 0 }}</span>
      <span :class="{ warn: conflictCount > 0 }">冲突 {{ conflictCount }}</span>
    </div>

    <nav class="map-view-modes" aria-label="视图模式">
      <button
        v-for="mode in viewModes"
        :key="mode.key"
        type="button"
        :class="{ active: viewMode === mode.key }"
        :data-test="`map-mode-${mode.key}`"
        @click="viewMode = mode.key"
      >
        {{ mode.label }}
      </button>
    </nav>

    <div class="map-layout" :class="`map-layout--${viewMode}`">
      <!-- Left: place navigation / hierarchy tree -->
      <aside class="map-sidebar" aria-label="地点导航">
        <template v-if="viewMode === 'hierarchy'">
          <p v-if="!flatHierarchy.length" class="map-empty-hint">暂无层级关系资料</p>
          <div
            v-for="node in flatHierarchy"
            :key="node.placeId"
            class="map-sidebar-row"
            :class="{ active: selectedPlaceId === node.placeId }"
            :style="{ paddingLeft: `${node.depth * 16 + 10}px` }"
            @click="loadPlaceDetail(node.placeId)"
          >
            <span>{{ node.name }}</span>
            <small>{{ placeTypeLabel(node.placeType) }}</small>
          </div>
        </template>
        <template v-else>
          <p v-if="!mapData?.places.length" class="map-empty-hint">暂无地点资料</p>
          <button
            v-for="place in mapData?.places ?? []"
            :key="place.id"
            type="button"
            class="map-sidebar-row"
            :class="{ active: selectedPlaceId === place.id }"
            :data-test="`map-place-${place.id}`"
            @click="loadPlaceDetail(place.id)"
          >
            <span>{{ place.name }}</span>
            <small>{{ placeTypeLabel(place.placeType) }}</small>
          </button>
        </template>
      </aside>

      <!-- Center: topology canvas or list -->
      <div class="map-main">
        <template v-if="viewMode === 'topology'">
          <svg
            v-if="topologyNodes.length && hasLayoutData"
            class="map-topology-canvas"
            :viewBox="topologyViewBox"
            role="img"
            aria-label="拓扑关系图"
          >
            <line
              v-for="edge in topologyEdges"
              :key="edge.edgeId"
              :x1="nodePos(edge.fromPlaceId).x"
              :y1="nodePos(edge.fromPlaceId).y"
              :x2="nodePos(edge.toPlaceId).x"
              :y2="nodePos(edge.toPlaceId).y"
              class="map-topology-edge"
            />
            <g
              v-for="node in topologyNodes"
              :key="node.placeId"
              class="map-topology-node"
              :class="{ selected: selectedPlaceId === node.placeId }"
              :transform="`translate(${nodePos(node.placeId).x}, ${nodePos(node.placeId).y})`"
              @click="loadPlaceDetail(node.placeId)"
            >
              <rect x="-50" y="-18" width="100" height="36" rx="10" />
              <text text-anchor="middle" dy="0.35em">{{ node.label }}</text>
            </g>
          </svg>
          <template v-else-if="topologyNodes.length">
            <p class="map-empty-hint">布局暂不可用，使用拓扑表格展示。</p>
            <article v-for="edge in topologyEdges" :key="edge.edgeId" class="map-edge-row">
              <span>{{ placeName(edge.fromPlaceId) }}</span>
              <strong>{{ edgeTypeLabel(edge.edgeType) }}</strong>
              <span>{{ placeName(edge.toPlaceId) }}</span>
            </article>
          </template>
          <p v-else class="map-empty-hint">暂无拓扑关系数据。</p>
        </template>

        <template v-else-if="viewMode === 'hierarchy'">
          <p v-if="!flatHierarchy.length" class="map-empty-hint">暂无层级关系资料</p>
          <div v-else class="hierarchy-tree">
            <div
              v-for="node in flatHierarchy"
              :key="node.placeId"
              class="hierarchy-tree-row"
              :style="{ paddingLeft: `${node.depth * 20 + 10}px` }"
            >
              <span>{{ node.name }}</span>
              <small>{{ placeTypeLabel(node.placeType) }}</small>
            </div>
          </div>
        </template>

        <template v-else>
          <!-- list view: edge table -->
          <p v-if="!mapData?.graph.edges.length" class="map-empty-hint">暂无拓扑边。</p>
          <article v-for="edge in mapData?.graph.edges ?? []" :key="edge.edgeId" class="map-edge-row">
            <span>{{ placeName(edge.fromPlaceId) }}</span>
            <strong>{{ edgeTypeLabel(edge.edgeType) }}</strong>
            <span>{{ placeName(edge.toPlaceId) }}</span>
          </article>
          <p v-if="graphWarnings.length" class="map-warning">{{ graphWarnings.join('；') }}</p>
        </template>
      </div>

      <!-- Right: selected place detail -->
      <section class="map-detail" aria-label="地点详情">
        <p v-if="detailLoading" class="map-empty-hint">地点详情加载中...</p>
        <V4EmptyState v-else-if="!selectedDetail" title="选择地点" message="点击左侧地点查看详情。" />
        <template v-else>
          <div class="map-detail-head">
            <div>
              <h3>{{ selectedDetail.name }}</h3>
              <p>{{ placeTypeLabel(selectedDetail.placeType) }}</p>
            </div>
          </div>
          <section v-if="selectedDetail.linkedOrganizations.length" class="map-linked-section">
            <strong>关联组织</strong>
            <span
              v-for="org in selectedDetail.linkedOrganizations"
              :key="`${org.id}-${org.linkType}`"
            >
              {{ org.name }} · {{ linkTypeLabel(org.linkType) }}
            </span>
          </section>
        </template>
      </section>
    </div>

    <section v-if="mapData?.conflicts.length" class="map-conflict" aria-label="地图冲突">
      <div class="section-head">
        <strong>冲突与不确定</strong>
        <span>{{ mapData.conflicts.length }}</span>
      </div>
      <article v-for="conflict in mapData.conflicts" :key="conflict.id" class="map-conflict-row">
        <strong>{{ conflictTypeLabel(conflict.conflictType) }}</strong>
        <span>{{ conflict.reasonCode }}</span>
        <small>{{ conflict.status }}</small>
      </article>
    </section>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useV4PanelState } from '../../composables/useV4PanelState'
import V4PanelShell from './v4/V4PanelShell.vue'
import V4EmptyState from './v4/V4EmptyState.vue'
import {
  getV4Map,
  getV4MapConflicts,
  getV4MapGraph,
  getV4MapLayout,
  getV4MapPlaceDetail,
  getV4MapPlaces,
} from '../../api/v4/book'
import type {
  V4MapConflictView,
  V4MapGraphView,
  V4MapLayoutView,
  V4MapOverviewView,
  V4PlaceDetailView,
  V4PlaceHierarchyNode,
  V4PlaceSummaryView,
} from '../../types/v4'

const props = defineProps<{
  bookUrl: string
}>()

/* ---- data shape returned by the multi-API fetcher ---- */
interface MapData {
  overview: V4MapOverviewView
  places: V4PlaceSummaryView[]
  hierarchy: V4PlaceHierarchyNode[]
  graph: V4MapGraphView
  layout: V4MapLayoutView
  conflicts: V4MapConflictView[]
}

type FlatHierarchyNode = V4PlaceHierarchyNode & { depth: number }

/* ---- panel state via composable ---- */
const bookUrlRef = computed(() => props.bookUrl)

const listState = useV4PanelState<MapData>(
  async () => {
    const [overviewData, placesData, graphData, layoutData, conflictsData] = await Promise.all([
      getV4Map(props.bookUrl),
      getV4MapPlaces(props.bookUrl),
      getV4MapGraph(props.bookUrl),
      getV4MapLayout(props.bookUrl),
      getV4MapConflicts(props.bookUrl),
    ])
    return {
      overview: overviewData,
      places: placesData.places || [],
      hierarchy: placesData.hierarchy || [],
      graph: graphData,
      layout: layoutData,
      conflicts: conflictsData.conflicts || [],
    }
  },
  bookUrlRef,
  {
    emptyCheck: (d) => d.places.length === 0 && d.graph.nodes.length === 0,
  },
)

const mapData = computed(() => listState.data.value)

/* ---- view mode ---- */
type ViewMode = 'topology' | 'hierarchy' | 'list'
const viewMode = ref<ViewMode>('topology')
const viewModes: { key: ViewMode; label: string }[] = [
  { key: 'topology', label: '拓扑视图' },
  { key: 'hierarchy', label: '层级视图' },
  { key: 'list', label: '地点列表' },
]

/* ---- detail loading (panel-local) ---- */
const detailLoading = ref(false)
const selectedPlaceId = ref('')
const selectedDetail = ref<V4PlaceDetailView | null>(null)
let detailRequestId = 0

async function loadPlaceDetail(placeId: string) {
  if (!props.bookUrl) return
  const req = ++detailRequestId
  detailLoading.value = true
  selectedPlaceId.value = placeId
  try {
    const detail = await getV4MapPlaceDetail(props.bookUrl, placeId)
    if (req !== detailRequestId) return
    selectedDetail.value = detail
  } catch {
    if (req !== detailRequestId) return
    selectedDetail.value = null
  } finally {
    if (req === detailRequestId) detailLoading.value = false
  }
}

/* ---- derived ---- */
const overviewText = computed(() => {
  if (!mapData.value) return ''
  const { overview } = mapData.value
  const hCount = flatHierarchy.value.length
  return `${overview.placeCount} 个地点 · ${hCount} 个层级关系 · ${overview.activeEdgeCount} 条拓扑边`
})

const conflictCount = computed(() => mapData.value?.conflicts.length ?? mapData.value?.overview.conflictCount ?? 0)

const flatHierarchy = computed(() => {
  const result: FlatHierarchyNode[] = []
  const visit = (nodes: V4PlaceHierarchyNode[], depth: number) => {
    for (const node of nodes) {
      result.push({ ...node, depth })
      visit(node.children || [], depth + 1)
    }
  }
  visit(mapData.value?.hierarchy ?? [], 0)
  return result
})

/* ---- topology layout ---- */
interface PositionedNode { placeId: string; label: string; placeType: string; x: number; y: number }

const topologyNodes = computed<PositionedNode[]>(() => {
  if (!mapData.value) return []
  const graphNodes = mapData.value.graph.nodes
  const layoutNodes = mapData.value.graph.layout?.nodes ?? mapData.value.layout.nodes ?? []

  if (layoutNodes.length) {
    return graphNodes.map((gn) => {
      const ln = layoutNodes.find((l) => l.placeId === gn.placeId)
      return {
        placeId: gn.placeId,
        label: gn.label,
        placeType: gn.placeType,
        x: ln?.x ?? 0,
        y: ln?.y ?? 0,
      }
    })
  }

  // fallback: simple spring-ish circular layout
  const n = graphNodes.length
  if (!n) return []
  const radius = Math.max(80, n * 30)
  return graphNodes.map((gn, i) => {
    const angle = (2 * Math.PI * i) / n
    return {
      placeId: gn.placeId,
      label: gn.label,
      placeType: gn.placeType,
      x: radius + radius * Math.cos(angle),
      y: radius + radius * Math.sin(angle),
    }
  })
})

const hasLayoutData = computed(() => {
  if (!mapData.value) return false
  const fromGraph = mapData.value.graph.layout?.nodes
  const fromLayout = mapData.value.layout.nodes
  return (fromGraph?.length ?? 0) > 0 || (fromLayout?.length ?? 0) > 0
})

const topologyEdges = computed(() => mapData.value?.graph.edges ?? [])

const nodePositions = computed(() => {
  const map = new Map<string, { x: number; y: number }>()
  for (const n of topologyNodes.value) {
    map.set(n.placeId, { x: n.x, y: n.y })
  }
  return map
})

function nodePos(placeId: string) {
  return nodePositions.value.get(placeId) ?? { x: 0, y: 0 }
}

const topologyViewBox = computed(() => {
  if (!topologyNodes.value.length) return '0 0 400 300'
  const xs = topologyNodes.value.map((n) => n.x)
  const ys = topologyNodes.value.map((n) => n.y)
  const pad = 70
  const minX = Math.min(...xs) - pad
  const minY = Math.min(...ys) - pad
  const width = Math.max(...xs) - minX + pad
  const height = Math.max(...ys) - minY + pad
  return `${minX} ${minY} ${width} ${height}`
})

const graphWarnings = computed(() => {
  if (!mapData.value) return []
  const warnings = new Set<string>()
  for (const w of mapData.value.graph.warnings || []) warnings.add(w)
  for (const w of mapData.value.layout.warnings || []) warnings.add(w)
  return Array.from(warnings)
})

/* ---- label helpers ---- */
function placeName(placeId: string): string {
  return mapData.value?.places.find((p) => p.id === placeId)?.name
    || mapData.value?.graph.nodes.find((n) => n.placeId === placeId)?.label
    || placeId
}

function placeTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    world: '世界',
    continent: '大陆',
    region: '区域',
    country: '国家',
    city: '城市',
    sect_site: '宗门驻地',
    building: '建筑',
    room: '房间',
    mountain: '山',
    river: '河流',
    forest: '森林',
    road: '道路',
    route: '路线',
    secret_realm: '秘境',
    battlefield: '战场',
    dungeon: '险地',
    unknown: '未知',
  }
  return labels[type] || type
}

function edgeTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    route_to: '路线',
    north_of: '北侧',
    south_of: '南侧',
    east_of: '东侧',
    west_of: '西侧',
    contains: '包含',
    inside: '位于',
    adjacent_to: '相邻',
    connected_to: '连通',
  }
  return labels[type] || type
}

function linkTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    organization_place_pair: '同名关联',
    based_at: '驻地',
    headquarters_of: '总部',
    related_entity: '相关实体',
  }
  return labels[type] || type
}

function conflictTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    duplicate_conflicting_direction: '方向冲突',
    hierarchy_cycle: '层级循环',
    topology_conflict: '拓扑冲突',
  }
  return labels[type] || type
}

defineExpose({ reload: () => listState.reload() })
</script>

<style scoped>
/* ---- stats ---- */
.map-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
}

.map-stats span {
  border: 1px solid rgba(100, 116, 139, 0.22);
  border-radius: 999px;
  padding: 4px 10px;
  color: var(--reader-muted-text, #64748b);
  font-size: 12px;
}

.map-stats .warn {
  border-color: rgba(217, 119, 6, 0.35);
  color: #b45309;
}

/* ---- view mode toggle ---- */
.map-view-modes {
  display: flex;
  gap: 6px;
  margin-bottom: 14px;
}

.map-view-modes button {
  border: 1px solid rgba(100, 116, 139, 0.2);
  border-radius: 999px;
  padding: 5px 14px;
  background: transparent;
  color: var(--color-text-secondary, #64748b);
  font-size: 13px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.map-view-modes button.active {
  border-color: var(--color-primary, #2563eb);
  background: rgba(37, 99, 235, 0.08);
  color: var(--color-primary, #2563eb);
}

/* ---- main layout ---- */
.map-layout {
  display: grid;
  grid-template-columns: minmax(160px, 0.8fr) minmax(200px, 1.2fr) minmax(160px, 0.8fr);
  gap: 14px;
  min-height: 300px;
}

.map-layout--list {
  grid-template-columns: minmax(160px, 0.6fr) minmax(200px, 1fr) minmax(160px, 0.8fr);
}

@media (max-width: 760px) {
  .map-layout,
  .map-layout--list {
    grid-template-columns: 1fr;
  }
}

/* ---- sidebar ---- */
.map-sidebar {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 400px;
  overflow-y: auto;
}

.map-sidebar-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  border: 1px solid rgba(100, 116, 139, 0.16);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  padding: 8px 12px;
  width: 100%;
  color: inherit;
  text-align: left;
  cursor: pointer;
  font-size: 14px;
}

button.map-sidebar-row {
  font: inherit;
}

.map-sidebar-row.active {
  border-color: rgba(37, 99, 235, 0.35);
  background: rgba(37, 99, 235, 0.08);
}

.map-sidebar-row small {
  color: var(--reader-muted-text, #64748b);
  font-size: 12px;
  white-space: nowrap;
}

/* ---- topology canvas ---- */
.map-topology-canvas {
  width: 100%;
  min-height: 250px;
  border-radius: 14px;
  border: 1px solid rgba(100, 116, 139, 0.12);
  background: rgba(255, 255, 255, 0.5);
}

.map-topology-edge {
  stroke: rgba(100, 116, 139, 0.35);
  stroke-width: 1.5;
}

.map-topology-node {
  cursor: pointer;
}

.map-topology-node rect {
  fill: #fff;
  stroke: rgba(100, 116, 139, 0.3);
  stroke-width: 1.2;
  transition: stroke 0.15s, fill 0.15s;
}

.map-topology-node.selected rect {
  stroke: var(--color-primary, #2563eb);
  stroke-width: 2;
  fill: rgba(37, 99, 235, 0.06);
}

.map-topology-node text {
  fill: var(--color-text, #1e293b);
  font-size: 12px;
  pointer-events: none;
}

/* ---- edge list rows ---- */
.map-edge-row {
  display: flex;
  align-items: center;
  gap: 10px;
  border: 1px solid rgba(100, 116, 139, 0.16);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  padding: 8px 12px;
  font-size: 14px;
}

.map-edge-row strong {
  color: #2563eb;
}

/* ---- hierarchy ---- */
.hierarchy-tree {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.hierarchy-tree-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  border: 1px solid rgba(100, 116, 139, 0.16);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  padding: 8px 12px;
  font-size: 14px;
}

.hierarchy-tree-row small {
  color: var(--reader-muted-text, #64748b);
  font-size: 12px;
  white-space: nowrap;
}

/* ---- detail ---- */
.map-detail {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.map-detail-head h3 {
  margin: 0;
}

.map-detail-head p {
  margin: 4px 0 0;
  color: var(--reader-muted-text, #64748b);
}

.map-linked-section {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.map-linked-section strong {
  flex-basis: 100%;
}

.map-linked-section span {
  border: 1px solid rgba(100, 116, 139, 0.16);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  padding: 6px 10px;
  font-size: 13px;
}

/* ---- conflict ---- */
.map-conflict {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 14px;
}

.section-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.map-conflict-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border: 1px solid rgba(100, 116, 139, 0.16);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  padding: 8px 12px;
  font-size: 13px;
}

/* ---- hints ---- */
.map-empty-hint {
  margin: 4px 0 0;
  color: var(--reader-muted-text, #64748b);
  font-size: 14px;
}

.map-warning {
  margin: 4px 0 0;
  color: var(--reader-muted-text, #64748b);
  font-size: 13px;
}
</style>
