<template>
  <V4PanelShell
    title="任务进度"
    subtitle="从现有 V4 状态记录推导补齐/生成任务进度"
    :loading="loading"
    :error="error"
    @retry="reload"
  >
    <section class="v4-task-progress" data-test="v4-task-panel">
      <div :class="['v4-task-progress-hero', `tone-${taskTone}`]">
        <div class="v4-task-progress-copy">
          <span class="v4-task-progress-kicker">当前任务</span>
          <h3>{{ taskTitle }}</h3>
          <p>{{ taskSummary }}</p>
        </div>
        <div class="v4-task-progress-ring" aria-label="任务处理比例" :style="{ '--progress': progressPercent }">
          <span>{{ progressPercent }}%</span>
        </div>
      </div>

      <div class="v4-task-metrics" aria-label="任务边界">
        <V4MetricCard label="当前章节" :value="formatChapterNumber(currentChapter)" />
        <V4MetricCard label="目标章节" :value="formatChapterNumber(targetChapter)" />
        <V4MetricCard label="已处理" :value="formatChapterNumber(maxProcessedChapter)" />
      </div>

      <section class="v4-task-stage-panel" aria-label="任务阶段">
        <article
          v-for="stage in stages"
          :key="stage.key"
          :data-test="`task-stage-${stage.key}`"
          :class="['v4-task-stage', { active: stage.active }]"
        >
          <span class="v4-task-stage-dot" />
          <div>
            <h4>{{ stage.label }}</h4>
            <p>{{ stage.description }}</p>
          </div>
        </article>
      </section>

      <section class="v4-task-navigator" aria-label="章节处理边界">
        <div class="v4-task-navigator-track">
          <span class="v4-task-navigator-fill" :style="{ width: `${progressPercent}%` }" />
          <span v-if="currentMarkerPercent !== null" class="v4-task-navigator-current" :style="{ left: `${currentMarkerPercent}%` }" />
        </div>
        <div class="v4-task-navigator-labels">
          <span>已处理 {{ formatChapterNumber(maxProcessedChapter) }}</span>
          <span>目标 {{ formatChapterNumber(targetChapter) }}</span>
        </div>
      </section>

      <V4InspectorSection v-if="lastError" title="失败诊断">
        <p class="v4-task-error">{{ lastError }}</p>
      </V4InspectorSection>
    </section>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { getV4CatchupStatus, getV4MemoryStatus } from '../../api/v4/book'
import { useV4PanelState } from '../../composables/useV4PanelState'
import type { V4CatchupStatus, V4CatchupStatusResponse, V4MemoryStatusResponse } from '../../types/v4'
import V4PanelShell from './v4/V4PanelShell.vue'
import V4InspectorSection from './v4/V4InspectorSection.vue'
import V4MetricCard from './v4/V4MetricCard.vue'

const props = defineProps<{
  bookUrl: string
}>()

interface TaskProgressData {
  memory: V4MemoryStatusResponse
  catchup: V4CatchupStatusResponse
}

const bookUrlRef = computed(() => props.bookUrl)

const { loading, error, data, reload } = useV4PanelState<TaskProgressData>(
  async () => {
    const [memory, catchup] = await Promise.all([
      getV4MemoryStatus(props.bookUrl),
      getV4CatchupStatus(props.bookUrl),
    ])
    return { memory, catchup }
  },
  bookUrlRef,
)

const memoryStatus = computed(() => data.value?.memory ?? null)
const catchupStatus = computed(() => data.value?.catchup ?? null)
const status = computed<V4CatchupStatus>(() => catchupStatus.value?.status ?? 'idle')
const currentChapter = computed(() => catchupStatus.value?.currentChapter ?? null)
const targetChapter = computed(() => catchupStatus.value?.targetChapter ?? memoryStatus.value?.maxReadChapter ?? null)
const maxProcessedChapter = computed(() => catchupStatus.value?.maxProcessedChapter ?? memoryStatus.value?.maxProcessedChapter ?? null)
const lastError = computed(() => catchupStatus.value?.lastError || memoryStatus.value?.lastError || '')

const taskTitle = computed(() => {
  if (status.value === 'running') return '运行中'
  if (status.value === 'failed') return '失败'
  if (status.value === 'cancel_requested') return '取消中'
  if (status.value === 'cancelled') return '已取消'
  if (status.value === 'completed') return '完成'
  if (memoryStatus.value?.processing) return '运行中'
  return '等待任务'
})

const taskTone = computed(() => {
  if (status.value === 'failed') return 'danger'
  if (status.value === 'running' || memoryStatus.value?.processing) return 'active'
  if (status.value === 'completed') return 'success'
  if (status.value === 'cancel_requested' || status.value === 'cancelled') return 'warning'
  return 'neutral'
})

const taskSummary = computed(() => {
  if (lastError.value) return lastError.value
  if (status.value === 'running') return `${formatChapterNumber(currentChapter.value)} 正在推进到 ${formatChapterNumber(targetChapter.value)}`
  if (status.value === 'completed') return `已处理到 ${formatChapterNumber(maxProcessedChapter.value)}`
  if (status.value === 'cancel_requested') return '任务正在响应取消请求'
  if (status.value === 'cancelled') return '任务已取消，可重新发起补齐'
  return '当前没有正在运行的补齐任务'
})

