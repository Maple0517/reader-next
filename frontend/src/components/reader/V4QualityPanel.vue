<template>
  <V4PanelShell
    title="质量治理"
    :subtitle="overviewText"
    :loading="panelState.loading.value"
    :error="panelState.error.value"
    :empty="false"
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
      <section class="v4-quality-governance-toolbar" data-test="quality-governance-toolbar" aria-label="质量治理动作">
        <p>默认按“最需要人介入”排序。这里只展示问题和处理动作，不把 canonical 修正悄悄塞回 projection。</p>
        <div class="v4-quality-action-row">
          <button type="button" class="v4-quality-action" data-test="quality-run-audit" @click="runAudit">
            运行审计
          </button>
          <button type="button" class="v4-quality-action" data-test="quality-create-reprocess-dry-run" @click="createDryRunReprocess">
            模拟运行
          </button>
          <button type="button" class="v4-quality-action" data-test="quality-run-regression" @click="runPromptRegression">
            运行测试集
          </button>
        </div>
      </section>

      <p v-if="isQualityEmpty" class="v4-quality-state v4-quality-empty-callout">
        尚未运行质量审计 / metrics。catchup 不会自动生成审计结果；可先运行审查审计或模拟重处理。
      </p>

      <section class="v4-quality-overview v4-quality-section">
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

      <section class="v4-quality-workbench">
        <aside class="v4-quality-queue-panel" data-test="quality-queue-nav" aria-label="质量队列">
          <div class="v4-quality-section-head">
            <strong>Queues</strong>
            <span>{{ triageItems.length }} open</span>
          </div>
          <button
            v-for="queue in queueItems"
            :key="queue.key"
            type="button"
            class="v4-quality-queue"
            :class="{ 'is-active': activeQueue === queue.key }"
            @click="activeQueue = queue.key"
          >
            <span>{{ queue.label }}</span>
            <strong>{{ queue.count }}</strong>
            <small>{{ queue.meta }}</small>
          </button>
        </aside>

        <section class="v4-quality-triage-list" data-test="quality-triage-list" aria-label="质量分诊列表">
          <div class="v4-quality-section-head">
            <strong>{{ activeQueueLabel }}</strong>
            <span>默认按最需要人介入排序</span>
          </div>
          <article
            v-for="item in visibleTriageItems"
            :key="item.key"
            class="v4-quality-row v4-quality-triage-row"
            :class="{ 'is-active': selectedKey === item.key }"
            @click="selectedKey = item.key"
          >
            <div class="v4-quality-row-main">
              <div class="v4-quality-row-title">
                <strong>{{ item.title }}</strong>
                <span :class="['v4-quality-severity', item.severityTone]">{{ item.severityLabel }}</span>
              </div>
              <p>{{ item.summary }}</p>
              <small>{{ item.meta }}</small>
            </div>
            <div class="v4-quality-row-actions" @click.stop>
              <button
                v-if="item.kind === 'quarantine'"
                type="button"
                class="v4-quality-action"
                :data-test="`quality-quarantine-accept-${item.raw.id}`"
                @click="acceptQuarantine(item.raw.id)"
              >
                通过审核
              </button>
              <button
                v-else-if="item.kind === 'finding'"
                type="button"
                class="v4-quality-action"
                :data-test="`quality-finding-convert-${item.raw.id}`"
                @click="convertFinding(item.raw.id)"
              >
                转为修正
              </button>
              <button
                v-else-if="item.kind === 'correction'"
                type="button"
                class="v4-quality-action"
                :data-test="`quality-apply-correction-${item.raw.id}`"
                @click="applyCorrection(item.raw.id)"
              >
                应用修正
              </button>
              <button
                v-else-if="item.kind === 'reprocess'"
                type="button"
                class="v4-quality-action"
                :data-test="`quality-cancel-reprocess-${item.raw.id}`"
                @click="cancelReprocess(item.raw.id)"
              >
                取消
              </button>
            </div>
          </article>
          <article v-if="!visibleTriageItems.length" class="v4-quality-empty-triage-row" data-test="quality-empty-triage-row">
            <span class="v4-quality-severity is-low">-</span>
            <div>
              <strong>当前队列暂无待处理项</strong>
              <p>运行审计或模拟重处理后，隔离、发现、失败任务会进入这里。</p>
            </div>
          </article>
        </section>

        <aside class="v4-quality-inspector" data-test="quality-review-inspector">
          <div class="v4-quality-section-head">
            <strong>审核检查器</strong>
            <span>{{ selectedItem?.status || 'idle' }}</span>
          </div>
          <template v-if="selectedItem">
            <div class="v4-quality-inspector-hero">
              <small>{{ selectedItem.kindLabel }}</small>
              <strong>{{ selectedItem.title }}</strong>
              <p>{{ selectedItem.summary }}</p>
            </div>
            <dl class="v4-quality-detail-grid">
              <div>
                <dt>问题</dt>
                <dd>{{ selectedItem.summary }}</dd>
              </div>
              <div>
                <dt>目标</dt>
                <dd>{{ selectedItem.target }}</dd>
              </div>
              <div>
                <dt>Decision</dt>
                <dd>{{ selectedItem.decision }}</dd>
              </div>
              <div>
                <dt>建议动作</dt>
                <dd>{{ selectedItem.suggestedAction }}</dd>
              </div>
              <div>
                <dt>canonical write</dt>
                <dd>{{ selectedItem.canonicalWrite }}</dd>
              </div>
            </dl>
            <section v-if="selectedItem.claimSummary" class="v4-quality-inspector-block">
              <strong>claim summary</strong>
              <p>{{ selectedItem.claimSummary }}</p>
            </section>
            <section v-if="selectedItem.sourceSpans.length" class="v4-quality-inspector-block">
              <strong>证据</strong>
              <blockquote v-for="span in selectedItem.sourceSpans" :key="span.id">
                第 {{ span.chapterIndex + 1 }} 章 · {{ span.textExcerpt }}
              </blockquote>
            </section>
            <section v-if="selectedItem.aiRunSummary" class="v4-quality-inspector-block">
              <strong>AI run</strong>
              <p>{{ selectedItem.aiRunSummary }}</p>
            </section>
            <details v-if="selectedItem.diagnostic" class="v4-quality-evidence-drawer">
              <summary>诊断详情</summary>
              <pre>{{ selectedItem.diagnostic }}</pre>
            </details>
            <section class="v4-quality-boundary-note">
              <strong>边界提示</strong>
              <p>人工确认应该生成明确 correction / reprocess 意图，不允许前端或 projection 直接改 canonical。</p>
            </section>
          </template>
          <template v-else>
            <p class="v4-quality-state">选择一个队列项查看原因、证据和可执行动作。</p>
            <section class="v4-quality-boundary-note">
              <strong>边界提示</strong>
              <p>人工确认应该生成明确 correction / reprocess 意图，不允许前端或 projection 直接改 canonical。</p>
            </section>
          </template>
        </aside>
      </section>

      <p v-if="actionMessage" class="v4-quality-state">{{ actionMessage }}</p>
    </div>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue'
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
type QueueKey = 'all' | 'quarantine' | 'finding' | 'correction' | 'reprocess' | 'regression'

