<template>
  <V4PanelShell
    title="质量控制"
    :subtitle="overviewText"
    :loading="panelState.loading.value"
    :error="panelState.error.value"
    :empty="panelState.empty.value"
    empty-title="暂无质量数据"
    empty-message="尚未运行质量审计 / metrics。catchup 不会自动生成审计结果；可先运行审查审计或模拟重处理。"
    @retry="panelState.reload()"
  >
    <template #toolbar>
      <button class="v4-quality-refresh" type="button" :disabled="panelState.loading.value" @click="panelState.reload()">
        刷新
      </button>
    </template>

    <div class="v4-quality-content">
      <p v-if="isQualityEmpty" class="v4-quality-state v4-quality-empty-callout">
        尚未运行质量审计 / metrics。catchup 不会自动生成审计结果；可先运行审查审计或模拟重处理。
      </p>

      <section class="v4-quality-section">
        <div class="v4-quality-section-head">
          <strong>概览</strong>
          <span>{{ openFindingCount }} 待处理发现</span>
        </div>
        <div class="v4-quality-metric-grid">
          <article v-for="metric in metrics" :key="metric.key" class="v4-quality-metric-card">
            <small>{{ metric.key }}</small>
            <strong>{{ metric.value }}</strong>
          </article>
          <p v-if="!metrics.length" class="v4-quality-state">暂无质量指标</p>
        </div>
      </section>

      <section class="v4-quality-section">
        <div class="v4-quality-section-head">
          <strong>隔离声明</strong>
          <span>{{ quarantine.length }}</span>
        </div>
        <article v-for="item in quarantine" :key="item.id" class="v4-quality-row">
          <div>
            <strong>{{ item.reasonCode }}</strong>
            <p>{{ item.reasonText || item.claim?.claimType || item.claimId }}</p>
            <details v-if="item.sourceSpans?.length" class="v4-quality-evidence-drawer">
              <summary>证据</summary>
              <blockquote v-for="span in item.sourceSpans" :key="span.id">
                第 {{ span.chapterIndex + 1 }} 章 · {{ span.textExcerpt }}
              </blockquote>
            </details>
          </div>
          <button
            type="button"
            class="v4-quality-action"
            :data-test="`quality-quarantine-accept-${item.id}`"
            @click="acceptQuarantine(item.id)"
          >
            通过审核
          </button>
        </article>
        <p v-if="!quarantine.length" class="v4-quality-state">暂无隔离声明</p>
      </section>

      <section class="v4-quality-section">
        <div class="v4-quality-section-head">
          <strong>审计发现</strong>
          <button type="button" class="v4-quality-action" data-test="quality-run-audit" @click="runAudit">
            运行审计
          </button>
        </div>
        <article v-for="finding in findings" :key="finding.id" class="v4-quality-row">
          <div>
            <strong>{{ finding.findingType }} · {{ finding.severity }}</strong>
            <p>{{ finding.reasonText || finding.reasonCode }}</p>
            <small>{{ finding.targetType }}: {{ finding.targetId }} · {{ finding.suggestedAction }}</small>
          </div>
          <button
            type="button"
            class="v4-quality-action"
            :data-test="`quality-finding-convert-${finding.id}`"
            @click="convertFinding(finding.id)"
          >
            转为修正
          </button>
        </article>
        <p v-if="!findings.length" class="v4-quality-state">暂无审计发现</p>
      </section>

      <section class="v4-quality-section">
        <div class="v4-quality-section-head">
          <strong>修正</strong>
          <span>{{ corrections.length }}</span>
        </div>
        <article v-for="correction in corrections" :key="correction.id" class="v4-quality-row">
          <div>
            <strong>{{ correction.correctionType }}</strong>
            <p>{{ correction.targetType }}: {{ correction.targetId }} · {{ correction.status }}</p>
          </div>
          <button
            type="button"
            class="v4-quality-action"
            :data-test="`quality-apply-correction-${correction.id}`"
            @click="applyCorrection(correction.id)"
          >
            应用修正
          </button>
        </article>
        <p v-if="!corrections.length" class="v4-quality-state">暂无修正</p>
      </section>

      <section class="v4-quality-section">
        <div class="v4-quality-section-head">
          <strong>重处理</strong>
          <button type="button" class="v4-quality-action" data-test="quality-create-reprocess-dry-run" @click="createDryRunReprocess">
            模拟运行
          </button>
        </div>
        <article v-for="job in reprocessJobs" :key="job.id" class="v4-quality-row">
          <div>
            <strong>{{ job.mode }} <span v-if="job.dryRun">· 模拟运行</span></strong>
            <p>{{ job.scopeType }} · {{ job.status }} · {{ job.reason || '写入前需审核' }}</p>
          </div>
          <button
            type="button"
            class="v4-quality-action"
            :data-test="`quality-cancel-reprocess-${job.id}`"
            @click="cancelReprocess(job.id)"
          >
            取消
          </button>
        </article>
        <p v-if="!reprocessJobs.length" class="v4-quality-state">暂无重处理任务</p>
      </section>

      <section class="v4-quality-section">
        <div class="v4-quality-section-head">
          <strong>Prompt 回归测试</strong>
          <button type="button" class="v4-quality-action" data-test="quality-run-regression" @click="runPromptRegression">
            运行测试集
          </button>
        </div>
        <article v-for="run in promptRegressionRuns" :key="run.id" class="v4-quality-row">
          <div>
            <strong>{{ run.fixtureSet }}</strong>
            <p>{{ run.status }} · {{ formatJson(run.summaryJson) }}</p>
          </div>
        </article>
        <p v-if="!promptRegressionRuns.length" class="v4-quality-state">暂无回归测试记录</p>
      </section>

      <section class="v4-quality-section">
        <div class="v4-quality-section-head">
          <strong>指标</strong>
          <span>{{ metrics.length }}</span>
        </div>
        <div class="v4-quality-metric-grid">
          <article v-for="metric in metrics" :key="`detail-${metric.key}`" class="v4-quality-metric-card">
            <small>{{ metric.key }}</small>
            <strong>{{ metric.value }}</strong>
          </article>
        </div>
      </section>

      <p v-if="actionMessage" class="v4-quality-state">{{ actionMessage }}</p>
    </div>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useV4PanelState } from '../../composables/useV4PanelState'
