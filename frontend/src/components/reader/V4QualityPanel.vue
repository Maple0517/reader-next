<template>
  <section class="v4-quality-panel panel-card" :style="bodyStyle" role="tabpanel" aria-label="V4 质量控制">
    <div class="panel-head">
      <div>
        <h2>质量控制</h2>
        <p>{{ overviewText }}</p>
      </div>
      <span class="quality-pill">Review-first</span>
    </div>

    <div v-if="loading" class="quality-state quality-loading">
      <span></span><span></span><span></span>
    </div>
    <p v-else-if="error" class="quality-state">质量数据加载失败。</p>

    <div v-else class="quality-content">
      <p v-if="isQualityEmpty" class="quality-state quality-empty-callout">
        尚未运行质量审计 / metrics。catchup 不会自动生成审计结果；可先运行 review audit 或 dry-run reprocess。
      </p>

      <section class="quality-section">
        <div class="section-head">
          <strong>Overview</strong>
          <span>{{ openFindingCount }} open findings</span>
        </div>
        <div class="metric-grid">
          <article v-for="metric in metrics" :key="metric.key" class="metric-card">
            <small>{{ metric.key }}</small>
            <strong>{{ metric.value }}</strong>
          </article>
          <p v-if="!metrics.length" class="quality-state">暂无质量指标</p>
        </div>
      </section>

      <section class="quality-section">
        <div class="section-head">
          <strong>Quarantine</strong>
          <span>{{ quarantine.length }}</span>
        </div>
        <article v-for="item in quarantine" :key="item.id" class="quality-row">
          <div>
            <strong>{{ item.reasonCode }}</strong>
            <p>{{ item.reasonText || item.claim?.claimType || item.claimId }}</p>
            <details v-if="item.sourceSpans?.length" class="evidence-drawer">
              <summary>Evidence</summary>
              <blockquote v-for="span in item.sourceSpans" :key="span.id">
                第 {{ span.chapterIndex + 1 }} 章 · {{ span.textExcerpt }}
              </blockquote>
            </details>
          </div>
          <button
            type="button"
            class="quality-action"
            :data-test="`quality-quarantine-accept-${item.id}`"
            @click="acceptQuarantine(item.id)"
          >
            Accept reviewed
          </button>
        </article>
        <p v-if="!quarantine.length" class="quality-state">暂无隔离 claim</p>
      </section>

      <section class="quality-section">
        <div class="section-head">
          <strong>Audit Findings</strong>
          <button type="button" class="quality-action" data-test="quality-run-audit" @click="runAudit">
            Run review audit
          </button>
        </div>
        <article v-for="finding in findings" :key="finding.id" class="quality-row">
          <div>
            <strong>{{ finding.findingType }} · {{ finding.severity }}</strong>
            <p>{{ finding.reasonText || finding.reasonCode }}</p>
            <small>{{ finding.targetType }}: {{ finding.targetId }} · {{ finding.suggestedAction }}</small>
          </div>
          <button
            type="button"
            class="quality-action"
            :data-test="`quality-finding-convert-${finding.id}`"
            @click="convertFinding(finding.id)"
          >
            Convert to correction
          </button>
        </article>
        <p v-if="!findings.length" class="quality-state">暂无 audit finding</p>
      </section>

      <section class="quality-section">
        <div class="section-head">
          <strong>Corrections</strong>
          <span>{{ corrections.length }}</span>
        </div>
        <article v-for="correction in corrections" :key="correction.id" class="quality-row">
          <div>
            <strong>{{ correction.correctionType }}</strong>
            <p>{{ correction.targetType }}: {{ correction.targetId }} · {{ correction.status }}</p>
          </div>
          <button
            type="button"
            class="quality-action"
            :data-test="`quality-apply-correction-${correction.id}`"
            @click="applyCorrection(correction.id)"
          >
            Apply reviewed correction
          </button>
        </article>
        <p v-if="!corrections.length" class="quality-state">暂无 correction</p>
      </section>

      <section class="quality-section">
        <div class="section-head">
          <strong>Reprocess</strong>
          <button type="button" class="quality-action" data-test="quality-create-reprocess-dry-run" @click="createDryRunReprocess">
            Queue dry-run
          </button>
        </div>
        <article v-for="job in reprocessJobs" :key="job.id" class="quality-row">
          <div>
            <strong>{{ job.mode }} <span v-if="job.dryRun">· dry-run</span></strong>
            <p>{{ job.scopeType }} · {{ job.status }} · {{ job.reason || 'review before write' }}</p>
          </div>
          <button
            type="button"
            class="quality-action"
            :data-test="`quality-cancel-reprocess-${job.id}`"
            @click="cancelReprocess(job.id)"
          >
            Cancel
          </button>
        </article>
        <p v-if="!reprocessJobs.length" class="quality-state">暂无 reprocess job</p>
      </section>

      <section class="quality-section">
        <div class="section-head">
          <strong>Prompt Regression</strong>
          <button type="button" class="quality-action" data-test="quality-run-regression" @click="runPromptRegression">
            Run fixture set
          </button>
        </div>
        <article v-for="run in promptRegressionRuns" :key="run.id" class="quality-row">
          <div>
            <strong>{{ run.fixtureSet }}</strong>
            <p>{{ run.status }} · {{ formatJson(run.summaryJson) }}</p>
          </div>
        </article>
        <p v-if="!promptRegressionRuns.length" class="quality-state">暂无 prompt regression run</p>
      </section>

      <section class="quality-section">
        <div class="section-head">
          <strong>Metrics</strong>
          <span>{{ metrics.length }}</span>
        </div>
        <div class="metric-grid">
          <article v-for="metric in metrics" :key="`detail-${metric.key}`" class="metric-card">
            <small>{{ metric.key }}</small>
            <strong>{{ metric.value }}</strong>
          </article>
        </div>
      </section>

      <p v-if="actionMessage" class="quality-state">{{ actionMessage }}</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { CSSProperties } from 'vue'
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

