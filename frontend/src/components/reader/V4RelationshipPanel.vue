<template>
  <section class="v4-relationship-panel panel-card" :style="bodyStyle" role="tabpanel" aria-label="V4 人物关系">
    <div class="panel-head">
      <div>
        <h2>人物关系</h2>
        <p v-if="total > 0">{{ total }} 条关系，{{ groups.length }} 个分组</p>
        <p v-else>V4 关系数据</p>
      </div>
    </div>

    <!-- Group filter pills -->
    <div v-if="groups.length > 1" class="group-filter">
      <button
        :class="['filter-pill', { active: !activeGroup }]"
        @click="activeGroup = null"
      >全部</button>
      <button
        v-for="g in groups"
        :key="g"
        :class="['filter-pill', `group-${g}`, { active: activeGroup === g }]"
        @click="activeGroup = activeGroup === g ? null : g"
      >{{ groupLabel(g) }} ({{ groupCount(g) }})</button>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="rel-state rel-loading">
      <span></span><span></span><span></span>
    </div>

    <!-- Error -->
    <p v-else-if="error" class="rel-state rel-empty">关系数据加载失败。</p>

    <!-- Empty -->
    <p v-else-if="!filteredEdges.length" class="rel-state rel-empty">
      {{ total > 0 ? '当前筛选无匹配关系' : '暂无关系数据' }}
    </p>

    <!-- Relationship list grouped -->
    <div v-else class="rel-list">
      <section v-for="group in displayGroups" :key="group.name" class="rel-group">
        <div :class="['rel-group-header', `group-${group.name}`]">
          <span :class="['rel-dot', `group-${group.name}`]" aria-hidden="true"></span>
          <strong>{{ groupLabel(group.name) }}</strong>
          <span class="rel-group-count">{{ group.edges.length }}</span>
        </div>
        <div class="rel-items">
          <article
            v-for="edge in group.edges"
            :key="edge.id"
            :class="['rel-item', { collapsed: isCollapsed(edge) }]"
            @click="toggleExpand(edge.id)"
          >
            <div class="rel-item-head">
              <span class="rel-source">{{ nodeName(edge.sourceId) }}</span>
              <span :class="['rel-arrow', { directed: edge.directionality === 'directed' }]">
                {{ edge.directionality === 'directed' ? '→' : '↔' }}
              </span>
              <span class="rel-target">{{ nodeName(edge.targetId) }}</span>
              <span class="rel-label">{{ edge.label }}</span>
            </div>
            <div class="rel-item-meta">
              <span v-if="edge.currentState" class="rel-state-text">{{ edge.currentState }}</span>
              <span :class="['rel-polarity', `polarity-${edge.polarity}`]">{{ polarityLabel(edge.polarity) }}</span>
              <span class="rel-strength">强度 {{ formatStrength(edge.strength) }}</span>
              <span v-if="edge.importanceScore < 0.3" class="rel-low-importance">低重要度</span>
            </div>
            <div v-if="expandedIds.has(edge.id)" class="rel-item-detail">
              <div class="rel-detail-row">
                <span>置信度：{{ (edge.confidence * 100).toFixed(0) }}%</span>
                <span>重要度：{{ edge.importanceScore.toFixed(2) }}</span>
                <span>事件数：{{ edge.eventCount }}</span>
              </div>
              <div class="rel-detail-row">
                <span>首次出现：第{{ edge.firstSeenChapter }}章</span>
                <span v-if="edge.lastChangedChapter !== edge.firstSeenChapter">变更：第{{ edge.lastChangedChapter }}章</span>
                <span>最近：第{{ edge.lastSeenChapter }}章</span>
              </div>
            </div>
          </article>
        </div>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { CSSProperties } from 'vue'
import type { V4RelationshipEdge, V4RelationshipNode, V4RelationshipGraphView } from '../../types/v4'
import { getV4Relationships } from '../../api/v4/book'

const props = defineProps<{
  bookUrl: string
  bodyStyle?: CSSProperties
}>()

const loading = ref(false)
const error = ref(false)
const graphData = ref<V4RelationshipGraphView | null>(null)
const activeGroup = ref<string | null>(null)
const expandedIds = ref(new Set<string>())

const COLLAPSE_THRESHOLD = 0.3

