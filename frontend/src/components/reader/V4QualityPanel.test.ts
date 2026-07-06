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

  it('renders seven quality sections and keeps risky operations review-first', async () => {
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
    expect(wrapper.text()).toContain('Overview')
    expect(wrapper.text()).toContain('Quarantine')
    expect(wrapper.text()).toContain('Audit Findings')
    expect(wrapper.text()).toContain('Corrections')
    expect(wrapper.text()).toContain('Reprocess')
    expect(wrapper.text()).toContain('Prompt Regression')
    expect(wrapper.text()).toContain('Metrics')
    expect(wrapper.text()).toContain('Identity reveal needs source review')
    expect(wrapper.text()).toContain('他终于承认自己就是旧名。')
    expect(wrapper.text()).toContain('dry-run')
    expect(wrapper.text()).not.toContain('raw JSON')
    expect(wrapper.text()).not.toContain('fix all')
    expect(wrapper.text()).not.toContain('delete')

    await wrapper.get('[data-test="quality-quarantine-accept-workflow-1"]').trigger('click')
    await wrapper.get('[data-test="quality-run-audit"]').trigger('click')
    await wrapper.get('[data-test="quality-finding-convert-finding-1"]').trigger('click')
    await wrapper.get('[data-test="quality-apply-correction-correction-1"]').trigger('click')
    await wrapper.get('[data-test="quality-create-reprocess-dry-run"]').trigger('click')
    await wrapper.get('[data-test="quality-cancel-reprocess-job-1"]').trigger('click')
    await wrapper.get('[data-test="quality-run-regression"]').trigger('click')

    expect(quarantineActionMock).toHaveBeenCalledWith('book-1', 'workflow-1', { action: 'accept', actor: 'reader-ui', note: 'reviewed in quality panel' })
    expect(auditRunMock).toHaveBeenCalledWith('book-1', { auditType: 'full_book_quality', scopeJson: { source: 'quality_panel' } })
    expect(findingActionMock).toHaveBeenCalledWith('book-1', 'finding-1', { action: 'convert_to_correction', actor: 'reader-ui' })
    expect(applyCorrectionMock).toHaveBeenCalledWith('book-1', 'correction-1')
    expect(createReprocessJobMock).toHaveBeenCalledWith('book-1', expect.objectContaining({ dryRun: true, mode: 'dry_run_compare' }))
    expect(cancelReprocessJobMock).toHaveBeenCalledWith('book-1', 'job-1')
    expect(promptRegressionMock).toHaveBeenCalledWith('book-1', expect.objectContaining({ fixtureSet: 'phase6' }))
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
})
