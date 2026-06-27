<template>
  <section class="chapter-summary-relationship-panel panel-card" :style="bodyStyle" role="tabpanel" aria-label="人物关系">
    <div class="panel-head">
      <div>
        <h2>人物关系</h2>
        <p>主角居中，点击角色查看详情。</p>
      </div>
    </div>

    <div v-if="status === 'loading'" class="relationship-state relationship-loading" aria-label="人物关系加载中">
      <span></span>
      <span></span>
      <span></span>
    </div>

    <p v-else-if="status === 'error'" class="relationship-state relationship-empty">人物关系暂时加载失败。</p>

    <p v-else-if="!graph.protagonist" class="relationship-state relationship-empty">{{ graph.emptyReason || '暂无人物关系资料，可先生成 AI资料。' }}</p>

    <div v-else class="relationship-layout">
      <!-- 圆形关系图谱 -->
      <div class="relationship-map" aria-label="人物关系图">
        <svg class="relationship-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <defs>
            <filter id="relationship-line-soften" x="-20%" y="-20%" width="140%" height="140%">
              <feGaussianBlur stdDeviation="0.35" />
            </filter>
          </defs>
          <g filter="url(#relationship-line-soften)">
            <path
              v-for="link in graph.links"
              :key="link.id"
              :class="['relationship-link', `tone-${link.tone}`, `strength-${link.strength}`]"
              :d="link.path"
            />
          </g>
        </svg>

        <div
          v-for="node in graph.nodes"
          :key="node.id"
          :class="['relationship-node', { protagonist: node.isProtagonist }]"
          :style="{ left: `${node.x}%`, top: `${node.y}%` }"
          @click="!node.isProtagonist && selectNode(node.id)"
        >
          <strong>{{ node.name }}</strong>
        </div>
      </div>

      <!-- 分组胶囊 -->
      <div class="relationship-groups">
        <section v-for="group in graph.groupedRows" :key="group.tone" class="relationship-group">
          <div :class="['relationship-group-header', `tone-${group.tone}`]">
            <span :class="['relationship-dot', `tone-${group.tone}`]" aria-hidden="true"></span>
            <strong>{{ group.label }}</strong>
            <span class="relationship-group-count">{{ group.rows.length }}</span>
          </div>
          <div class="relationship-pills">
            <button
              v-for="row in group.rows"
              :key="row.id"
              :class="['relationship-pill', `tone-${row.tone}`, { active: selectedId === row.id }]"
              @click="selectNode(row.id)"
            >
              <span class="pill-name">{{ row.name }}</span>
              <span class="pill-label">{{ row.label }}</span>
            </button>
          </div>
        </section>
      </div>

      <!-- 选中角色详情 popover -->
      <Transition name="detail-pop">
        <div v-if="selectedRow" class="relationship-popover" :class="`tone-${selectedRow.tone}`">
          <div class="popover-head">
            <span :class="['relationship-dot', `tone-${selectedRow.tone}`]" aria-hidden="true"></span>
            <strong>{{ selectedRow.name }}</strong>
            <span class="popover-tag">{{ selectedRow.label }}</span>
            <button class="popover-close" @click="selectedId = null" aria-label="关闭">×</button>
          </div>
          <p class="popover-summary">{{ selectedRow.summary }}</p>
        </div>
      </Transition>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { CSSProperties } from 'vue'
import type { SummaryRelationshipGraphView } from '../../utils/summaryRelationshipGraph'

const props = defineProps<{
  graph: SummaryRelationshipGraphView
  status: 'idle' | 'loading' | 'ready' | 'error'
  bodyStyle: CSSProperties
}>()

const selectedId = ref<string | null>(null)

const selectedRow = computed(() => {
  if (!selectedId.value) return null
  for (const group of props.graph.groupedRows) {
    const found = group.rows.find((r) => r.id === selectedId.value)
    if (found) return found
  }
  return null
})

function selectNode(id: string) {
  selectedId.value = selectedId.value === id ? null : id
}
</script>