import {
  applyV4QualityCorrection,
  cancelV4QualityReprocessJob,
  createV4QualityAuditRun,
  createV4QualityPromptRegressionRun,
  createV4QualityReprocessJob,
  getV4Quality,
  runV4QualityAuditFindingAction,
  runV4QualityQuarantineAction,
} from '../../api/v4/book'
import type {
  V4QualityAuditFindingView,
  V4QualityAuditRunView,
  V4QualityCorrectionView,
  V4QualityMetricView,
  V4QualityPromptRegressionRunView,
  V4QualityQuarantineView,
  V4QualityReprocessJobView,
} from '../../types/v4'
import V4PanelShell from './v4/V4PanelShell.vue'

const props = defineProps<{
  bookUrl: string
}>()

type MetricItem = { key: string; value: string }

type QualityData = {
  qualityMetrics: Record<string, number | V4QualityMetricView>
  quarantine: V4QualityQuarantineView[]
  auditRuns: V4QualityAuditRunView[]
  findings: V4QualityAuditFindingView[]
  corrections: V4QualityCorrectionView[]
  reprocessJobs: V4QualityReprocessJobView[]
  promptRegressionRuns: V4QualityPromptRegressionRunView[]
}

const panelState = useV4PanelState<QualityData>(
  async () => {
    const data = await getV4Quality(props.bookUrl)
    return {
      qualityMetrics: data.qualityMetrics || {},
      quarantine: data.quarantine || data.quarantinedClaims || [],
      auditRuns: data.auditRuns || [],
      findings: data.findings || [],
      corrections: data.corrections || [],
      reprocessJobs: data.reprocessJobs || [],
      promptRegressionRuns: data.promptRegressionRuns || [],
    }
  },
  computed(() => props.bookUrl),
  {
    emptyCheck: (d) => (
      Object.keys(d.qualityMetrics).length === 0
      && d.quarantine.length === 0
      && d.auditRuns.length === 0
      && d.findings.length === 0
      && d.corrections.length === 0
      && d.reprocessJobs.length === 0
      && d.promptRegressionRuns.length === 0
    ),
  },
)

const actionMessage = ref('')

const qualityMetrics = computed(() => panelState.data.value?.qualityMetrics ?? {})
const quarantine = computed(() => panelState.data.value?.quarantine ?? [])
const auditRuns = computed(() => panelState.data.value?.auditRuns ?? [])
const findings = computed(() => panelState.data.value?.findings ?? [])
const corrections = computed(() => panelState.data.value?.corrections ?? [])
const reprocessJobs = computed(() => panelState.data.value?.reprocessJobs ?? [])
const promptRegressionRuns = computed(() => panelState.data.value?.promptRegressionRuns ?? [])

