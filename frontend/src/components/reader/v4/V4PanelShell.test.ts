import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import V4PanelShell from './V4PanelShell.vue'

describe('V4PanelShell', () => {
  it('renders title and subtitle', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色', subtitle: '共 12 个角色' },
      slots: { default: '<p>content</p>' },
    })

    expect(wrapper.find('h2').text()).toBe('角色')
    expect(wrapper.find('.v4-panel-shell-header p').text()).toBe('共 12 个角色')
  })

  it('hides subtitle when not provided', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色' },
      slots: { default: '<p>content</p>' },
    })

    expect(wrapper.find('.v4-panel-shell-header p').exists()).toBe(false)
  })

  it('shows V4LoadingState when loading is true', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色', loading: true },
    })

    expect(wrapper.find('.v4-loading-state').exists()).toBe(true)
    expect(wrapper.find('.v4-panel-shell-body').text()).not.toContain('content')
  })

  it('shows V4ErrorState when error prop is set', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色', error: '加载失败' },
    })

    expect(wrapper.find('.v4-error-state').exists()).toBe(true)
    expect(wrapper.find('.v4-error-state').text()).toContain('加载失败')
  })

  it('emits retry when V4ErrorState retry button is clicked', async () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色', error: '加载失败' },
    })

    await wrapper.find('.v4-error-state button').trigger('click')
    expect(wrapper.emitted('retry')).toHaveLength(1)
  })

  it('shows V4EmptyState when empty is true', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色', empty: true },
    })

    expect(wrapper.find('.v4-empty-state').exists()).toBe(true)
    expect(wrapper.find('.v4-empty-state').text()).toContain('暂无数据')
  })

  it('uses custom emptyTitle and emptyMessage', () => {
    const wrapper = mount(V4PanelShell, {
      props: {
        title: '角色',
        empty: true,
        emptyTitle: '暂无角色',
        emptyMessage: '请先导入书籍',
      },
    })

    expect(wrapper.find('.v4-empty-state').text()).toContain('暂无角色')
    expect(wrapper.find('.v4-empty-state').text()).toContain('请先导入书籍')
  })

  it('renders default slot content in normal state', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色' },
      slots: { default: '<div class="test-content">角色列表</div>' },
    })

    expect(wrapper.find('.test-content').exists()).toBe(true)
    expect(wrapper.text()).toContain('角色列表')
  })

  it('renders toolbar slot when provided', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色' },
      slots: {
        toolbar: '<button>刷新</button>',
        default: '<p>content</p>',
      },
    })

    expect(wrapper.find('.v4-panel-shell-toolbar').exists()).toBe(true)
    expect(wrapper.find('.v4-panel-shell-toolbar button').text()).toBe('刷新')
  })

  it('hides toolbar wrapper when toolbar slot is not provided', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色' },
      slots: { default: '<p>content</p>' },
    })

    expect(wrapper.find('.v4-panel-shell-toolbar').exists()).toBe(false)
  })

  it('has role="tabpanel" on root element', () => {
    const wrapper = mount(V4PanelShell, {
      props: { title: '角色' },
      slots: { default: '<p>content</p>' },
    })

    expect(wrapper.find('section').attributes('role')).toBe('tabpanel')
  })
})
