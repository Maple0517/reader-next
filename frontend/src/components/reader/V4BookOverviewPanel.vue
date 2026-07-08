<template>
  <V4PanelShell
    title="书籍概览"
    subtitle="Projection、processing 和安全质量信号一览"
    :loading="panelState.loading.value"
    :error="panelState.error.value"
    @retry="panelState.reload()"
  >
    <template #toolbar>
      <button
        type="button"
        :disabled="panelState.loading.value"
        @click="panelState.reload()"
      >刷新</button>
    </template>

    <div v-if="overview" class="v4-overview">
      <section class="v4-live-task-card" aria-label="实时任务" data-test="overview-live-task">
        <div>
          <span>实时任务</span>
          <strong>{{ liveTaskTitle }}</strong>
          <p>{{ liveTaskDetail }}</p>
        </div>
        <div class="v4-live-task-status">{{ processingLabel }}</div>
      </section>

      <section class="v4-processing-strip" aria-label="处理进度">
        <V4MetricCard label="已处理" :value="`已处理 ${formatChapter(overview.status?.maxProcessedChapter ?? overview.memory?.maxProcessedChapter)}`" />
        <V4MetricCard label="已读边界" :value="`已读 ${formatChapter(overview.status?.maxReadChapter ?? overview.memory?.maxReadChapter)}`" />
        <V4MetricCard label="补齐任务" :value="catchupLabel" />
        <V4MetricCard label="处理状态" :value="processingLabel" :tone="overview.status?.lastError ? 'danger' : undefined" />
      </section>

      <section class="v4-domain-grid" aria-label="领域概要" data-test="overview-domain-health">
        <V4MetricCard label="角色" :value="`角色 ${countLabel(overview.memory?.characterCount)}`" />
        <V4MetricCard label="关系" :value="`关系 ${countLabel(overview.memory?.relationshipCount)}`" />
        <V4MetricCard label="知识" :value="`知识 ${countLabel(overview.memory?.knowledgeCount)}`" />
        <V4MetricCard
          :label="overview.mapError ? '地图不可用' : '地点'"
          :value="overview.mapError ? '打开地图页查看' : `地点 ${countLabel(overview.mapOverview?.placeCount)}`"
          :meta="mapAttentionLabel"
          :tone="overview.mapError ? 'danger' : undefined"
        />
        <V4MetricCard label="质量发现" :value="`质量发现 ${qualityFindingCount}`" :meta="`纠错 ${qualityCorrectionCount}`" />
      </section>

      <section class="v4-attention-panel" aria-label="需要关注" data-test="overview-attention-list">
        <h3>需要注意</h3>
        <ul>
          <li v-if="overview.status?.lastError">{{ overview.status.lastError }}</li>
          <li v-if="overview.mapOverview && overview.mapOverview.conflictCount > 0">地图冲突 {{ overview.mapOverview.conflictCount }}</li>
          <li v-if="qualityFindingCount !== '0'">质量发现 {{ qualityFindingCount }}</li>
          <li v-if="!hasAttention">暂无高优先级提醒</li>
        </ul>
      </section>
    </div>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { getV4CatchupStatus, getV4Map, getV4Memory, getV4MemoryStatus, getV4Quality } from '../../api/v4/book'
import { useV4PanelState } from '../../composables/useV4PanelState'
import V4PanelShell from './v4/V4PanelShell.vue'
import V4MetricCard from './v4/V4MetricCard.vue'

const props = defineProps<{
  bookUrl: string
}>()

const panelState = useV4PanelState(
  async () => {
    const [memoryResult, statusResult, catchupResult, mapResult, qualityResult] = await Promise.allSettled([
      getV4Memory(props.bookUrl),
      getV4MemoryStatus(props.bookUrl),
      getV4CatchupStatus(props.bookUrl),
      getV4Map(props.bookUrl),
      getV4Quality(props.bookUrl),
    ])

    if (memoryResult.status === 'rejected' && statusResult.status === 'rejected') {
      throw new Error(summarizeError(memoryResult.reason))
    }

    return {
      memory: memoryResult.status === 'fulfilled' ? memoryResult.value : null,
      status: statusResult.status === 'fulfilled' ? statusResult.value : null,
      catchup: catchupResult.status === 'fulfilled' ? catchupResult.value : null,
      mapOverview: mapResult.status === 'fulfilled' ? mapResult.value : null,
      quality: qualityResult.status === 'fulfilled' ? qualityResult.value : null,
      mapError: mapResult.status === 'rejected' ? summarizeError(mapResult.reason) : '',
    }
  },
  computed(() => props.bookUrl),
  { emptyCheck: () => false },
)

