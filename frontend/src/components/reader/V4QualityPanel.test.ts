import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
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

vi.mock('../../api/v4/book', () => ({
  applyV4QualityCorrection: vi.fn(),
  cancelV4QualityReprocessJob: vi.fn(),
  createV4QualityAuditRun: vi.fn(),
  createV4QualityPromptRegressionRun: vi.fn(),
  createV4QualityReprocessJob: vi.fn(),
  getV4Quality: vi.fn(),
  runV4QualityAuditFindingAction: vi.fn(),
  runV4QualityQuarantineAction: vi.fn(),
}))

const getV4QualityMock = vi.mocked(getV4Quality)
const quarantineActionMock = vi.mocked(runV4QualityQuarantineAction)
const auditRunMock = vi.mocked(createV4QualityAuditRun)
const findingActionMock = vi.mocked(runV4QualityAuditFindingAction)
const applyCorrectionMock = vi.mocked(applyV4QualityCorrection)
const createReprocessJobMock = vi.mocked(createV4QualityReprocessJob)
const cancelReprocessJobMock = vi.mocked(cancelV4QualityReprocessJob)
const promptRegressionMock = vi.mocked(createV4QualityPromptRegressionRun)

describe('V4QualityPanel', () => {
  beforeEach(() => {
    getV4QualityMock.mockReset()
    quarantineActionMock.mockReset()
    auditRunMock.mockReset()
    findingActionMock.mockReset()
    applyCorrectionMock.mockReset()
    createReprocessJobMock.mockReset()
    cancelReprocessJobMock.mockReset()
    promptRegressionMock.mockReset()
  })

  it('renders seven quality sections with Chinese labels and review-first actions', async () => {
    getV4QualityMock.mockResolvedValue({
      bookUrl: 'book-1',
      qualityMetrics: {
        quarantinedClaimCount: 2,
        duplicateEntityCandidateCount: 1,
        promptRegressionFailureCount: 1,
      },
      quarantine: [
        {
          id: 'workflow-1',
          claimId: 'claim-1',
          reasonCode: 'identity_risk',
          reasonText: 'Identity reveal needs source review',
          suggestedAction: 'accept',
          status: 'open',
          priority: 10,
          claim: { id: 'claim-1', claimType: 'identity_reveal', status: 'quarantined', chapterIndex: 8 },
          sourceSpans: [{ id: 'span-1', chapterIndex: 8, textExcerpt: '他终于承认自己就是旧名。' }],
          aiRun: { id: 'run-1', runType: 'resolve', promptVersion: 'p6' },
        },
      ],
      auditRuns: [{ id: 'audit-1', auditType: 'duplicate_entities', status: 'completed', summaryJson: { findingCount: 1 } }],
      findings: [{
        id: 'finding-1',
        findingType: 'duplicate_entity_candidate',
        severity: 'high',
        targetType: 'entity',
        targetId: 'char-a',
        reasonCode: 'alias_overlap',
        reasonText: 'Two entities share alias',
        suggestedAction: 'create_correction',
        status: 'open',
        evidenceJson: { confidence: 0.91 },
      }],
      corrections: [{ id: 'correction-1', targetType: 'claim', targetId: 'claim-1', correctionType: 'reject_claim', status: 'validated', source: 'audit' }],
      reprocessJobs: [{ id: 'job-1', scopeType: 'chapter', mode: 'dry_run_compare', status: 'queued', dryRun: true, reason: 'review before write' }],
      promptRegressionRuns: [{ id: 'reg-1', fixtureSet: 'phase6', status: 'completed', summaryJson: { passed: 14, failed: 1 } }],
    })
    quarantineActionMock.mockResolvedValue({ status: 'accepted' } as any)
    auditRunMock.mockResolvedValue({ runId: 'audit-2', status: 'completed', findingCount: 0 } as any)
    findingActionMock.mockResolvedValue({ correction: { id: 'correction-2' } } as any)
    applyCorrectionMock.mockResolvedValue({ correctionId: 'correction-1', status: 'applied', message: null } as any)
    createReprocessJobMock.mockResolvedValue({ id: 'job-2', dryRun: true } as any)
    cancelReprocessJobMock.mockResolvedValue({ id: 'job-1', status: 'cancelled' } as any)
    promptRegressionMock.mockResolvedValue({ runId: 'reg-2', status: 'completed', total: 15 } as any)

    const wrapper = mount(await import('./V4QualityPanel.vue').then((mod) => mod.default), {
      props: { bookUrl: 'book-1' },
    })
    await flushPromises()

    expect(getV4QualityMock).toHaveBeenCalledWith('book-1')

    // Chinese section labels
    expect(wrapper.text()).toContain('概览')
    expect(wrapper.text()).toContain('隔离声明')
    expect(wrapper.text()).toContain('审计发现')
    expect(wrapper.text()).toContain('修正')
    expect(wrapper.text()).toContain('重处理')
    expect(wrapper.text()).toContain('Prompt 回归测试')
    expect(wrapper.text()).toContain('指标')

    // No English labels
    expect(wrapper.text()).not.toContain('Overview')
    expect(wrapper.text()).not.toContain('Quarantine')
    expect(wrapper.text()).not.toContain('Audit Findings')
    expect(wrapper.text()).not.toContain('Corrections')
    expect(wrapper.text()).not.toContain('Reprocess')
    expect(wrapper.text()).not.toContain('Prompt Regression')
    expect(wrapper.text()).not.toContain('Metrics')

    // No Review-first pill
    expect(wrapper.text()).not.toContain('Review-first')

    // Content preserved
    expect(wrapper.text()).toContain('Identity reveal needs source review')
    expect(wrapper.text()).toContain('他终于承认自己就是旧名。')
    expect(wrapper.text()).toContain('模拟运行')
    expect(wrapper.text()).not.toContain('raw JSON')
    expect(wrapper.text()).not.toContain('fix all')
    expect(wrapper.text()).not.toContain('delete')

    // Chinese action buttons
    expect(wrapper.text()).toContain('通过审核')
    expect(wrapper.text()).toContain('运行审计')
    expect(wrapper.text()).toContain('转为修正')
    expect(wrapper.text()).toContain('重处理')  // "Queue dry-run" section head
    expect(wrapper.text()).toContain('模拟运行')  // dry-run label
    expect(wrapper.text()).toContain('取消')
    expect(wrapper.text()).toContain('证据')  // Evidence -> 证据

    // Click actions and verify Chinese messages (each overwrites previous)
    await wrapper.get('[data-test="quality-quarantine-accept-workflow-1"]').trigger('click')
    await flushPromises()
    expect(quarantineActionMock).toHaveBeenCalledWith('book-1', 'workflow-1', { action: 'accept', actor: 'reader-ui', note: 'reviewed in quality panel' })
    expect(wrapper.text()).toContain('已提交审核通过操作。')

    await wrapper.get('[data-test="quality-run-audit"]').trigger('click')
    await flushPromises()
    expect(auditRunMock).toHaveBeenCalledWith('book-1', { auditType: 'full_book_quality', scopeJson: { source: 'quality_panel' } })
    expect(wrapper.text()).toContain('已提交审查审计。')

    await wrapper.get('[data-test="quality-finding-convert-finding-1"]').trigger('click')
    await flushPromises()
    expect(findingActionMock).toHaveBeenCalledWith('book-1', 'finding-1', { action: 'convert_to_correction', actor: 'reader-ui' })
    expect(wrapper.text()).toContain('发现已转为修正。')

    await wrapper.get('[data-test="quality-apply-correction-correction-1"]').trigger('click')
    await flushPromises()
    expect(applyCorrectionMock).toHaveBeenCalledWith('book-1', 'correction-1')
    expect(wrapper.text()).toContain('已提交修正应用。')

    await wrapper.get('[data-test="quality-create-reprocess-dry-run"]').trigger('click')
    await flushPromises()
    expect(createReprocessJobMock).toHaveBeenCalledWith('book-1', expect.objectContaining({ dryRun: true, mode: 'dry_run_compare' }))
    expect(wrapper.text()).toContain('已提交模拟重处理。')

    await wrapper.get('[data-test="quality-cancel-reprocess-job-1"]').trigger('click')
    await flushPromises()
    expect(cancelReprocessJobMock).toHaveBeenCalledWith('book-1', 'job-1')
    expect(wrapper.text()).toContain('已提交重处理任务取消。')

    await wrapper.get('[data-test="quality-run-regression"]').trigger('click')
    await flushPromises()
    expect(promptRegressionMock).toHaveBeenCalledWith('book-1', expect.objectContaining({ fixtureSet: 'phase6' }))
    expect(wrapper.text()).toContain('已提交 Prompt 回归测试。')

    // Uses V4PanelShell (no panel-card class)
    expect(wrapper.find('.panel-card').exists()).toBe(false)
    expect(wrapper.find('.v4-panel-shell').exists()).toBe(true)
  })

  it('renders an honest empty quality state before audits or metrics exist', async () => {
    getV4QualityMock.mockResolvedValue({
      bookUrl: 'book-1',
      qualityMetrics: {},
      quarantine: [],
      auditRuns: [],
      findings: [],
      corrections: [],
      reprocessJobs: [],
      promptRegressionRuns: [],
    })

    const wrapper = mount(await import('./V4QualityPanel.vue').then((mod) => mod.default), {
      props: { bookUrl: 'book-1' },
    })
    await flushPromises()

    expect(wrapper.text()).toContain('尚未运行质量审计 / metrics')
    expect(wrapper.text()).toContain('catchup 不会自动生成审计结果')
  })

  it('uses V4PanelShell with correct title and subtitle', async () => {
    getV4QualityMock.mockResolvedValue({
      bookUrl: 'book-1',
      qualityMetrics: {},
      quarantine: [],
      auditRuns: [],
      findings: [],
      corrections: [],
      reprocessJobs: [],
      promptRegressionRuns: [],
    })

    const wrapper = mount(await import('./V4QualityPanel.vue').then((mod) => mod.default), {
      props: { bookUrl: 'book-1' },
    })
    await flushPromises()

    const shell = wrapper.findComponent({ name: 'V4PanelShell' })
    expect(shell.exists()).toBe(true)
    expect(shell.props('title')).toBe('质量控制')
  })
})