const nodes = computed(() => graphData.value?.nodes ?? [])
const edges = computed(() => graphData.value?.edges ?? [])
const groups = computed(() => graphData.value?.groups ?? [])
const total = computed(() => graphData.value?.total ?? 0)

const nodeMap = computed(() => {
  const map = new Map<string, V4RelationshipNode>()
  for (const n of nodes.value) map.set(n.id, n)
  return map
})

const filteredEdges = computed(() => {
  if (!activeGroup.value) return edges.value
  return edges.value.filter((e) => e.group === activeGroup.value)
})

interface DisplayGroup {
  name: string
  edges: V4RelationshipEdge[]
}

const displayGroups = computed<DisplayGroup[]>(() => {
  const map = new Map<string, V4RelationshipEdge[]>()
  for (const edge of filteredEdges.value) {
    const arr = map.get(edge.group) || []
    arr.push(edge)
    map.set(edge.group, arr)
  }
  // Sort edges within each group by importanceScore desc
  const result: DisplayGroup[] = []
  for (const [name, groupEdges] of map) {
    groupEdges.sort((a, b) => b.importanceScore - a.importanceScore)
    result.push({ name, edges: groupEdges })
  }
  // Sort groups by total edges count desc
  result.sort((a, b) => b.edges.length - a.edges.length)
  return result
})

function groupCount(g: string): number {
  return edges.value.filter((e) => e.group === g).length
}

function nodeName(id: string): string {
  return nodeMap.value.get(id)?.name || id
}

function isCollapsed(edge: V4RelationshipEdge): boolean {
  return edge.importanceScore < COLLAPSE_THRESHOLD && !expandedIds.value.has(edge.id)
}

