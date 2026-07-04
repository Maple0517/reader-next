import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  applyV4QualityCorrection,
  cancelV4QualityReprocessJob,
  createV4QualityAuditRun,
  createV4QualityCorrection,
  createV4QualityPromptRegressionRun,
  createV4QualityReprocessJob,
  getV4Quality,
  getV4QualityAuditFindings,
  getV4QualityAuditRuns,
  getV4QualityCorrections,
  getV4QualityPromptRegressionResults,
  getV4QualityPromptRegressionRuns,
  getV4QualityQuarantine,
  getV4QualityReprocessJobs,
  runV4QualityAuditFindingAction,
  runV4QualityQuarantineAction,
} from './book'
import v4Http from './http'
import type {
  V4QualityAuditRunRequest,
  V4QualityCorrectionRequest,
  V4QualityPromptRegressionRunRequest,
  V4QualityReprocessJobRequest,
} from '../../types/v4'

vi.mock('./http', () => ({
  default: {
    get: vi.fn(),
    post: vi.fn(),
  },
}))

const httpGetMock = vi.mocked(v4Http.get)
const httpPostMock = vi.mocked(v4Http.post)

describe('V4 quality API', () => {
  beforeEach(() => {
    httpGetMock.mockReset()
    httpPostMock.mockReset()
  })

  it('loads quality overview and all quality lists with shared bookUrl params', async () => {
    httpGetMock.mockResolvedValue({ data: { total: 0, items: [] } } as any)

    await getV4Quality('book-1')
    await getV4QualityQuarantine('book-1', { status: 'open' })
    await getV4QualityAuditRuns('book-1', { auditType: 'duplicate_entities' })
    await getV4QualityAuditFindings('book-1', { severity: 'high' })
    await getV4QualityCorrections('book-1', { correctionType: 'reject_claim' })
    await getV4QualityReprocessJobs('book-1', { mode: 'dry_run_compare' })
    await getV4QualityPromptRegressionRuns('book-1', { fixtureSet: 'phase6' })
    await getV4QualityPromptRegressionResults('book-1', 'run/1')

    expect(httpGetMock).toHaveBeenNthCalledWith(1, '/quality', { params: { bookUrl: 'book-1' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(2, '/quality/quarantine', { params: { bookUrl: 'book-1', status: 'open' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(3, '/quality/audit-runs', { params: { auditType: 'duplicate_entities', bookUrl: 'book-1' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(4, '/quality/audit-findings', { params: { bookUrl: 'book-1', severity: 'high' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(5, '/quality/corrections', { params: { bookUrl: 'book-1', correctionType: 'reject_claim' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(6, '/quality/reprocess-jobs', { params: { bookUrl: 'book-1', mode: 'dry_run_compare' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(7, '/quality/prompt-regression-runs', { params: { bookUrl: 'book-1', fixtureSet: 'phase6' } })
    expect(httpGetMock).toHaveBeenNthCalledWith(8, `/quality/prompt-regression-runs/${encodeURIComponent('run/1')}/results`, { params: { bookUrl: 'book-1' } })
  })

  it('posts safe review actions and dry-run operations to quality endpoints', async () => {
    httpPostMock.mockResolvedValue({ data: { status: 'ok' } } as any)
    const auditRun: V4QualityAuditRunRequest = { auditType: 'full_book_quality', scopeJson: { dryRun: true } }
    const correction: V4QualityCorrectionRequest = {
      targetType: 'claim',
      targetId: 'claim-1',
      correctionType: 'reject_claim',
      correctionJson: { reason: 'bad evidence' },
      source: 'user',
      createdBy: 'tester',
    }
    const reprocess: V4QualityReprocessJobRequest = {
      scopeType: 'chapter',
      scopeJson: { chapterIndex: 3 },
      mode: 'dry_run_compare',
      requestedBy: 'tester',
      reason: 'review first',
      dryRun: true,
    }
    const promptRegression: V4QualityPromptRegressionRunRequest = {
      promptVersion: 'phase6',
      schemaVersion: 'v4',
      model: 'mock',
      fixtureSet: 'phase6',
    }

    await runV4QualityQuarantineAction('book-1', 'workflow/1', { action: 'accept', actor: 'tester', note: 'source checked' })
    await createV4QualityAuditRun('book-1', auditRun)
    await runV4QualityAuditFindingAction('book-1', 'finding/1', { action: 'convert_to_correction', actor: 'tester' })
    await createV4QualityCorrection('book-1', correction)
    await applyV4QualityCorrection('book-1', 'correction/1')
    await createV4QualityReprocessJob('book-1', reprocess)
    await cancelV4QualityReprocessJob('book-1', 'job/1')
    await createV4QualityPromptRegressionRun('book-1', promptRegression)

    expect(httpPostMock).toHaveBeenNthCalledWith(1, `/quality/quarantine/${encodeURIComponent('workflow/1')}/action`, {
      action: 'accept',
      actor: 'tester',
      bookUrl: 'book-1',
      note: 'source checked',
    })
    expect(httpPostMock).toHaveBeenNthCalledWith(2, '/quality/audit-runs', { ...auditRun, bookUrl: 'book-1' })
    expect(httpPostMock).toHaveBeenNthCalledWith(3, `/quality/audit-findings/${encodeURIComponent('finding/1')}/action`, {
      action: 'convert_to_correction',
      actor: 'tester',
      bookUrl: 'book-1',
    })
    expect(httpPostMock).toHaveBeenNthCalledWith(4, '/quality/corrections', { ...correction, bookUrl: 'book-1' })
    expect(httpPostMock).toHaveBeenNthCalledWith(5, `/quality/corrections/${encodeURIComponent('correction/1')}/apply`, { bookUrl: 'book-1' })
    expect(httpPostMock).toHaveBeenNthCalledWith(6, '/quality/reprocess-jobs', { ...reprocess, bookUrl: 'book-1' })
    expect(httpPostMock).toHaveBeenNthCalledWith(7, `/quality/reprocess-jobs/${encodeURIComponent('job/1')}/cancel`, { bookUrl: 'book-1' })
    expect(httpPostMock).toHaveBeenNthCalledWith(8, '/quality/prompt-regression-runs', { ...promptRegression, bookUrl: 'book-1' })
  })
})
