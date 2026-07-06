<template>
  <V4PanelFrame title="V4 总览" subtitle="只展示 V4 projection、processing 和安全质量信号。">
    <V4LoadingState v-if="loading" title="正在加载 V4 总览" />
    <V4ErrorState
      v-else-if="pageError"
      title="总览加载失败"
      :message="pageError"
      retry-label="重试"
      :on-retry="load"
    />

    <div v-else class="v4-overview">
      <section class="v4-processing-strip" aria-label="V4 processing progress">
        <article>
          <span>已处理</span>
          <strong>已处理 {{ formatChapter(status?.maxProcessedChapter ?? memory?.maxProcessedChapter) }}</strong>
        </article>
        <article>
          <span>已读边界</span>
          <strong>已读 {{ formatChapter(status?.maxReadChapter ?? memory?.maxReadChapter) }}</strong>
        </article>
        <article>
          <span>补齐任务</span>
          <strong>{{ catchupLabel }}</strong>
        </article>
        <article :class="{ 'is-error': Boolean(status?.lastError) }">
          <span>处理状态</span>
          <strong>{{ processingLabel }}</strong>
        </article>
      </section>

      <section class="v4-domain-grid" aria-label="V4 domain summary">
        <article class="v4-domain-card">
          <span>角色</span>
          <strong>角色 {{ countLabel(memory?.characterCount) }}</strong>
          <small>来自 V4 aggregate</small>
        </article>
        <article class="v4-domain-card">
          <span>关系</span>
          <strong>关系 {{ countLabel(memory?.relationshipCount) }}</strong>
          <small>来自 V4 aggregate</small>
        </article>
        <article class="v4-domain-card">
          <span>知识</span>
          <strong>知识 {{ countLabel(memory?.knowledgeCount) }}</strong>
          <small>来自 V4 aggregate</small>
        </article>
        <article class="v4-domain-card" :class="{ unavailable: Boolean(mapError) }">
          <span>{{ mapError ? '地图不可用' : '地点' }}</span>
          <strong>{{ mapError ? 'open tab to view' : `地点 ${countLabel(mapOverview?.placeCount)}` }}</strong>
          <small>{{ mapAttentionLabel }}</small>
        </article>
        <article class="v4-domain-card">
          <span>质量发现</span>
          <strong>质量发现 {{ qualityFindingCount }}</strong>
          <small>纠错 {{ qualityCorrectionCount }}</small>
        </article>
      </section>

      <section class="v4-attention-panel" aria-label="V4 attention">
        <h3>需要注意</h3>
        <ul>
          <li v-if="status?.lastError">{{ status.lastError }}</li>
          <li v-if="mapOverview && mapOverview.conflictCount > 0">地图冲突 {{ mapOverview.conflictCount }}</li>
          <li v-if="qualityFindingCount !== '0'">质量发现 {{ qualityFindingCount }}</li>
          <li v-if="!hasAttention">暂无高优先级提醒</li>
        </ul>
      </section>
    </div>
  </V4PanelFrame>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getV4CatchupStatus, getV4Map, getV4Memory, getV4MemoryStatus, getV4Quality } from '../../api/v4/book'
import type {
  V4CatchupStatusResponse,
  V4MapOverviewView,
  V4MemoryResponse,
  V4MemoryStatusResponse,
  V4QualityOverviewView,
} from '../../types/v4'
import V4ErrorState from './v4/V4ErrorState.vue'
import V4LoadingState from './v4/V4LoadingState.vue'
import V4PanelFrame from './v4/V4PanelFrame.vue'

const props = defineProps<{
  bookUrl: string
}>()

const loading = ref(false)
const pageError = ref('')
const memory = ref<V4MemoryResponse | null>(null)
const status = ref<V4MemoryStatusResponse | null>(null)
const catchup = ref<V4CatchupStatusResponse | null>(null)
const mapOverview = ref<V4MapOverviewView | null>(null)
const quality = ref<V4QualityOverviewView | null>(null)
const mapError = ref('')
let requestId = 0

