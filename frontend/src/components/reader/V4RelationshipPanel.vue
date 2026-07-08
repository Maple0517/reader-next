<template>
  <section class="v4-relationship-panel" :style="bodyStyle" role="tabpanel" aria-label="人物关系">
    <V4PanelShell
      title="人物关系"
      :subtitle="panelSubtitle"
      :loading="listState.loading.value"
      :error="listState.error.value"
      :empty="listState.empty.value"
      empty-title="暂无关系"
      empty-message="当前资料还没有可展示的人物关系。"
      @retry="listState.reload()"
    >
      <template #toolbar>
        <button
          class="v4-relationship-refresh"
          type="button"
          :disabled="listState.loading.value"
          @click="listState.reload()"
        >刷新</button>
      </template>

      <!-- Group filter pills -->
      <div v-if="groups.length > 1" class="group-filter">
        <button
          :class="['group-pill', { active: !activeGroup }]"
          @click="activeGroup = null"
        >全部</button>
        <button
          v-for="g in groups"
          :key="g"
          :class="['group-pill', `group-color-${g}`, { active: activeGroup === g }]"
          @click="activeGroup = activeGroup === g ? null : g"
        >{{ groupLabel(g) }} ({{ groupCount(g) }})</button>
      </div>

      <p v-if="!filteredEdges.length && total > 0" class="rel-filter-empty">
        当前筛选无匹配关系
      </p>

      <div v-else-if="filteredEdges.length" class="relationship-workbench">
        <section class="relationship-edge-list" aria-label="关系边列表" data-test="relationship-edge-list">
          <section v-for="group in displayGroups" :key="group.name" class="rel-group">
            <div :class="['rel-group-header', `group-color-${group.name}`]">
              <span :class="['rel-dot', `group-color-${group.name}`]" aria-hidden="true"></span>
              <strong>{{ groupLabel(group.name) }}</strong>
              <span class="rel-group-count">{{ group.edges.length }}</span>
            </div>
            <div class="rel-items">
              <article
                v-for="edge in group.edges"
                :key="edge.id"
                :data-edge-id="edge.id"
                :class="['rel-item', { active: selectedEdgeId === edge.id, 'low-importance': edge.importanceScore < 0.3 }]"
                @click="selectEdge(edge.id)"
              >
                <div class="rel-item-head">
                  <span class="rel-source">{{ nodeName(edge.sourceId) }}</span>
                  <span class="rel-arrow">{{ edge.directionality === 'directed' ? '→' : '↔' }}</span>
                  <span class="rel-target">{{ nodeName(edge.targetId) }}</span>
                  <span class="rel-label">{{ edge.label }}</span>
                  <span :class="['rel-polarity', `polarity-${edge.polarity}`]">{{ polarityLabel(edge.polarity) }}</span>
                  <span class="rel-strength">{{ formatStrength(edge.strength) }}</span>
                </div>
                <div
                  v-if="expandedIds.has(edge.id)"
                  :data-edge-detail="edge.id"
                  class="rel-item-detail"
                >
                  <div v-if="edge.currentState" class="rel-detail-row">
                    <span>{{ edge.currentState }}</span>
                  </div>
                  <div class="rel-detail-row">
                    <span>置信度 {{ (edge.confidence * 100).toFixed(0) }}%</span>
                    <span>第{{ edge.firstSeenChapter }}~{{ edge.lastSeenChapter }}章</span>
                  </div>
                </div>
              </article>
            </div>
          </section>
        </section>

        <section class="relationship-network" aria-label="关系网络" data-test="relationship-network">
          <div class="relationship-network-nodes">
            <span
              v-for="node in visibleNetworkNodes"
              :key="node.id"
              class="relationship-node"
              :class="{ active: selectedEdge && (selectedEdge.sourceId === node.id || selectedEdge.targetId === node.id) }"
            >
              {{ node.name }}
            </span>
          </div>
          <div class="relationship-network-edges">
            <button
              v-for="edge in filteredEdges"
              :key="edge.id"
              type="button"
              :class="['relationship-network-edge', `group-color-${edge.group}`, { active: selectedEdgeId === edge.id }]"
              @click="selectEdge(edge.id)"
            >
              <span>{{ nodeName(edge.sourceId) }}</span>
              <strong>{{ edge.directionality === 'directed' ? '→' : '↔' }} {{ edge.label }}</strong>
              <span>{{ nodeName(edge.targetId) }}</span>
            </button>
          </div>
        </section>

        <aside class="relationship-edge-inspector" aria-label="选中关系" data-test="relationship-edge-inspector">
          <div class="relationship-inspector-head">
            <h3>选中关系</h3>
            <span v-if="selectedEdge" :class="['rel-label', `group-color-${selectedEdge.group}`]">{{ groupLabel(selectedEdge.group) }}</span>
          </div>
          <template v-if="selectedEdge">
            <div class="relationship-inspector-title">
              <strong>{{ nodeName(selectedEdge.sourceId) }}</strong>
              <span>{{ selectedEdge.directionality === 'directed' ? '→' : '↔' }}</span>
              <strong>{{ nodeName(selectedEdge.targetId) }}</strong>
            </div>
            <dl class="relationship-inspector-facts">
              <div><dt>标签</dt><dd>{{ selectedEdge.label }}</dd></div>
              <div v-if="selectedEdge.currentState"><dt>当前状态</dt><dd>{{ selectedEdge.currentState }}</dd></div>
              <div><dt>立场</dt><dd>{{ polarityLabel(selectedEdge.polarity) }}</dd></div>
              <div><dt>强度</dt><dd>{{ formatStrength(selectedEdge.strength) }}</dd></div>
              <div><dt>置信</dt><dd>{{ (selectedEdge.confidence * 100).toFixed(0) }}%</dd></div>
              <div><dt>事件</dt><dd>事件 {{ selectedEdge.eventCount }}</dd></div>
              <div><dt>章节</dt><dd>第{{ selectedEdge.firstSeenChapter }}~{{ selectedEdge.lastSeenChapter }}章</dd></div>
              <div><dt>证据</dt><dd>{{ selectedEdge.evidenceAvailable ? '有证据可追溯' : '证据待补齐' }}</dd></div>
            </dl>
          </template>
          <p v-else class="relationship-inspector-empty">选择一条关系查看状态、强度和证据摘要。</p>
        </aside>
      </div>
    </V4PanelShell>
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { CSSProperties } from 'vue'
import type { V4RelationshipEdge } from '../../types/v4'
import { getV4Relationships } from '../../api/v4/book'
import { useV4PanelState } from '../../composables/useV4PanelState'
import V4PanelShell from './v4/V4PanelShell.vue'