type TriageItem =
  | ReturnType<typeof buildQuarantineItem>
  | ReturnType<typeof buildFindingItem>
  | ReturnType<typeof buildCorrectionItem>
  | ReturnType<typeof buildReprocessItem>
  | ReturnType<typeof buildRegressionItem>

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

const actionMessage = shallowRef('')
const activeQueue = shallowRef<QueueKey>('all')
const selectedKey = shallowRef('')

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
const overviewText = computed(() => `${triageItems.value.length} open · 默认按最需要人介入排序`)
const isQualityEmpty = computed(() => (
  metrics.value.length === 0
  && quarantine.value.length === 0
  && auditRuns.value.length === 0
  && findings.value.length === 0
  && corrections.value.length === 0
  && reprocessJobs.value.length === 0
  && promptRegressionRuns.value.length === 0
))

const triageItems = computed(() => [
  ...quarantine.value.map(buildQuarantineItem),
  ...findings.value.map(buildFindingItem),
  ...corrections.value.map(buildCorrectionItem),
  ...reprocessJobs.value.map(buildReprocessItem),
  ...promptRegressionRuns.value.map(buildRegressionItem),
].sort(compareTriageItems))

const queueItems = computed(() => [
  { key: 'all' as const, label: '全部', count: triageItems.value.length, meta: '最需要人介入' },
  { key: 'quarantine' as const, label: '隔离 Claims', count: quarantine.value.length, meta: 'Decision 拒绝自动写入' },
  { key: 'finding' as const, label: '审计发现', count: findings.value.length, meta: '质量审计发现' },
  { key: 'correction' as const, label: '修正', count: corrections.value.length, meta: '待应用/已验证修正' },
  { key: 'reprocess' as const, label: '重处理', count: reprocessJobs.value.length, meta: '失败章节与 dry-run' },
  { key: 'regression' as const, label: 'Prompt 回归测试', count: promptRegressionRuns.value.length, meta: '输出结构风险' },
])