<style scoped>
.chapter-summary-relationship-panel {
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

/* ── 图谱 ── */

.relationship-layout {
  display: grid;
  gap: 14px;
  position: relative;
}

.relationship-map {
  position: relative;
  container-type: inline-size;
  min-height: 220px;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 16px;
  background:
    radial-gradient(circle at 50% 50%, color-mix(in srgb, var(--color-primary, #c97f3a) 10%, transparent), transparent 40%),
    color-mix(in srgb, currentColor 2%, transparent);
}

.relationship-map::before {
  content: '';
  position: absolute;
  inset: 14px;
  border-radius: 999px;
  border: 1px dashed color-mix(in srgb, currentColor 6%, transparent);
  pointer-events: none;
}

.relationship-lines {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

.relationship-link {
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  opacity: 0.28;
  vector-effect: non-scaling-stroke;
}

.relationship-link.strength-critical,
.relationship-link.strength-strong {
  opacity: 0.52;
  stroke-width: 0.9;
}

.relationship-link.strength-moderate {
  opacity: 0.38;
  stroke-width: 0.7;
}

.relationship-link.strength-weak,
.relationship-link.strength-unknown {
  opacity: 0.2;
  stroke-width: 0.5;
}

/* ── 图谱节点 ── */

.relationship-node {
  position: absolute;
  transform: translate(-50%, -50%);
  min-width: 52px;
  max-width: 80px;
  padding: 5px 10px;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 10px;
  color: inherit;
  background: color-mix(in srgb, var(--color-bg, #fff) 85%, transparent);
  text-align: center;
  backdrop-filter: blur(6px);
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.relationship-node:hover {
  border-color: color-mix(in srgb, currentColor 20%, transparent);
  box-shadow: 0 2px 12px color-mix(in srgb, #000 8%, transparent);
}

.relationship-node.protagonist {
  min-width: 60px;
  max-width: 88px;
  padding: 7px 12px;
  border-color: color-mix(in srgb, var(--color-primary, #c97f3a) 35%, transparent);
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 12%, var(--color-bg, #fff) 88%);
  cursor: default;
  font-weight: 600;
}

.relationship-node strong {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.82rem;
  font-weight: 600;
  line-height: 1.3;
}

/* ── 分组胶囊 ── */

.relationship-groups {
  display: grid;
  gap: 12px;
}

.relationship-group {
  display: grid;
  gap: 6px;
}

.relationship-group-header {
  display: flex;
  gap: 6px;
  align-items: center;
}

.relationship-group-header strong {
  font-size: 0.8rem;
  font-weight: 600;
  opacity: 0.75;
  letter-spacing: 0.02em;
}

.relationship-group-header .relationship-dot {
  margin-top: 0;
  width: 6px;
  height: 6px;
}

.relationship-group-count {
  font-size: 0.7rem;
  opacity: 0.4;
  margin-left: 1px;
}

.relationship-pills {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.relationship-pill {
  display: inline-flex;
  gap: 4px;
  align-items: center;
  padding: 4px 10px;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, currentColor 3%, transparent);
  font-size: 0.8rem;
  line-height: 1.3;
  cursor: pointer;
  transition: all 0.15s;
  color: inherit;
  font-family: inherit;
}

.relationship-pill:hover {
  background: color-mix(in srgb, currentColor 7%, transparent);
  border-color: color-mix(in srgb, currentColor 16%, transparent);
}

.relationship-pill.active {
  border-color: color-mix(in srgb, var(--color-primary, #c97f3a) 40%, transparent);
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 8%, transparent);
}

.pill-name {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100px;
}

.pill-label {
  opacity: 0.5;
  font-size: 0.72rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── Popover 详情 ── */

.relationship-popover {
  position: relative;
  padding: 12px 14px;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, var(--color-bg, #fff) 95%, transparent);
  box-shadow: 0 4px 20px color-mix(in srgb, #000 8%, transparent);
}

.popover-head {
  display: flex;
  gap: 6px;
  align-items: center;
}

.popover-head strong {
  font-size: 0.92rem;
  font-weight: 700;
}

.popover-head .relationship-dot {
  margin-top: 0;
  width: 7px;
  height: 7px;
}

.popover-tag {
  font-size: 0.72rem;
  opacity: 0.55;
  padding: 1px 6px;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 6px;
}

.popover-close {
  margin-left: auto;
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: inherit;
  opacity: 0.4;
  font-size: 1.1rem;
  cursor: pointer;
  border-radius: 6px;
  transition: opacity 0.15s, background 0.15s;
}

.popover-close:hover {
  opacity: 0.8;
  background: color-mix(in srgb, currentColor 6%, transparent);
}

.popover-summary {
  margin: 8px 0 0;
  font-size: 0.84rem;
  line-height: 1.55;
  opacity: 0.72;
}

/* ── Popover 动画 ── */

.detail-pop-enter-active,
.detail-pop-leave-active {
  transition: opacity 0.18s, transform 0.18s;
}

.detail-pop-enter-from,
.detail-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

/* ── Tone 颜色 ── */

.tone-family {
  color: #5f7fc8;
  stroke: #5f7fc8;
}

.tone-romance {
  color: #c86a8e;
  stroke: #c86a8e;
}

.tone-ally {
  color: #5d9d72;
  stroke: #5d9d72;
}

.tone-conflict {
  color: #c66a5d;
  stroke: #c66a5d;
}

.tone-affiliation {
  color: #7f6bc8;
  stroke: #7f6bc8;
}

.tone-neutral {
  color: currentColor;
  stroke: currentColor;
}

/* ── 状态 ── */

.relationship-state {
  margin: 0;
}

.relationship-loading {
  display: grid;
  gap: 10px;
  padding: 16px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 14px;
}

.relationship-loading span {
  height: 12px;
  border-radius: 8px;
  background: linear-gradient(90deg, color-mix(in srgb, currentColor 4%, transparent), color-mix(in srgb, currentColor 10%, transparent), color-mix(in srgb, currentColor 4%, transparent));
}

.relationship-empty {
  padding: 2px 0 0;
  opacity: 0.5;
}

/* ── 响应式 ── */

@container (max-width: 320px) {
  .relationship-map {
    min-height: 190px;
  }

  .relationship-node {
    min-width: 46px;
    max-width: 68px;
    padding: 4px 7px;
  }

  .relationship-node strong {
    font-size: 0.75rem;
  }
}

@media (max-width: 720px) {
  .relationship-map {
    min-height: 190px;
  }

  .relationship-node {
    min-width: 48px;
    max-width: 72px;
    padding: 4px 8px;
  }
}
</style>