const progressPercent = computed(() => {
  const target = targetChapter.value
  const processed = maxProcessedChapter.value
  if (typeof target !== 'number' || target < 0 || typeof processed !== 'number' || processed < 0) return 0
  return Math.max(0, Math.min(100, Math.round(((processed + 1) / (target + 1)) * 100)))
})

const currentMarkerPercent = computed(() => {
  const target = targetChapter.value
  const current = currentChapter.value
  if (typeof target !== 'number' || target < 0 || typeof current !== 'number' || current < 0) return null
  return Math.max(0, Math.min(100, Math.round(((current + 1) / (target + 1)) * 100)))
})

const stages = computed(() => [
  {
    key: 'status',
    label: '状态读取',
    description: memoryStatus.value ? '已读取 V4 memory/status 与 catchup/status' : '等待状态接口返回',
    active: status.value === 'idle' && !memoryStatus.value?.processing,
  },
  {
    key: 'running',
    label: '章节处理中',
    description: `当前 ${formatChapterNumber(currentChapter.value)}，目标 ${formatChapterNumber(targetChapter.value)}`,
    active: status.value === 'running' || Boolean(memoryStatus.value?.processing),
  },
  {
    key: 'failed',
    label: '失败诊断',
    description: lastError.value || '暂无失败信息',
    active: status.value === 'failed' || Boolean(lastError.value),
  },
  {
    key: 'done',
    label: '完成/等待',
    description: status.value === 'completed' ? '补齐已完成' : '没有正在运行的任务',
    active: status.value === 'completed' || status.value === 'cancelled',
  },
])

function formatChapterNumber(index?: number | null) {
  return typeof index === 'number' ? `第 ${index + 1} 章` : '不可用'
}
</script>

<style scoped>
.v4-task-progress {
  display: grid;
  gap: 16px;
}

.v4-task-progress-hero {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 104px;
  gap: 18px;
  align-items: center;
  min-height: 150px;
  padding: 20px;
  border-radius: 16px;
  background: color-mix(in srgb, var(--color-bg-sunken) 76%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 72%, transparent);
}

.v4-task-progress-hero.tone-active {
  background: color-mix(in srgb, var(--color-primary) 10%, var(--color-bg-sunken));
}

.v4-task-progress-hero.tone-danger {
  background: color-mix(in srgb, var(--color-danger) 10%, var(--color-bg-sunken));
}

.v4-task-progress-hero.tone-success {
  background: color-mix(in srgb, var(--color-success) 10%, var(--color-bg-sunken));
}

.v4-task-progress-hero.tone-warning {
  background: color-mix(in srgb, var(--color-warning) 10%, var(--color-bg-sunken));
}

.v4-task-progress-copy {
  display: grid;
  gap: 8px;
}

.v4-task-progress-kicker {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 850;
}

.v4-task-progress-copy h3,
.v4-task-progress-copy p {
  margin: 0;
}

.v4-task-progress-copy h3 {
  color: var(--color-text);
  font-size: 32px;
  line-height: 1.05;
}

.v4-task-progress-copy p {
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.v4-task-progress-ring {
  display: grid;
  width: 92px;
  aspect-ratio: 1;
  place-items: center;
  border-radius: 50%;
  background:
    radial-gradient(circle at center, var(--color-bg) 56%, transparent 58%),
    conic-gradient(var(--color-primary) calc(var(--progress, 0) * 1%), color-mix(in srgb, var(--color-border) 72%, transparent) 0);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 72%, transparent);
}

.v4-task-progress-ring span {
  color: var(--color-text);
  font-weight: 900;
}

.v4-task-metrics {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.v4-task-stage-panel {
  display: grid;
  gap: 8px;
}

.v4-task-stage {
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr);
  gap: 10px;
  align-items: start;
  padding: 12px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--color-bg-sunken) 60%, transparent);
}

.v4-task-stage.active {
  background: color-mix(in srgb, var(--color-primary) 10%, var(--color-bg-sunken));
}

.v4-task-stage-dot {
  width: 10px;
  aspect-ratio: 1;
  margin-top: 5px;
  border-radius: 50%;
  background: var(--color-border);
}

.v4-task-stage.active .v4-task-stage-dot {
  background: var(--color-primary);
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--color-primary) 14%, transparent);
}

.v4-task-stage h4,
.v4-task-stage p,
.v4-task-error {
  margin: 0;
}

.v4-task-stage h4 {
  color: var(--color-text);
  font-size: 14px;
}

.v4-task-stage p,
.v4-task-error {
  margin-top: 4px;
  color: var(--color-text-secondary);
  line-height: 1.55;
}

.v4-task-navigator {
  display: grid;
  gap: 8px;
  padding: 14px;
  border-radius: 14px;
  background: color-mix(in srgb, var(--color-bg-sunken) 68%, transparent);
}

.v4-task-navigator-track {
  position: relative;
  height: 10px;
  overflow: hidden;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-border) 72%, transparent);
}

.v4-task-navigator-fill {
  position: absolute;
  inset: 0 auto 0 0;
  border-radius: inherit;
  background: var(--color-primary);
}

.v4-task-navigator-current {
  position: absolute;
  top: 50%;
  width: 14px;
  aspect-ratio: 1;
  border: 2px solid var(--color-bg);
  border-radius: 50%;
  background: var(--color-warning);
  transform: translate(-50%, -50%);
}

.v4-task-navigator-labels {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 750;
}

@media (max-width: 680px) {
  .v4-task-progress-hero,
  .v4-task-metrics {
    grid-template-columns: 1fr;
  }
}
</style>
