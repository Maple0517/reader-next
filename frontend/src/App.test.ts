import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'

const routeName = ref<string | symbol | null>('home')

vi.mock('vue-router', () => ({
  useRoute: () => ({ name: routeName.value }),
}))

vi.mock('./stores/app', () => ({
  useAppStore: () => ({
    showSettingsDrawer: false,
    showLoginModal: false,
    showSourceManager: false,
    showUserManager: false,
    showWebdavManager: false,
    toasts: [],
    isLoggedIn: true,
    fetchUserInfo: vi.fn().mockResolvedValue(undefined),
  }),
}))

describe('App shell chrome', () => {
  it('renders ai-book as an immersive console route without global app chrome', async () => {
    routeName.value = 'ai-book'
    const App = await import('./App.vue').then((mod) => mod.default)

    const wrapper = mount(App, {
      global: {
        stubs: {
          AppTopBar: { template: '<header data-test="app-topbar" />' },
          AppBottomNav: { template: '<nav data-test="app-bottom-nav" />' },
          SettingsDrawer: { template: '<div />' },
          LoginModal: { template: '<div />' },
          SourceManager: { template: '<div />' },
          UserManager: { template: '<div />' },
          WebdavManager: { template: '<div />' },
          RouterView: { template: '<section data-test="route-view" />' },
          TransitionGroup: { template: '<div><slot /></div>' },
        },
      },
    })

    expect(wrapper.find('[data-test="app-topbar"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="app-bottom-nav"]').exists()).toBe(false)
    expect(wrapper.get('.app-main').classes()).toContain('without-header')
    expect(wrapper.get('.app-main').classes()).not.toContain('with-bottom-nav')
  })
})
