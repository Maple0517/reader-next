<template>
  <section class="chapter-summary-relationship-panel panel-card" :style="bodyStyle" role="tabpanel" aria-label="人物关系">
    <div class="panel-head">
      <div>
        <h2>人物关系</h2>
        <p>主角居中，展示与当前章节最相关的一跳关系。</p>
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
        >
          <strong>{{ node.name }}</strong>
          <small>{{ node.description || (node.isProtagonist ? '主角' : '相关角色') }}</small>
        </div>
      </div>

      <div class="relationship-rows">
        <article v-for="row in graph.rows" :key="row.id" class="relationship-row">
          <span :class="['relationship-dot', `tone-${row.tone}`]" aria-hidden="true"></span>
          <div>
            <strong>{{ row.name }}｜{{ row.label }}</strong>
            <small>{{ row.summary }}</small>
          </div>
        </article>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { CSSProperties } from 'vue'
import type { SummaryRelationshipGraphView } from '../../utils/summaryRelationshipGraph'

defineProps<{
  graph: SummaryRelationshipGraphView
  status: 'idle' | 'loading' | 'ready' | 'error'
  bodyStyle: CSSProperties
}>()
</script>

<style scoped>
.chapter-summary-relationship-panel {
  display: grid;
  gap: 12px;
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
  opacity: 0.68;
  font-size: 0.88rem;
}

.relationship-layout {
  display: grid;
  gap: 12px;
}

.relationship-map {
  position: relative;
  container-type: inline-size;
  min-height: 264px;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 18px;
  background:
    radial-gradient(circle at 50% 50%, color-mix(in srgb, var(--color-primary, #c97f3a) 15%, transparent), transparent 35%),
    linear-gradient(180deg, color-mix(in srgb, currentColor 3%, transparent), color-mix(in srgb, currentColor 1%, transparent));
}

.relationship-map::before {
  content: '';
  position: absolute;
  inset: 18px;
  border-radius: 999px;
  border: 1px dashed color-mix(in srgb, currentColor 8%, transparent);
  opacity: 0.75;
  pointer-events: none;
}

.relationship-map::after {
  content: '';
  position: absolute;
  inset: 30% 18%;
  border-radius: 999px;
  border: 1px dashed color-mix(in srgb, currentColor 7%, transparent);
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
  opacity: 0.34;
  vector-effect: non-scaling-stroke;
}

.relationship-link.strength-critical,
.relationship-link.strength-strong {
  opacity: 0.62;
  stroke-width: 0.95;
}

.relationship-link.strength-moderate {
  opacity: 0.48;
  stroke-width: 0.78;
}

.relationship-link.strength-weak,
.relationship-link.strength-unknown {
  opacity: 0.28;
  stroke-width: 0.62;
}

.relationship-node {
  position: absolute;
  transform: translate(-50%, -50%);
  display: grid;
  gap: 2px;
  min-width: 64px;
  max-width: clamp(72px, 31%, 88px);
  padding: 7px 9px;
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  border-radius: 999px;
  color: inherit;
  background: color-mix(in srgb, currentColor 4%, transparent);
  box-shadow: 0 12px 28px color-mix(in srgb, #000 10%, transparent);
  text-align: center;
  backdrop-filter: blur(4px);
}

.relationship-node.protagonist {
  min-width: 78px;
  max-width: clamp(84px, 34%, 98px);
  padding: 10px 11px;
  border-color: color-mix(in srgb, var(--color-primary, #c97f3a) 42%, transparent);
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 16%, transparent);
  box-shadow: 0 14px 32px color-mix(in srgb, var(--color-primary, #c97f3a) 14%, transparent);
}

.relationship-node strong,
.relationship-row strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.9rem;
}

.relationship-node small,
.relationship-row small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.64;
  font-size: 0.78rem;
}

.relationship-rows {
  display: grid;
  gap: 8px;
}

.relationship-row {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, currentColor 9%, transparent);
  border-radius: 14px;
  background: color-mix(in srgb, currentColor 3%, transparent);
}

.relationship-row > div {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.relationship-dot {
  width: 8px;
  height: 8px;
  margin-top: 0.65em;
  border-radius: 999px;
  background: currentColor;
  opacity: 0.74;
  flex: 0 0 auto;
}

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

.relationship-state {
  margin: 0;
}

.relationship-loading {
  display: grid;
  gap: 10px;
  padding: 16px;
  border: 1px solid color-mix(in srgb, currentColor 9%, transparent);
  border-radius: 18px;
}

.relationship-loading span {
  height: 14px;
  border-radius: 999px;
  background: linear-gradient(90deg, color-mix(in srgb, currentColor 5%, transparent), color-mix(in srgb, currentColor 12%, transparent), color-mix(in srgb, currentColor 5%, transparent));
}

.relationship-empty {
  padding: 2px 0 0;
  opacity: 0.68;
}

@container (max-width: 320px) {
  .relationship-map {
    min-height: 232px;
  }

  .relationship-node {
    min-width: 58px;
    max-width: 76px;
    padding: 6px 7px;
  }

  .relationship-node.protagonist {
    min-width: 70px;
    max-width: 84px;
    padding: 8px 9px;
  }
}

@media (max-width: 720px) {
  .relationship-map {
    min-height: 232px;
  }

  .relationship-node {
    min-width: 58px;
    max-width: 76px;
    padding: 7px 8px;
  }

  .relationship-node.protagonist {
    min-width: 70px;
    max-width: 84px;
  }
}
</style>
