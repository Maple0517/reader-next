<template>
  <section class="v4-map-panel panel-card" :style="bodyStyle" role="tabpanel" aria-label="V4 地图">
    <div class="panel-head">
      <div>
        <h2>地图</h2>
        <p>{{ overviewText }}</p>
      </div>
      <span class="map-pill">V4</span>
    </div>

    <div v-if="loading" class="map-state map-loading">
      <span></span><span></span><span></span>
    </div>

    <p v-else-if="error" class="map-state">地图资料加载失败。</p>

    <div v-else class="map-content">
      <div class="map-stats">
        <span>地点 {{ overview?.placeCount ?? places.length }}</span>
        <span>层级关系 {{ flatHierarchy.length }}</span>
        <span>拓扑边 {{ overview?.activeEdgeCount ?? graph?.edges.length ?? 0 }}</span>
        <span :class="{ warn: conflictCount > 0 }">冲突 {{ conflictCount }}</span>
      </div>

      <div class="map-grid">
        <section class="place-list" aria-label="地点列表">
          <div class="section-head">
            <strong>地点</strong>
            <span>{{ places.length }}</span>
          </div>
          <p v-if="!places.length" class="map-state">暂无地点资料</p>
          <button
            v-for="place in places"
            :key="place.id"
            type="button"
            class="place-row"
            :class="{ active: selectedPlaceId === place.id }"
            :data-test="`map-place-${place.id}`"
            @click="loadPlaceDetail(place.id)"
          >
            <span>{{ place.name }}</span>
            <small>{{ placeTypeLabel(place.placeType) }}</small>
          </button>
        </section>

        <section class="hierarchy-list" aria-label="地点层级">
          <div class="section-head">
            <strong>层级关系</strong>
            <span>{{ flatHierarchy.length }} 个地点</span>
          </div>
          <p v-if="!flatHierarchy.length" class="map-state">暂无层级关系资料</p>
          <div
            v-for="node in flatHierarchy"
            :key="node.placeId"
            class="hierarchy-row"
            :style="{ paddingLeft: `${node.depth * 16 + 10}px` }"
          >
            <span>{{ node.name }}</span>
            <small>{{ placeTypeLabel(node.placeType) }}</small>
          </div>
        </section>

        <section class="graph-panel" aria-label="地图拓扑">
          <div class="section-head">
            <strong>拓扑关系</strong>
            <span>{{ graph?.edges.length ?? 0 }} 条拓扑边</span>
          </div>
          <div v-if="layoutAvailable" class="layout-grid">
            <div v-for="node in layoutNodes" :key="node.placeId" class="layout-node">
              {{ node.label }}
            </div>
          </div>
          <p v-else class="map-state">布局暂不可用，使用拓扑表格展示。</p>
          <article v-for="edge in graph?.edges || []" :key="edge.edgeId" class="edge-row">
            <span>{{ placeName(edge.fromPlaceId) }}</span>
            <strong>{{ edgeTypeLabel(edge.edgeType) }}</strong>
            <span>{{ placeName(edge.toPlaceId) }}</span>
          </article>
          <p v-if="graphWarnings.length" class="map-warning">{{ graphWarnings.join('；') }}</p>
        </section>

        <section class="detail-panel" aria-label="地点详情">
          <p v-if="detailLoading" class="map-state">地点详情加载中...</p>
          <p v-else-if="!selectedDetail" class="map-state">选择一个地点查看详情。</p>
          <template v-else>
            <div class="detail-head">
              <div>
                <h3>{{ selectedDetail.name }}</h3>
                <p>{{ placeTypeLabel(selectedDetail.placeType) }}</p>
              </div>
            </div>
            <section v-if="selectedDetail.linkedOrganizations.length" class="linked-section">
              <strong>关联组织</strong>
              <span
                v-for="organization in selectedDetail.linkedOrganizations"
                :key="`${organization.id}-${organization.linkType}`"
              >
                {{ organization.name }} · {{ linkTypeLabel(organization.linkType) }}
              </span>
            </section>
          </template>
        </section>
      </div>

      <section v-if="conflicts.length" class="conflict-panel" aria-label="地图冲突">
        <div class="section-head">
          <strong>冲突与不确定</strong>
          <span>{{ conflicts.length }}</span>
        </div>
        <article v-for="conflict in conflicts" :key="conflict.id" class="conflict-row">
          <strong>{{ conflictTypeLabel(conflict.conflictType) }}</strong>
          <span>{{ conflict.reasonCode }}</span>
          <small>{{ conflict.status }}</small>
        </article>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { CSSProperties } from 'vue'
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
  bodyStyle?: CSSProperties
}>()

type FlatHierarchyNode = V4PlaceHierarchyNode & { depth: number }

