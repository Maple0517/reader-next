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

      <!-- Empty from filter -->
      <p v-if="!filteredEdges.length && total > 0" class="rel-filter-empty">
        当前筛选无匹配关系
      </p>

      <!-- Relationship list grouped -->
      <div v-else-if="filteredEdges.length" class="rel-list">
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
              :class="['rel-item', { 'low-importance': edge.importanceScore < 0.3 }]"
              @click="toggleExpand(edge.id)"
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
  border: 0;
  border-radius: 999px;
  background: var(--color-primary);
  color: #fff;
  font-weight: 800;
  cursor: pointer;
  transition: transform 220ms cubic-bezier(0.32, 0.72, 0, 1), opacity 220ms cubic-bezier(0.32, 0.72, 0, 1);
}

.v4-relationship-refresh:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.v4-relationship-refresh:active:not(:disabled) {
  transform: translateY(1px) scale(0.98);
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
  border-radius: 8px;
  background: color-mix(in srgb, currentColor 3%, transparent);
  font-size: 0.76rem;
  line-height: 1.3;
  cursor: pointer;
  transition: all 0.15s;
  color: inherit;
  font-family: inherit;
}

.group-pill:hover {
  background: color-mix(in srgb, currentColor 7%, transparent);
  border-color: color-mix(in srgb, currentColor 16%, transparent);
}

.group-pill.active {
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
  border-radius: 5px;
}

.rel-polarity {
  padding: 0 4px;
  border-radius: 4px;
  font-size: 0.7rem;
  margin-left: auto;
}

.rel-strength {
  font-size: 0.72rem;
  opacity: 0.5;
}

.polarity-positive {
  color: #28a745;
  background: rgba(40, 167, 69, 0.1);
}

.polarity-negative {
  color: #dc3545;
  background: rgba(220, 53, 69, 0.08);
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