const metrics = computed<MetricItem[]>(() => Object.entries(qualityMetrics.value || {}).map(([key, raw]) => {
  const value = typeof raw === 'number' ? raw : raw.value
  return { key, value: value == null ? '—' : String(value) }
}))

const openFindingCount = computed(() => findings.value.filter((finding) => finding.status === 'open').length)
const overviewText = computed(() => `${quarantine.value.length} 隔离 · ${findings.value.length} 发现 · ${corrections.value.length} 修正`)
const isQualityEmpty = computed(() => (
  metrics.value.length === 0
  && quarantine.value.length === 0
  && auditRuns.value.length === 0
  && findings.value.length === 0
  && corrections.value.length === 0
  && reprocessJobs.value.length === 0
  && promptRegressionRuns.value.length === 0
))

async function acceptQuarantine(id: string) {
  await runV4QualityQuarantineAction(props.bookUrl, id, {
    action: 'accept',
    actor: 'reader-ui',
    note: 'reviewed in quality panel',
  })
  actionMessage.value = '已提交审核通过操作。'
}

async function runAudit() {
  await createV4QualityAuditRun(props.bookUrl, {
    auditType: 'full_book_quality',
    scopeJson: { source: 'quality_panel' },
  })
  actionMessage.value = '已提交审查审计。'
}

async function convertFinding(id: string) {
  await runV4QualityAuditFindingAction(props.bookUrl, id, {
    action: 'convert_to_correction',
    actor: 'reader-ui',
  })
  actionMessage.value = '发现已转为修正。'
}

async function applyCorrection(id: string) {
  await applyV4QualityCorrection(props.bookUrl, id)
  actionMessage.value = '已提交修正应用。'
}

async function createDryRunReprocess() {
  await createV4QualityReprocessJob(props.bookUrl, {
    scopeType: 'full_book',
    scopeJson: { source: 'quality_panel' },
    mode: 'dry_run_compare',
    requestedBy: 'reader-ui',
    reason: 'review before write',
    dryRun: true,
  })
  actionMessage.value = '已提交模拟重处理。'
}

async function cancelReprocess(id: string) {
  await cancelV4QualityReprocessJob(props.bookUrl, id)
  actionMessage.value = '已提交重处理任务取消。'
}

async function runPromptRegression() {
  await createV4QualityPromptRegressionRun(props.bookUrl, {
    promptVersion: 'phase6',
    schemaVersion: 'v4',
    model: 'mock',
    fixtureSet: 'phase6',
  })
  actionMessage.value = '已提交 Prompt 回归测试。'
}

function formatJson(value: unknown): string {
  if (!value) return '无摘要'
  if (typeof value === 'string') return value
  return JSON.stringify(value)
}

defineExpose({ reload: () => panelState.reload() })
</script>

<style scoped>
.v4-quality-content,
.v4-quality-section {
  display: grid;
  gap: 14px;
}

.v4-quality-section-head,
.v4-quality-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.v4-quality-row p {
  margin: 0;
}

.v4-quality-state,
.v4-quality-row small {
  margin: 4px 0 0;
  color: var(--color-text-tertiary, #64748b);
}

.v4-quality-action,
.v4-quality-refresh {
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, currentColor 3%, transparent);
  color: inherit;
  padding: 4px 10px;
  font-size: 0.76rem;
  cursor: pointer;
  font-weight: 700;
}

.v4-quality-refresh:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.v4-quality-metric-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 8px;
}

.v4-quality-metric-card,
.v4-quality-row {
  border: 1px solid color-mix(in srgb, currentColor 9%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, currentColor 2%, transparent);
  padding: 10px 12px;
}

.v4-quality-metric-card {
  display: grid;
  gap: 4px;
}

.v4-quality-evidence-drawer {
  margin-top: 8px;
}

.v4-quality-evidence-drawer blockquote {
  margin: 6px 0 0;
  padding-left: 10px;
  border-left: 2px solid color-mix(in srgb, currentColor 18%, transparent);
  color: var(--color-text-secondary, #475569);
}

@media (max-width: 760px) {
  .v4-quality-section-head,
  .v4-quality-row {
    display: grid;
  }
}
</style>