const activeQueueLabel = computed(() => queueItems.value.find((queue) => queue.key === activeQueue.value)?.label ?? '全部')
const visibleTriageItems = computed(() => (
  activeQueue.value === 'all'
    ? triageItems.value
    : triageItems.value.filter((item) => item.kind === activeQueue.value)
))
const selectedItem = computed(() => (
  visibleTriageItems.value.find((item) => item.key === selectedKey.value)
  || visibleTriageItems.value[0]
  || null
))

watch(visibleTriageItems, (items) => {
  if (!items.some((item) => item.key === selectedKey.value)) {
    selectedKey.value = items[0]?.key ?? ''
  }
}, { immediate: true })

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

function buildQuarantineItem(item: V4QualityQuarantineView) {
  const claimType = item.claim?.claimType || 'claim'
  return {
    key: `quarantine-${item.id}`,
    kind: 'quarantine' as const,
    kindLabel: '隔离 Claims',
    raw: item,
    title: item.reasonCode,
    summary: item.reasonText || item.claim?.claimType || item.claimId,
    meta: `第 ${chapterLabel(item.claim?.chapterIndex)} 章 · ${claimType} · quarantine · 不写 canonical`,
    status: item.status,
    priority: item.priority ?? 0,
    severityRank: item.priority >= 8 ? 3 : item.priority >= 4 ? 2 : 1,
    severityLabel: item.priority >= 8 ? 'H' : item.priority >= 4 ? 'M' : 'L',
    severityTone: item.priority >= 8 ? 'is-high' : item.priority >= 4 ? 'is-medium' : 'is-low',
    target: claimType,
    decision: '隔离，不写 canonical',
    suggestedAction: item.suggestedAction || 'review',
    canonicalWrite: 'none',
    claimSummary: item.claim ? `${item.claim.claimType} · ${item.claim.status} · claim ${item.claim.id}` : item.claimId,
    sourceSpans: item.sourceSpans || [],
    aiRunSummary: item.aiRun ? `${item.aiRun.runType}${item.aiRun.promptVersion ? ` · ${item.aiRun.promptVersion}` : ''}` : '',
    diagnostic: '',
  }
}

function buildFindingItem(finding: V4QualityAuditFindingView) {
  const severityRank = severityToRank(finding.severity)
  return {
    key: `finding-${finding.id}`,
    kind: 'finding' as const,
    kindLabel: '审计发现',
    raw: finding,
    title: finding.findingType,
    summary: finding.reasonText || finding.reasonCode,
    meta: `${finding.targetType}: ${finding.targetId} · ${finding.suggestedAction}`,
    status: finding.status,
    priority: severityRank,
    severityRank,
    severityLabel: finding.severity?.slice(0, 1).toUpperCase() || 'M',
    severityTone: severityRank >= 3 ? 'is-high' : severityRank === 2 ? 'is-medium' : 'is-low',
    target: `${finding.targetType}: ${finding.targetId}`,
    decision: 'Audit finding',
    suggestedAction: finding.suggestedAction,
    canonicalWrite: 'none until correction applied',
    claimSummary: finding.relatedTargetType ? `${finding.relatedTargetType}: ${finding.relatedTargetId || 'unknown'}` : '',
    sourceSpans: [],
    aiRunSummary: '',
    diagnostic: finding.evidenceJson ? formatJson(finding.evidenceJson) : '',
  }
}