const props = defineProps<{
  bookUrl: string
  bodyStyle?: CSSProperties
}>()

const listState = useV4PanelState(
  () => getV4Relationships(props.bookUrl),
  computed(() => props.bookUrl),
  { emptyCheck: (d) => d.edges.length === 0 },
)

const activeGroup = ref<string | null>(null)
const expandedIds = ref(new Set<string>())
const selectedEdgeId = ref<string | null>(null)

const nodes = computed(() => listState.data.value?.nodes ?? [])
const edges = computed(() => listState.data.value?.edges ?? [])
const groups = computed(() => listState.data.value?.groups ?? [])
const total = computed(() => listState.data.value?.total ?? 0)

const panelSubtitle = computed(() => {
  if (total.value > 0) return `${total.value} 条关系，${groups.value.length} 个分组`
  return ''
})

const nodeMap = computed(() => {
  const map = new Map<string, { name: string }>()
  for (const n of nodes.value) map.set(n.id, n)
  return map
})

const filteredEdges = computed(() => {
  if (!activeGroup.value) return edges.value
  return edges.value.filter((e) => e.group === activeGroup.value)
})
const selectedEdge = computed(() => {
  if (!selectedEdgeId.value) return null
  return edges.value.find((edge) => edge.id === selectedEdgeId.value) ?? null
})
const visibleNetworkNodes = computed(() => {
  const ids = new Set<string>()
  for (const edge of filteredEdges.value) {
    ids.add(edge.sourceId)
    ids.add(edge.targetId)
  }
  return nodes.value.filter((node) => ids.has(node.id))
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
  const result: DisplayGroup[] = []
  for (const [name, groupEdges] of map) {
    groupEdges.sort((a, b) => b.importanceScore - a.importanceScore)
    result.push({ name, edges: groupEdges })
  }
  result.sort((a, b) => b.edges.length - a.edges.length)
  return result
})

function groupCount(g: string): number {
  return edges.value.filter((e) => e.group === g).length
}

function nodeName(id: string): string {
  return nodeMap.value.get(id)?.name || id
}