const catchupLabel = computed(() => {
  if (!catchup.value) return '不可用'
  if (catchup.value.status === 'running') {
    const current = formatChapter(catchup.value.currentChapter)
    const target = formatChapter(catchup.value.targetChapter)
    return `${current} / ${target}`
  }
  return catchup.value.status
})
const processingLabel = computed(() => {
  if (status.value?.lastError) return '失败'
  return status.value?.processing ? '处理中' : '空闲'
})
const mapAttentionLabel = computed(() => {
  if (mapError.value) return '地图 tab 可单独查看'
  if (!mapOverview.value) return 'unavailable'
  return `边 ${mapOverview.value.activeEdgeCount} · 冲突 ${mapOverview.value.conflictCount}`
})
const qualityFindingCount = computed(() => String(quality.value?.findings?.length ?? 0))
const qualityCorrectionCount = computed(() => String(quality.value?.corrections?.length ?? 0))
const hasAttention = computed(() => {
  return Boolean(status.value?.lastError)
    || Boolean(mapOverview.value && mapOverview.value.conflictCount > 0)
    || qualityFindingCount.value !== '0'
})

watch(
  () => props.bookUrl,
  () => {
    void load()
  },
  { immediate: true },
)

async function load() {
  requestId += 1
  const currentRequestId = requestId
  loading.value = true
  pageError.value = ''
  mapError.value = ''

  const [memoryResult, statusResult, catchupResult, mapResult, qualityResult] = await Promise.allSettled([
    getV4Memory(props.bookUrl),
    getV4MemoryStatus(props.bookUrl),
    getV4CatchupStatus(props.bookUrl),
    getV4Map(props.bookUrl),
    getV4Quality(props.bookUrl),
  ])

  if (currentRequestId !== requestId) return

  memory.value = memoryResult.status === 'fulfilled' ? memoryResult.value : null
  status.value = statusResult.status === 'fulfilled' ? statusResult.value : null
  catchup.value = catchupResult.status === 'fulfilled' ? catchupResult.value : null
  mapOverview.value = mapResult.status === 'fulfilled' ? mapResult.value : null
  quality.value = qualityResult.status === 'fulfilled' ? qualityResult.value : null
  mapError.value = mapResult.status === 'rejected' ? summarizeError(mapResult.reason) : ''

  if (memoryResult.status === 'rejected' && statusResult.status === 'rejected') {
    pageError.value = summarizeError(memoryResult.reason)
  }
  loading.value = false
}

function countLabel(value?: number | null) {
  return typeof value === 'number' ? String(value) : 'unavailable'
}

function formatChapter(index?: number | null) {
  return typeof index === 'number' ? `第 ${index + 1} 章` : '不可用'
}

function summarizeError(error: unknown) {
  return error instanceof Error && error.message ? error.message : '请求失败'
}
</script>

<style scoped>
.v4-overview {
  display: grid;
  gap: 16px;
}

.v4-processing-strip,
.v4-domain-grid {
  display: grid;
  gap: 10px;
}

.v4-processing-strip {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.v4-domain-grid {
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.v4-processing-strip article,
.v4-domain-card,
.v4-attention-panel {
  padding: 14px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-bg-sunken) 80%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 64%, transparent);
}

.v4-processing-strip article,
.v4-domain-card {
  display: grid;
  gap: 6px;
}

.v4-processing-strip span,
.v4-domain-card span,
.v4-domain-card small {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 800;
}

.v4-processing-strip strong,
.v4-domain-card strong {
  color: var(--color-text);
  font-size: 18px;
  letter-spacing: -0.02em;
}

.v4-processing-strip article.is-error strong,
.v4-domain-card.unavailable strong {
  color: var(--color-danger);
}

.v4-attention-panel {
  display: grid;
  gap: 10px;
}

.v4-attention-panel h3 {
  margin: 0;
  font-size: 16px;
}

.v4-attention-panel ul {
  display: grid;
  gap: 6px;
  margin: 0;
  padding-left: 18px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

@media (max-width: 900px) {
  .v4-processing-strip,
  .v4-domain-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 640px) {
  .v4-processing-strip,
  .v4-domain-grid {
    grid-template-columns: 1fr;
  }
}
</style>