function buildCorrectionItem(correction: V4QualityCorrectionView) {
  return {
    key: `correction-${correction.id}`,
    kind: 'correction' as const,
    kindLabel: '修正',
    raw: correction,
    title: correction.correctionType,
    summary: `${correction.targetType}: ${correction.targetId} · ${correction.status}`,
    meta: `${correction.source} · ${correction.error || 'ready'}`,
    status: correction.status,
    priority: correction.status === 'validated' ? 5 : 1,
    severityRank: correction.status === 'validated' ? 2 : 1,
    severityLabel: correction.status === 'validated' ? 'M' : 'L',
    severityTone: correction.status === 'validated' ? 'is-medium' : 'is-low',
    target: `${correction.targetType}: ${correction.targetId}`,
    decision: 'Correction',
    suggestedAction: 'apply when reviewed',
    canonicalWrite: 'only through correction action',
    claimSummary: '',
    sourceSpans: [],
    aiRunSummary: '',
    diagnostic: correction.error || '',
  }
}

function buildReprocessItem(job: V4QualityReprocessJobView) {
  return {
    key: `reprocess-${job.id}`,
    kind: 'reprocess' as const,
    kindLabel: '重处理',
    raw: job,
    title: job.mode,
    summary: `${job.scopeType} · ${job.status} · ${job.reason || '写入前需审核'}`,
    meta: job.dryRun ? '模拟运行' : 'may write after review',
    status: job.status,
    priority: job.status === 'failed' ? 8 : 3,
    severityRank: job.status === 'failed' ? 3 : 1,
    severityLabel: job.status === 'failed' ? 'H' : 'L',
    severityTone: job.status === 'failed' ? 'is-high' : 'is-low',
    target: job.scopeType,
    decision: job.dryRun ? 'Dry run' : 'Reprocess job',
    suggestedAction: job.status === 'queued' ? 'cancel if needed' : 'review result',
    canonicalWrite: job.dryRun ? 'none' : 'depends on backend job',
    claimSummary: '',
    sourceSpans: [],
    aiRunSummary: '',
    diagnostic: job.error || '',
  }
}

function buildRegressionItem(run: V4QualityPromptRegressionRunView) {
  return {
    key: `regression-${run.id}`,
    kind: 'regression' as const,
    kindLabel: 'Prompt 回归测试',
    raw: run,
    title: run.fixtureSet,
    summary: `${run.status} · ${formatJson(run.summaryJson)}`,
    meta: run.error || '输出结构趋势',
    status: run.status,
    priority: run.status === 'failed' ? 8 : 1,
    severityRank: run.status === 'failed' ? 3 : 1,
    severityLabel: run.status === 'failed' ? 'H' : 'L',
    severityTone: run.status === 'failed' ? 'is-high' : 'is-low',
    target: run.fixtureSet,
    decision: 'Prompt regression',
    suggestedAction: 'review fixtures',
    canonicalWrite: 'none',
    claimSummary: '',
    sourceSpans: [],
    aiRunSummary: '',
    diagnostic: run.error || '',
  }
}

function compareTriageItems(a: TriageItem, b: TriageItem): number {
  const openDelta = openRank(b.status) - openRank(a.status)
  if (openDelta !== 0) return openDelta
  const severityDelta = b.severityRank - a.severityRank
  if (severityDelta !== 0) return severityDelta
  return b.priority - a.priority
}

function openRank(status: string): number {
  return status === 'open' || status === 'queued' || status === 'failed' || status === 'validated' ? 1 : 0
}

function severityToRank(severity: string): number {
  if (severity === 'critical' || severity === 'high') return 3
  if (severity === 'medium') return 2
  return 1
}

function chapterLabel(chapterIndex: number | undefined): string {
  return typeof chapterIndex === 'number' ? String(chapterIndex + 1) : '未知'
}

defineExpose({ reload: () => panelState.reload() })
</script>

<style scoped>
.v4-quality-content,
.v4-quality-section {
  display: grid;
  gap: 12px;
}

.v4-quality-workbench {
  display: grid;
  grid-template-columns: minmax(184px, 0.62fr) minmax(320px, 1.42fr) minmax(260px, 0.9fr);
  gap: 10px;
  align-items: start;
}

.v4-quality-governance-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--color-border) 82%, transparent);
}