const overview = computed(() => panelState.data.value)

const catchupLabel = computed(() => {
  const catchup = overview.value?.catchup
  if (!catchup) return '不可用'
  if (catchup.status === 'running') {
    const current = formatChapter(catchup.currentChapter)
    const target = formatChapter(catchup.targetChapter)
    return `${current} / ${target}`
  }
  return catchup.status
})
const liveTaskTitle = computed(() => {
  const catchup = overview.value?.catchup
  if (catchup?.status === 'running') return '任务运行中'
  if (catchup?.status === 'failed') return '任务失败'
  if (catchup?.status === 'completed') return '任务完成'
  if (catchup?.status === 'cancel_requested') return '正在取消'
  if (catchup?.status === 'cancelled') return '任务已取消'
  return overview.value?.status?.processing ? '任务运行中' : '暂无运行任务'
})
const liveTaskDetail = computed(() => {
  const catchup = overview.value?.catchup
  if (catchup?.status === 'running') {
    return `当前 ${formatChapter(catchup.currentChapter)}，目标 ${formatChapter(catchup.targetChapter)}`
  }
  if (overview.value?.status?.lastError) return overview.value.status.lastError
  if (catchup?.status === 'completed') return `已处理到 ${formatChapter(catchup.maxProcessedChapter)}`
  return `已处理到 ${formatChapter(overview.value?.status?.maxProcessedChapter ?? overview.value?.memory?.maxProcessedChapter)}`
})
const processingLabel = computed(() => {
  const status = overview.value?.status
  if (status?.lastError) return '失败'
  return status?.processing ? '处理中' : '空闲'
})
const mapAttentionLabel = computed(() => {
  const o = overview.value
  if (!o) return ''
  if (o.mapError) return '地图 tab 可单独查看'
  if (!o.mapOverview) return ''
  return `边 ${o.mapOverview.activeEdgeCount} · 冲突 ${o.mapOverview.conflictCount}`
})
const qualityFindingCount = computed(() => String(overview.value?.quality?.findings?.length ?? 0))
const qualityCorrectionCount = computed(() => String(overview.value?.quality?.corrections?.length ?? 0))
const hasAttention = computed(() => {
  const o = overview.value
  if (!o) return false
  return Boolean(o.status?.lastError)
    || Boolean(o.mapOverview && o.mapOverview.conflictCount > 0)
    || qualityFindingCount.value !== '0'
})

function countLabel(value?: number | null) {
  return typeof value === 'number' ? String(value) : '暂无数据'
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

.v4-attention-panel {
  padding: 14px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-bg-sunken) 80%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 64%, transparent);
}

.v4-live-task-card {
  display: flex;
  justify-content: space-between;
  gap: 18px;
  align-items: center;
  min-height: 118px;
  padding: 18px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-primary) 9%, var(--color-bg-sunken));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 64%, transparent);
}

.v4-live-task-card div:first-child {
  display: grid;
  gap: 8px;
}

.v4-live-task-card span {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 800;
}

.v4-live-task-card strong {
  color: var(--color-text);
  font-size: 28px;
  letter-spacing: -0.02em;
}

.v4-live-task-card p {
  margin: 0;
  color: var(--color-text-secondary);
}

.v4-live-task-status {
  display: grid;
  min-width: 88px;
  min-height: 88px;
  place-items: center;
  border-radius: 50%;
  background: color-mix(in srgb, var(--color-bg) 82%, transparent);
  color: var(--color-primary);
  font-weight: 900;
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
  .v4-live-task-card {
    display: grid;
  }

  .v4-processing-strip,
  .v4-domain-grid {
    grid-template-columns: 1fr;
  }
}
</style>
