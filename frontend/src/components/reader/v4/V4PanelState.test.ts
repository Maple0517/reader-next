import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import V4EmptyState from './V4EmptyState.vue'
import V4ErrorState from './V4ErrorState.vue'
import V4PanelFrame from './V4PanelFrame.vue'

describe('V4 shared panel states', () => {
  it('renders a framed panel with heading, subtitle, action, and body slot', () => {
    const wrapper = mount(V4PanelFrame, {
      props: { title: 'V4 Overview', subtitle: '只读取 V4 数据' },
      slots: {
        action: '<button type="button">刷新</button>',
        default: '<p>panel body</p>',
      },
    })

    expect(wrapper.text()).toContain('V4 Overview')
    expect(wrapper.text()).toContain('只读取 V4 数据')
    expect(wrapper.text()).toContain('刷新')
    expect(wrapper.text()).toContain('panel body')
  })

  it('renders a retryable error state without dumping raw errors', async () => {
    const retry = vi.fn()
    const wrapper = mount(V4ErrorState, {
      props: { title: '加载失败', message: 'V4 资料暂时不可用', retryLabel: '重试', onRetry: retry },
    })

    expect(wrapper.text()).toContain('加载失败')
    expect(wrapper.text()).toContain('V4 资料暂时不可用')
    await wrapper.get('button').trigger('click')
    expect(retry).toHaveBeenCalledTimes(1)
  })

  it('renders a concise empty state with optional safe action', () => {
    const wrapper = mount(V4EmptyState, {
      props: { title: '暂无角色', message: '打开角色 tab 查看 V4 人物卡。', actionLabel: '查看角色' },
    })

    expect(wrapper.text()).toContain('暂无角色')
    expect(wrapper.text()).toContain('打开角色 tab 查看 V4 人物卡。')
    expect(wrapper.text()).toContain('查看角色')
  })
})