.v4-quality-governance-toolbar p {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 0.78rem;
  line-height: 1.65;
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

.v4-quality-queue-panel,
.v4-quality-triage-list,
.v4-quality-inspector {
  display: grid;
  gap: 10px;
}

.v4-quality-queue {
  display: grid;
  gap: 4px;
  width: 100%;
  border: 1px solid color-mix(in srgb, var(--color-border) 82%, transparent);
  border-radius: 3px;
  background: color-mix(in srgb, var(--color-bg) 94%, var(--color-bg-sunken) 6%);
  color: inherit;
  padding: 10px;
  text-align: left;
  cursor: pointer;
}

.v4-quality-queue.is-active,
.v4-quality-triage-row.is-active {
  border-color: color-mix(in srgb, var(--color-primary) 74%, var(--color-border));
  background: color-mix(in srgb, var(--color-primary) 7%, var(--color-bg));
}

.v4-quality-queue strong {
  font-size: 1.25rem;
}

.v4-quality-queue small,
.v4-quality-state,
.v4-quality-row small {
  margin: 4px 0 0;
  color: var(--color-text-tertiary, #64748b);
}

.v4-quality-row-main {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.v4-quality-row-title,
.v4-quality-row-actions,
.v4-quality-action-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.v4-quality-triage-row {
  cursor: pointer;
}

.v4-quality-severity {
  display: inline-grid;
  min-width: 24px;
  min-height: 24px;
  place-items: center;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 800;
  background: color-mix(in srgb, currentColor 6%, transparent);
}

.v4-quality-severity.is-high {
  color: #b42318;
  background: #fff0ec;
}

.v4-quality-severity.is-medium {
  color: #915930;
  background: #fff6e8;
}

.v4-quality-severity.is-low {
  color: #315970;
  background: #eef8fb;
}

.v4-quality-action,
.v4-quality-refresh {
  border: 1px solid color-mix(in srgb, var(--color-border) 88%, transparent);
  border-radius: 3px;
  background: color-mix(in srgb, var(--color-bg) 92%, var(--color-primary) 8%);
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
  border: 1px solid color-mix(in srgb, var(--color-border) 86%, transparent);
  border-radius: 3px;
  background: color-mix(in srgb, var(--color-bg) 96%, transparent);
  padding: 10px 12px;
}

.v4-quality-metric-card {
  display: grid;
  gap: 4px;
}

.v4-quality-inspector,
.v4-quality-queue-panel,
.v4-quality-triage-list {
  border: 1px solid color-mix(in srgb, var(--color-border) 88%, transparent);
  border-radius: 3px;
  background: color-mix(in srgb, var(--color-bg) 98%, transparent);
  padding: 12px;
}

.v4-quality-inspector-hero {
  display: grid;
  gap: 6px;
  padding: 12px;
  border: 1px solid color-mix(in srgb, var(--color-border) 76%, transparent);
  border-radius: 3px;
  background: color-mix(in srgb, var(--color-primary) 5%, var(--color-bg));
}

.v4-quality-inspector-hero p,
.v4-quality-inspector-block p,
.v4-quality-boundary-note p,
.v4-quality-empty-triage-row p {
  margin: 0;
}

.v4-quality-detail-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin: 0;
}

.v4-quality-detail-grid div,
.v4-quality-inspector-block,
.v4-quality-boundary-note {
  display: grid;
  gap: 4px;
  padding: 10px;
  border: 1px solid color-mix(in srgb, var(--color-border) 82%, transparent);
  border-radius: 3px;
}

.v4-quality-boundary-note {
  border-left: 3px solid color-mix(in srgb, var(--color-success, #2f855a) 72%, var(--color-border));
  background: color-mix(in srgb, var(--color-success, #2f855a) 6%, var(--color-bg));
}

.v4-quality-empty-triage-row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 10px;
  align-items: start;
  min-height: 78px;
  padding: 12px;
  border: 1px solid color-mix(in srgb, var(--color-border) 82%, transparent);
  border-radius: 3px;
  background: color-mix(in srgb, var(--color-bg-sunken) 36%, transparent);
  color: var(--color-text-secondary);
}

.v4-quality-detail-grid dt {
  color: var(--color-text-tertiary, #64748b);
  font-size: 0.72rem;
}

.v4-quality-detail-grid dd {
  margin: 0;
  font-weight: 700;
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

.v4-quality-evidence-drawer pre {
  max-width: 100%;
  overflow: auto;
  white-space: pre-wrap;
}

@media (max-width: 760px) {
  .v4-quality-workbench {
    grid-template-columns: 1fr;
  }

  .v4-quality-governance-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .v4-quality-section-head,
  .v4-quality-row {
    display: grid;
  }

  .v4-quality-detail-grid {
    grid-template-columns: 1fr;
  }
}
</style>