const loading = ref(false)
const detailLoading = ref(false)
const error = ref(false)
const overview = ref<V4MapOverviewView | null>(null)
const places = ref<V4PlaceSummaryView[]>([])
const hierarchy = ref<V4PlaceHierarchyNode[]>([])
const graph = ref<V4MapGraphView | null>(null)
const layout = ref<V4MapLayoutView | null>(null)
const conflicts = ref<V4MapConflictView[]>([])
const selectedPlaceId = ref('')
const selectedDetail = ref<V4PlaceDetailView | null>(null)
let requestId = 0
let detailRequestId = 0

const overviewText = computed(() => {
  const placeCount = overview.value?.placeCount ?? places.value.length
  const hierarchyCount = flatHierarchy.value.length
  const edgeCount = overview.value?.activeEdgeCount ?? graph.value?.edges.length ?? 0
  return `${placeCount} 个地点 · ${hierarchyCount} 个层级关系 · ${edgeCount} 条拓扑边`
})

const conflictCount = computed(() => conflicts.value.length || overview.value?.conflictCount || 0)

const flatHierarchy = computed(() => {
  const result: FlatHierarchyNode[] = []
  const visit = (nodes: V4PlaceHierarchyNode[], depth: number) => {
    for (const node of nodes) {
      result.push({ ...node, depth })
      visit(node.children || [], depth + 1)
    }
  }
  visit(hierarchy.value, 0)
  return result
})

const layoutNodes = computed(() => {
  if (graph.value?.layout?.nodes?.length) return graph.value.layout.nodes
  return layout.value?.nodes || []
})

const layoutAvailable = computed(() => layoutNodes.value.length > 0)

const graphWarnings = computed(() => {
  const warnings = new Set<string>()
  for (const warning of graph.value?.warnings || []) warnings.add(warning)
  for (const warning of layout.value?.warnings || []) warnings.add(warning)
  return Array.from(warnings)
})

function placeName(placeId: string): string {
  return places.value.find((place) => place.id === placeId)?.name
    || graph.value?.nodes.find((node) => node.placeId === placeId)?.label
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

async function loadMap() {
  if (!props.bookUrl) return
  const req = ++requestId
  loading.value = true
  error.value = false
  selectedDetail.value = null
  selectedPlaceId.value = ''
  try {
    const [overviewData, placesData, graphData, layoutData, conflictsData] = await Promise.all([
      getV4Map(props.bookUrl),
      getV4MapPlaces(props.bookUrl),
      getV4MapGraph(props.bookUrl),
      getV4MapLayout(props.bookUrl),
      getV4MapConflicts(props.bookUrl),
    ])
    if (req !== requestId) return
    overview.value = overviewData
    places.value = placesData.places || []
    hierarchy.value = placesData.hierarchy || []
    graph.value = graphData
    layout.value = layoutData
    conflicts.value = conflictsData.conflicts || []
  } catch {
    if (req !== requestId) return
    error.value = true
    places.value = []
    hierarchy.value = []
    graph.value = null
    layout.value = null
    conflicts.value = []
  } finally {
    if (req === requestId) loading.value = false
  }
}

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

watch(() => props.bookUrl, () => {
  void loadMap()
}, { immediate: true })

defineExpose({ reload: loadMap })
</script>

<style scoped>
.v4-map-panel,
.map-content,
.place-list,
.hierarchy-list,
.graph-panel,
.detail-panel,
.conflict-panel {
  display: grid;
  gap: 14px;
}

.panel-head,
.section-head,
.detail-head,
.edge-row,
.conflict-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.panel-head h2,
.detail-head h3 {
  margin: 0;
}

.panel-head p,
.detail-head p,
.map-state,
.map-warning {
  margin: 4px 0 0;
  color: var(--reader-muted-text, #64748b);
}

.map-pill,
.map-stats span {
  border: 1px solid rgba(100, 116, 139, 0.22);
  border-radius: 999px;
  padding: 4px 10px;
  color: var(--reader-muted-text, #64748b);
  font-size: 12px;
}

.map-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.map-stats .warn {
  border-color: rgba(217, 119, 6, 0.35);
  color: #b45309;
}

.map-grid {
  display: grid;
  grid-template-columns: minmax(180px, 0.9fr) minmax(180px, 1fr);
  gap: 14px;
}

.graph-panel,
.detail-panel {
  min-width: 0;
}

.place-row,
.hierarchy-row,
.edge-row,
.conflict-row,
.linked-section span,
.layout-node {
  border: 1px solid rgba(100, 116, 139, 0.16);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.72);
  padding: 10px 12px;
}

.place-row {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.place-row.active {
  border-color: rgba(37, 99, 235, 0.35);
  background: rgba(37, 99, 235, 0.08);
}

.hierarchy-row {
  display: flex;
  justify-content: space-between;
}

.edge-row strong {
  color: #2563eb;
}

.layout-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 10px;
}

.linked-section {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.linked-section strong {
  flex-basis: 100%;
}

.map-loading {
  display: inline-flex;
  gap: 6px;
}

.map-loading span {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: #94a3b8;
}

@media (max-width: 760px) {
  .map-grid {
    grid-template-columns: 1fr;
  }
}
</style>
