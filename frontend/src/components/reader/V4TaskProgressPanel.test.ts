import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const getV4MemoryStatusMock = vi.fn()
const getV4CatchupStatusMock = vi.fn()

vi.mock('../../api/v4/book', () => ({
  getV4MemoryStatus: (...args: unknown[]) => getV4MemoryStatusMock(...args),
  getV4CatchupStatus: (...args: unknown[]) => getV4CatchupStatusMock(...args),
}))

describe('V4TaskProgressPanel', () => {
  beforeEach(() => {
    getV4MemoryStatusMock.mockReset()
    getV4CatchupStatusMock.mockReset()
    getV4MemoryStatusMock.mockResolvedValue({
      maxReadChapter: 12,
      maxProcessedChapter: 4,
      processing: false,
      lastError: null,
    })
    getV4CatchupStatusMock.mockResolvedValue({
      status: 'idle',
      targetChapter: null,
      currentChapter: null,
      maxProcessedChapter: 4,
      lastError: null,
    })
  })

  it('renders running task boundaries from existing status APIs', async () => {
    getV4MemoryStatusMock.mockResolvedValue({
      maxReadChapter: 12,
      maxProcessedChapter: 4,
      processing: true,
      lastError: null,
    })
    getV4CatchupStatusMock.mockResolvedValue({
      status: 'running',
      targetChapter: 9,
      currentChapter: 5,
      maxProcessedChapter: 4,
      lastError: null,
    })

    const V4TaskProgressPanel = await import('./V4TaskProgressPanel.vue').then((mod) => mod.default)
    const wrapper = mount(V4TaskProgressPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(getV4MemoryStatusMock).toHaveBeenCalledWith('book-1')
    expect(getV4CatchupStatusMock).toHaveBeenCalledWith('book-1')
    expect(wrapper.text()).toContain('运行中')
    expect(wrapper.text()).toContain('第 6 章')
    expect(wrapper.text()).toContain('第 10 章')
    expect(wrapper.text()).toContain('第 5 章')
    expect(wrapper.text()).toContain('章节处理中')
    expect(wrapper.find('[data-test="task-stage-running"]').classes()).toContain('active')
  })

  it('surfaces failed stage and concrete error text', async () => {
    getV4MemoryStatusMock.mockResolvedValue({
      maxReadChapter: 12,
      maxProcessedChapter: 5,
      processing: false,
      lastError: '模型响应超时',
    })
    getV4CatchupStatusMock.mockResolvedValue({
      status: 'failed',
      targetChapter: 10,
      currentChapter: 6,
      maxProcessedChapter: 5,
      lastError: 'Decision rejected unsupported knowledge claim',
    })

    const V4TaskProgressPanel = await import('./V4TaskProgressPanel.vue').then((mod) => mod.default)
    const wrapper = mount(V4TaskProgressPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.text()).toContain('失败')
    expect(wrapper.text()).toContain('失败诊断')
    expect(wrapper.text()).toContain('Decision rejected unsupported knowledge claim')
    expect(wrapper.find('[data-test="task-stage-failed"]').classes()).toContain('active')
  })

  it('renders an honest idle/completed state without fake stage counts', async () => {
    getV4MemoryStatusMock.mockResolvedValue({
      maxReadChapter: 7,
      maxProcessedChapter: 7,
      processing: false,
      lastError: null,
    })
    getV4CatchupStatusMock.mockResolvedValue({
      status: 'completed',
      targetChapter: 7,
      currentChapter: null,
      maxProcessedChapter: 7,
      lastError: null,
    })

    const V4TaskProgressPanel = await import('./V4TaskProgressPanel.vue').then((mod) => mod.default)
    const wrapper = mount(V4TaskProgressPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.text()).toContain('完成')
    expect(wrapper.text()).toContain('完成/等待')
    expect(wrapper.text()).not.toContain('claim')
    expect(wrapper.text()).not.toContain('reducer')
    expect(wrapper.text()).not.toContain('projection')
  })

  it('renders the shared error state when task status cannot load', async () => {
    getV4MemoryStatusMock.mockRejectedValue(new Error('状态接口失败'))

    const V4TaskProgressPanel = await import('./V4TaskProgressPanel.vue').then((mod) => mod.default)
    const wrapper = mount(V4TaskProgressPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.text()).toContain('状态接口失败')
  })
})
