import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import V4ConsoleShell from './V4ConsoleShell.vue'

describe('V4ConsoleShell', () => {
  it('keeps the masthead as the full-width console header above the rail/body grid', () => {
    const wrapper = mount(V4ConsoleShell, {
      props: {
        activeKey: 'overview',
        railItems: [
          { key: 'overview', label: '总览' },
          { key: 'quality', label: '质量' },
        ],
      },
      slots: {
        header: '<div data-test="masthead">记忆控制台</div>',
        default: '<section data-test="content">content</section>',
      },
    })

    const shell = wrapper.get('.v4-console')
    const header = wrapper.get('.v4-top-bar')
    const body = wrapper.get('.v4-console-body')

    expect(shell.element.children[0]).toBe(header.element)
    expect(shell.element.children[1]).toBe(body.element)
    expect(body.find('.v4-console-rail').exists()).toBe(true)
    expect(body.find('.v4-console-main').exists()).toBe(true)
    expect(header.find('[data-test="masthead"]').exists()).toBe(true)
  })
})