function selectEdge(id: string) {
  selectedEdgeId.value = id
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

defineExpose({ reload: () => listState.reload() })
</script>

<style scoped>
.v4-relationship-panel {
  display: grid;
  gap: 14px;
}

.v4-relationship-refresh {
  min-height: 34px;
  padding: 0 14px;
  border: 1px solid var(--v4-line);
  background: var(--v4-accent);
  color: #fff;
  font-weight: 800;
  cursor: pointer;
}

.v4-relationship-refresh:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

/* ── Group filter ── */

.group-filter {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-bottom: 10px;
}

.group-pill {
  display: inline-flex;
  align-items: center;
  padding: 3px 9px;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  background: color-mix(in srgb, currentColor 3%, transparent);
  font-size: 0.76rem;
  line-height: 1.3;
  cursor: pointer;
  color: inherit;
  font-family: inherit;
}

.group-pill:hover {
  background: color-mix(in srgb, currentColor 7%, transparent);
  border-color: color-mix(in srgb, currentColor 16%, transparent);
}

.group-pill.active {
  border-color: color-mix(in srgb, var(--v4-accent, #c97f3a) 40%, transparent);
  background: color-mix(in srgb, var(--v4-accent, #c97f3a) 8%, transparent);
  font-weight: 600;
}

.relationship-workbench {
  display: grid;
  grid-template-columns: minmax(230px, 0.85fr) minmax(280px, 1.2fr) minmax(240px, 0.95fr);
  gap: 14px;
  align-items: start;
}

.relationship-edge-list {
  display: grid;
  gap: 14px;
  max-height: 680px;
  overflow: auto;
  padding-right: 2px;
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
  background: color-mix(in srgb, currentColor 2%, transparent);
  cursor: pointer;
}

.rel-item:hover {
  border-color: color-mix(in srgb, currentColor 14%, transparent);
  background: color-mix(in srgb, currentColor 4%, transparent);
}

.rel-item.active {
  border-color: color-mix(in srgb, var(--v4-accent) 42%, transparent);
  background: color-mix(in srgb, var(--v4-accent) 9%, transparent);
}

.rel-item.low-importance {
  opacity: 0.6;
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

.rel-label {
  font-size: 0.78rem;
  opacity: 0.6;
  padding: 1px 5px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
}

.rel-polarity {
  padding: 0 4px;
  font-size: 0.7rem;
  margin-left: auto;
}

.rel-strength {
  font-size: 0.72rem;
  opacity: 0.5;
}

.polarity-positive {
  color: var(--v4-success);
}

.polarity-negative {
  color: var(--v4-danger);
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

.rel-filter-empty {
  margin: 0;
  padding: 2px 0 0;
  opacity: 0.5;
}

.relationship-network,
.relationship-edge-inspector {
  display: grid;
  gap: 12px;
  padding: 14px;
  background: var(--v4-soft);
  border: 1px solid var(--v4-line);
}

.relationship-network-nodes {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  min-height: 112px;
  align-content: center;
  justify-content: center;
  padding: 14px;
  background: var(--v4-bg);
  border: 1px solid var(--v4-line);
}

.relationship-node {
  display: inline-flex;
  min-height: 34px;
  align-items: center;
  padding: 0 11px;
  background: var(--v4-soft);
  color: var(--v4-ink);
  font-size: 12px;
  font-weight: 850;
  border: 1px solid var(--v4-line);
}

.relationship-node.active {
  background: color-mix(in srgb, var(--v4-accent) 14%, var(--v4-bg));
  color: var(--v4-accent);
}

.relationship-network-edges {
  display: grid;
  gap: 7px;
}

.relationship-network-edge {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  gap: 8px;
  align-items: center;
  min-height: 34px;
  padding: 0 10px;
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  background: color-mix(in srgb, currentColor 3%, transparent);
  color: inherit;
  cursor: pointer;
}

.relationship-network-edge.active {
  background: color-mix(in srgb, currentColor 11%, transparent);
  font-weight: 850;
}

.relationship-network-edge span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.relationship-network-edge span:last-child {
  text-align: right;
}

.relationship-inspector-head,
.relationship-inspector-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.relationship-inspector-head h3 {
  margin: 0;
  color: var(--v4-ink);
  font-size: 16px;
}

.relationship-inspector-title {
  justify-content: flex-start;
  color: var(--v4-ink);
}

.relationship-inspector-facts {
  display: grid;
  gap: 8px;
  margin: 0;
}

.relationship-inspector-facts div {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 8px;
  padding: 8px 0;
  border-bottom: 1px solid var(--v4-line);
}

.relationship-inspector-facts dt,
.relationship-inspector-empty {
  color: var(--v4-muted);
  font-size: 12px;
}

.relationship-inspector-facts dd {
  margin: 0;
  color: var(--v4-muted);
  line-height: 1.45;
}

@media (max-width: 980px) {
  .relationship-workbench {
    grid-template-columns: 1fr;
  }
}

/* ── Group colors ── */

.group-color-family { color: #5f7fc8; }
.group-color-romance { color: #c86a8e; }
.group-color-friendship { color: #5d9d72; }
.group-color-mentorship { color: #7f6bc8; }
.group-color-hierarchy { color: #8b7355; }
.group-color-alliance { color: #4a9d8e; }
.group-color-rivalry { color: #c66a5d; }
.group-color-hostility { color: #b91c1c; }
.group-color-debt_obligation { color: #b8860b; }
.group-color-contract { color: #6b7280; }
.group-color-acquaintance { color: #94a3b8; }
.group-color-other_social { color: #64748b; }
.group-color-unknown_significant { color: currentColor; }

.rel-dot.group-color-family { background: #5f7fc8; }
.rel-dot.group-color-romance { background: #c86a8e; }
.rel-dot.group-color-friendship { background: #5d9d72; }
.rel-dot.group-color-mentorship { background: #7f6bc8; }
.rel-dot.group-color-hierarchy { background: #8b7355; }
.rel-dot.group-color-alliance { background: #4a9d8e; }
.rel-dot.group-color-rivalry { background: #c66a5d; }
.rel-dot.group-color-hostility { background: #b91c1c; }
.rel-dot.group-color-debt_obligation { background: #b8860b; }
.rel-dot.group-color-contract { background: #6b7280; }
.rel-dot.group-color-acquaintance { background: #94a3b8; }
.rel-dot.group-color-other_social { background: #64748b; }
.rel-dot.group-color-unknown_significant { background: currentColor; }
</style>