const props = defineProps<{
  bookUrl: string
  bodyStyle?: CSSProperties
}>()

type MetricItem = { key: string; value: string }

const loading = ref(false)
const error = ref(false)
const actionMessage = ref('')
const qualityMetrics = ref<Record<string, number | V4QualityMetricView>>({})
const quarantine = ref<V4QualityQuarantineView[]>([])
const auditRuns = ref<V4QualityAuditRunView[]>([])
const findings = ref<V4QualityAuditFindingView[]>([])
const corrections = ref<V4QualityCorrectionView[]>([])
const reprocessJobs = ref<V4QualityReprocessJobView[]>([])
const promptRegressionRuns = ref<V4QualityPromptRegressionRunView[]>([])
let requestId = 0

const metrics = computed<MetricItem[]>(() => Object.entries(qualityMetrics.value || {}).map(([key, raw]) => {
  const value = typeof raw === 'number' ? raw : raw.value
  return { key, value: value == null ? '—' : String(value) }
}))

const openFindingCount = computed(() => findings.value.filter((finding) => finding.status === 'open').length)
const overviewText = computed(() => `${quarantine.value.length} quarantined · ${findings.value.length} findings · ${corrections.value.length} corrections`)
const isQualityEmpty = computed(() => (
  metrics.value.length === 0
  && quarantine.value.length === 0
  && auditRuns.value.length === 0
  && findings.value.length === 0
  && corrections.value.length === 0
  && reprocessJobs.value.length === 0
  && promptRegressionRuns.value.length === 0
))

async function load() {
  if (!props.bookUrl) return
  const req = ++requestId
  loading.value = true
  error.value = false
  try {
    const data = await getV4Quality(props.bookUrl)
    if (req !== requestId) return
    qualityMetrics.value = data.qualityMetrics || {}
    quarantine.value = data.quarantine || data.quarantinedClaims || []
    auditRuns.value = data.auditRuns || []
    findings.value = data.findings || []
    corrections.value = data.corrections || []
    reprocessJobs.value = data.reprocessJobs || []
    promptRegressionRuns.value = data.promptRegressionRuns || []
  } catch {
    if (req !== requestId) return
    error.value = true
  } finally {
    if (req === requestId) loading.value = false
  }
}

async function acceptQuarantine(id: string) {
  await runV4QualityQuarantineAction(props.bookUrl, id, {
    action: 'accept',
    actor: 'reader-ui',
    note: 'reviewed in quality panel',
  })
  actionMessage.value = 'Quarantine action submitted for reviewed claim.'
}

async function runAudit() {
  await createV4QualityAuditRun(props.bookUrl, {
    auditType: 'full_book_quality',
    scopeJson: { source: 'quality_panel' },
  })
  actionMessage.value = 'Review audit submitted.'
}

async function convertFinding(id: string) {
  await runV4QualityAuditFindingAction(props.bookUrl, id, {
    action: 'convert_to_correction',
    actor: 'reader-ui',
  })
  actionMessage.value = 'Finding converted to correction.'
}

async function applyCorrection(id: string) {
  await applyV4QualityCorrection(props.bookUrl, id)
  actionMessage.value = 'Reviewed correction apply submitted.'
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
  actionMessage.value = 'Dry-run reprocess queued.'
}

async function cancelReprocess(id: string) {
  await cancelV4QualityReprocessJob(props.bookUrl, id)
  actionMessage.value = 'Reprocess job cancellation submitted.'
}

async function runPromptRegression() {
  await createV4QualityPromptRegressionRun(props.bookUrl, {
    promptVersion: 'phase6',
    schemaVersion: 'v4',
    model: 'mock',
    fixtureSet: 'phase6',
  })
  actionMessage.value = 'Prompt regression fixture run submitted.'
}

function formatJson(value: unknown): string {
  if (!value) return 'no summary'
  if (typeof value === 'string') return value
  return JSON.stringify(value)
}

watch(() => props.bookUrl, () => {
  void load()
}, { immediate: true })

defineExpose({ reload: load })
</script>

<style scoped>
.v4-quality-panel,
.quality-content,
.quality-section {
  display: grid;
  gap: 14px;
}

.panel-head,
.section-head,
.quality-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.panel-head h2,
.quality-row p {
  margin: 0;
}

.panel-head p,
.quality-state,
.quality-row small {
  margin: 4px 0 0;
  color: var(--color-text-tertiary, #64748b);
}

.quality-pill,
.quality-action {
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, currentColor 3%, transparent);
  color: inherit;
  padding: 4px 10px;
  font-size: 0.76rem;
}

.quality-action {
  cursor: pointer;
  font-weight: 700;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 8px;
}

.metric-card,
.quality-row {
  border: 1px solid color-mix(in srgb, currentColor 9%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, currentColor 2%, transparent);
  padding: 10px 12px;
}

.metric-card {
  display: grid;
  gap: 4px;
}

.evidence-drawer {
  margin-top: 8px;
}

.evidence-drawer blockquote {
  margin: 6px 0 0;
  padding-left: 10px;
  border-left: 2px solid color-mix(in srgb, currentColor 18%, transparent);
  color: var(--color-text-secondary, #475569);
}

.quality-loading {
  display: inline-flex;
  gap: 6px;
}

.quality-loading span {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.45;
}

@media (max-width: 760px) {
  .panel-head,
  .section-head,
  .quality-row {
    display: grid;
  }
}
</style>
