<template>
  <section class="v4-identity-panel panel-card" :style="bodyStyle" role="tabpanel" aria-label="V4 身份调试">
    <div class="panel-head">
      <div>
        <h2>身份线索</h2>
        <p v-if="totalLinks > 0">{{ totalLinks }} 条身份链接，{{ totalOperations }} 个合并操作</p>
        <p v-else>V4 identity debug surface</p>
      </div>
      <span class="debug-pill">Debug only</span>
    </div>

    <div v-if="loading" class="identity-state identity-loading">
      <span></span><span></span><span></span>
    </div>

    <p v-else-if="error" class="identity-state identity-empty">身份调试数据加载失败。</p>

    <div v-else class="identity-content">
      <section class="identity-section">
        <div class="section-head">
          <strong>Identity links</strong>
          <span>{{ totalLinks }}</span>
        </div>
        <p v-if="!links.length" class="identity-empty">暂无 identity links</p>
        <article v-for="link in links" :key="link.id" class="identity-link-card">
          <div class="identity-link-head">
            <span :class="['link-type', `type-${link.linkType}`]">{{ link.linkType }}</span>
            <strong>{{ confidenceLabel(link.confidence) }}</strong>
          </div>
          <div class="entity-pair">
            <code>{{ link.entityAId }}</code>
            <span>{{ link.linkType === 'redirect' ? '→' : '↔' }}</span>
            <code>{{ link.entityBId }}</code>
          </div>
          <div class="identity-meta">
            <span>status：{{ link.status }}</span>
            <span>source claim：{{ link.sourceClaimId }}</span>
            <span v-if="link.redirectTargetId">redirect target：{{ link.redirectTargetId }}</span>
          </div>
        </article>
      </section>

      <section class="identity-section">
        <div class="section-head">
          <strong>Merge operations</strong>
          <span>{{ totalOperations }}</span>
        </div>
        <p v-if="!operations.length" class="identity-empty">暂无 merge operations</p>
        <article v-for="operation in operations" :key="operation.id" class="merge-card">
          <div class="identity-link-head">
            <span class="link-type">{{ operation.status }}</span>
            <strong>{{ confidenceLabel(operation.confidence) }}</strong>
          </div>
          <div class="entity-pair">
            <code>{{ operation.victimEntityId }}</code>
            <span>→</span>
            <code>{{ operation.survivorEntityId }}</code>
          </div>
          <div class="identity-meta">
            <span>reason：{{ operation.reasonCode }}</span>
            <span>source link：{{ operation.sourceIdentityLinkId }}</span>
            <span>relationship merges：{{ operation.relationshipMergeCount }}</span>
            <span>property conflicts：{{ operation.propertyConflictCount }}</span>
          </div>
        </article>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { CSSProperties } from 'vue'
import { getV4IdentityLinks, getV4MergeOperations } from '../../api/v4/book'
import type { V4IdentityLink, V4MergeOperation } from '../../types/v4'

const props = defineProps<{
  bookUrl: string
  bodyStyle?: CSSProperties
}>()

const loading = ref(false)
const error = ref(false)
const links = ref<V4IdentityLink[]>([])
const operations = ref<V4MergeOperation[]>([])

const totalLinks = computed(() => links.value.length)
const totalOperations = computed(() => operations.value.length)

let requestId = 0

function confidenceLabel(value: number): string {
  return `${Math.round(value * 100)}%`
}

async function load() {
  if (!props.bookUrl) return
  const req = ++requestId
  loading.value = true
  error.value = false
  links.value = []
  operations.value = []
  try {
    const [linkData, operationData] = await Promise.all([
      getV4IdentityLinks(props.bookUrl),
      getV4MergeOperations(props.bookUrl),
    ])
    if (req !== requestId) return
    links.value = linkData.identityLinks || []
    operations.value = operationData.mergeOperations || []
  } catch {
    if (req !== requestId) return
    error.value = true
  } finally {
    if (req === requestId) loading.value = false
  }
}

watch(() => props.bookUrl, () => {
  void load()
}, { immediate: true })

defineExpose({ reload: load })
</script>

<style scoped>
.v4-identity-panel {
  display: grid;
  gap: 14px;
}

.panel-head,
.section-head,
.identity-link-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.panel-head h2 {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
}

.panel-head p {
  margin: 4px 0 0;
  opacity: 0.55;
  font-size: 0.84rem;
}

.debug-pill,
.link-type {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 9px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 10%, transparent);
  color: var(--color-primary, #c97f3a);
  font-size: 0.72rem;
  font-weight: 700;
}

.identity-content,
.identity-section {
  display: grid;
  gap: 10px;
}

.section-head {
  color: var(--color-text-secondary);
  font-size: 0.84rem;
}

.identity-link-card,
.merge-card {
  display: grid;
  gap: 8px;
  padding: 10px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, currentColor 2%, transparent);
}

.entity-pair {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.entity-pair code {
  padding: 2px 6px;
  border-radius: 6px;
  background: var(--color-bg-sunken);
  color: var(--color-text);
  font-size: 0.78rem;
}

.entity-pair span {
  color: var(--color-text-tertiary);
}

.identity-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  color: var(--color-text-tertiary);
  font-size: 0.76rem;
}

.identity-state,
.identity-empty {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: 0.84rem;
}

.identity-loading {
  display: inline-flex;
  gap: 5px;
  align-items: center;
}

.identity-loading span {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: pulse 0.9s ease-in-out infinite;
}

.identity-loading span:nth-child(2) {
  animation-delay: 0.12s;
}

.identity-loading span:nth-child(3) {
  animation-delay: 0.24s;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.25;
  }

  50% {
    opacity: 1;
  }
}
</style>