function toggleExpand(id: string) {
  const s = new Set(expandedIds.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  expandedIds.value = s
}

function formatStrength(v: number): string {
  if (v >= 0.8) return '强'
  if (v >= 0.5) return '中'
  return '弱'
}

const GROUP_LABELS: Record<string, string> = {
  family: '家族',
  romance: '情感',
  friendship: '友情',
  mentorship: '师徒',
  hierarchy: '上下级',
  alliance: '同盟',
  rivalry: '竞争',
  hostility: '敌对',
  debt_obligation: '恩义',
  contract: '契约',
  acquaintance: '相识',
  other_social: '社交',
  unknown_significant: '未分类',
}

function groupLabel(g: string): string {
  return GROUP_LABELS[g] || g
}

function polarityLabel(p: string): string {
  const labels: Record<string, string> = {
    positive: '正面',
    negative: '负面',
    mixed: '复杂',
    neutral: '中性',
    unknown: '未知',
  }
  return labels[p] || p
}

let requestId = 0

async function load() {
  if (!props.bookUrl) return
  const req = ++requestId
  loading.value = true
  error.value = false
  graphData.value = null
  try {
    const data = await getV4Relationships(props.bookUrl)
    if (req !== requestId) return
    graphData.value = data
  } catch {
    if (req !== requestId) return
    error.value = true
  } finally {
    if (req === requestId) loading.value = false
  }
}

watch(() => props.bookUrl, () => {
  activeGroup.value = null
  expandedIds.value = new Set()
  void load()
}, { immediate: true })

defineExpose({ reload: load })
</script>

<style scoped>
.v4-relationship-panel {
  display: grid;
  gap: 14px;
}

.panel-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.panel-head h2 {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
}

.panel-head p {
  margin: 4px 0 0;
  opacity: 0.5;
  font-size: 0.84rem;
}

/* ── Group filter ── */

.group-filter {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.filter-pill {
  display: inline-flex;
  align-items: center;
  padding: 3px 9px;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, currentColor 3%, transparent);
  font-size: 0.76rem;
  line-height: 1.3;
  cursor: pointer;
  transition: all 0.15s;
  color: inherit;
  font-family: inherit;
}

.filter-pill:hover {
  background: color-mix(in srgb, currentColor 7%, transparent);
  border-color: color-mix(in srgb, currentColor 16%, transparent);
}

.filter-pill.active {
  border-color: color-mix(in srgb, var(--color-primary, #c97f3a) 40%, transparent);
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 8%, transparent);
  font-weight: 600;
}

/* ── Relationship list ── */

.rel-list {
  display: grid;
  gap: 14px;
}

.rel-group {
  display: grid;
  gap: 6px;
}

.rel-group-header {
  display: flex;
  gap: 6px;
  align-items: center;
}

.rel-group-header strong {
  font-size: 0.8rem;
  font-weight: 600;
  opacity: 0.75;
  letter-spacing: 0.02em;
}

.rel-group-header .rel-dot {
  margin-top: 0;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
}

.rel-group-count {
  font-size: 0.7rem;
  opacity: 0.4;
  margin-left: 1px;
}

.rel-items {
  display: grid;
  gap: 4px;
}

.rel-item {
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, currentColor 2%, transparent);
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}

.rel-item:hover {
  border-color: color-mix(in srgb, currentColor 14%, transparent);
  background: color-mix(in srgb, currentColor 4%, transparent);
}

.rel-item-head {
  display: flex;
  gap: 5px;
  align-items: center;
  flex-wrap: wrap;
}

.rel-source,
.rel-target {
  font-weight: 600;
  font-size: 0.84rem;
}

.rel-arrow {
  opacity: 0.4;
  font-size: 0.8rem;
}

.rel-arrow.directed {
  opacity: 0.6;
}

.rel-label {
  font-size: 0.78rem;
  opacity: 0.6;
  padding: 1px 5px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 5px;
}

.rel-item-meta {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  margin-top: 4px;
  font-size: 0.74rem;
  opacity: 0.6;
}

.rel-state-text {
  font-style: italic;
}

.rel-polarity {
  padding: 0 4px;
  border-radius: 4px;
  font-size: 0.7rem;
}

.polarity-positive {
  color: #3d8b5e;
}

.polarity-negative {
  color: #c66a5d;
}

.polarity-mixed {
  color: #b8860b;
}

.rel-low-importance {
  opacity: 0.5;
  font-style: italic;
}

/* ── Collapsed state ── */

.rel-item.collapsed .rel-item-meta {
  display: none;
}

/* ── Detail expansion ── */

.rel-item-detail {
  margin-top: 6px;
  padding-top: 6px;
  border-top: 1px solid color-mix(in srgb, currentColor 6%, transparent);
  display: grid;
  gap: 3px;
}

.rel-detail-row {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  font-size: 0.72rem;
  opacity: 0.55;
}

/* ── State messages ── */

.rel-state {
  margin: 0;
}

.rel-loading {
  display: grid;
  gap: 10px;
  padding: 16px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 14px;
}

.rel-loading span {
  height: 12px;
  border-radius: 8px;
  background: linear-gradient(90deg, color-mix(in srgb, currentColor 4%, transparent), color-mix(in srgb, currentColor 10%, transparent), color-mix(in srgb, currentColor 4%, transparent));
}

.rel-empty {
  padding: 2px 0 0;
  opacity: 0.5;
}

/* ── Group colors ── */

.group-family { color: #5f7fc8; }
.group-romance { color: #c86a8e; }
.group-friendship { color: #5d9d72; }
.group-mentorship { color: #7f6bc8; }
.group-hierarchy { color: #8b7355; }
.group-alliance { color: #4a9d8e; }
.group-rivalry { color: #c66a5d; }
.group-hostility { color: #b91c1c; }
.group-debt_obligation { color: #b8860b; }
.group-contract { color: #6b7280; }
.group-acquaintance { color: #94a3b8; }
.group-other_social { color: #64748b; }
.group-unknown_significant { color: currentColor; }

.rel-dot.group-family { background: #5f7fc8; }
.rel-dot.group-romance { background: #c86a8e; }
.rel-dot.group-friendship { background: #5d9d72; }
.rel-dot.group-mentorship { background: #7f6bc8; }
.rel-dot.group-hierarchy { background: #8b7355; }
.rel-dot.group-alliance { background: #4a9d8e; }
.rel-dot.group-rivalry { background: #c66a5d; }
.rel-dot.group-hostility { background: #b91c1c; }
.rel-dot.group-debt_obligation { background: #b8860b; }
.rel-dot.group-contract { background: #6b7280; }
.rel-dot.group-acquaintance { background: #94a3b8; }
.rel-dot.group-other_social { background: #64748b; }
.rel-dot.group-unknown_significant { background: currentColor; }
</style>
