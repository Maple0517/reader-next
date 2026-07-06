import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { getV4CharacterCard, getV4Characters } from '../../api/v4/book'
import { expectNoForbiddenV3AiBookBoundaryImports } from '../../../tests/helpers/v4BoundaryGuard'
import V4CharacterPanel from './V4CharacterPanel.vue'

vi.mock('../../api/v4/book', () => ({
  getV4Characters: vi.fn(),
  getV4CharacterCard: vi.fn(),
}))

const getV4CharactersMock = vi.mocked(getV4Characters)
const getV4CharacterCardMock = vi.mocked(getV4CharacterCard)

describe('V4CharacterPanel', () => {
  beforeEach(() => {
    getV4CharactersMock.mockReset()
    getV4CharacterCardMock.mockReset()
  })

  it('renders character grid cards with name, initial, importance and chapter range', async () => {
    getV4CharactersMock.mockResolvedValue({
      characters: [
        {
          id: 'char-1',
          name: '林玄',
          aliases: ['玄子'],
          summary: '外门弟子，刚入青云门。',
          importance: 0.8,
          firstSeenChapter: 0,
          lastSeenChapter: 4,
          visibilityScore: 0.9,
        },
      ],
      total: 1,
    })

    const wrapper = mount(V4CharacterPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(getV4CharactersMock).toHaveBeenCalledWith('book-1')
    const card = wrapper.find('[data-character-id="char-1"]')
    expect(card.exists()).toBe(true)
    expect(card.text()).toContain('林')
    expect(card.text()).toContain('林玄')
    expect(card.text()).toContain('高')
    expect(card.text()).toContain('第 1 章')
    expect(card.text()).toContain('第 5 章')
    // aliases displayed on card
    expect(card.text()).toContain('玄子')
  })

  it('maps importance to correct level labels', async () => {
    getV4CharactersMock.mockResolvedValue({
      characters: [
        {
          id: 'char-high',
          name: '角色高',
          aliases: [],
          summary: null,
          importance: 0.9,
          firstSeenChapter: 0,
          lastSeenChapter: 1,
          visibilityScore: 0.5,
        },
        {
          id: 'char-mid',
          name: '角色中',
          aliases: [],
          summary: null,
          importance: 0.5,
          firstSeenChapter: 2,
          lastSeenChapter: 3,
          visibilityScore: 0.5,
        },
        {
          id: 'char-low',
          name: '角色低',
          aliases: [],
          summary: null,
          importance: 0.1,
          firstSeenChapter: 4,
          lastSeenChapter: 5,
          visibilityScore: 0.5,
        },
      ],
      total: 3,
    })

    const wrapper = mount(V4CharacterPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.find('[data-character-id="char-high"]').text()).toContain('高')
    expect(wrapper.find('[data-character-id="char-mid"]').text()).toContain('中')
    expect(wrapper.find('[data-character-id="char-low"]').text()).toContain('低')
  })

  it('collapses low-priority characters behind an explicit reveal action', async () => {
    getV4CharactersMock.mockResolvedValue({
      characters: Array.from({ length: 14 }, (_, index) => ({
        id: `char-${index + 1}`,
        name: `角色${index + 1}`,
        aliases: [],
        summary: null,
        importance: 0.5,
        firstSeenChapter: 0,
        lastSeenChapter: index,
        visibilityScore: 1 - index * 0.01,
      })),
      total: 14,
    })

    const wrapper = mount(V4CharacterPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.findAll('[data-character-id]').length).toBe(12)
    expect(wrapper.text()).toContain('显示其余 2 位角色')
    expect(wrapper.text()).not.toContain('角色14')

    await wrapper.get('.v4-character-toggle').trigger('click')
    await flushPromises()

    expect(wrapper.findAll('[data-character-id]').length).toBe(14)
    expect(wrapper.text()).toContain('角色14')
  })

  it('loads V4 character detail after selection', async () => {
    getV4CharactersMock.mockResolvedValue({
      characters: [
        {
          id: 'char-1',
          name: '林玄',
          aliases: [],
          summary: null,
          importance: 0.8,
          firstSeenChapter: 0,
          lastSeenChapter: 4,
          visibilityScore: 0.9,
        },
      ],
      total: 1,
    })
    getV4CharacterCardMock.mockResolvedValue({
      character: {
        id: 'char-1',
        name: '林玄',
        aliases: ['玄子'],
        summary: '掌握入门心法。',
        importance: 0.8,
        firstSeenChapter: 0,
        lastSeenChapter: 4,
        currentStates: {
          realm: { label: '境界', value: '炼气一层', updatedChapter: 4, confidence: 0.92 },
          location: { label: '位置', value: '青云门', updatedChapter: 4, confidence: 0.86 },
        },
      },
      relationshipCount: 2,
      recentChanges: [],
      evidenceAvailable: true,
    })

    const wrapper = mount(V4CharacterPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()
    await wrapper.get('[data-character-id="char-1"]').trigger('click')
    await flushPromises()

    expect(getV4CharacterCardMock).toHaveBeenCalledWith('book-1', 'char-1')
    expect(wrapper.text()).toContain('掌握入门心法。')
    expect(wrapper.text()).toContain('炼气一层')
    expect(wrapper.text()).toContain('青云门')
    expect(wrapper.text()).toContain('关系 2')
  })

  it('renders evidence placeholder as deferred UI instead of absent evidence', async () => {
    getV4CharactersMock.mockResolvedValue({
      characters: [
        {
          id: 'char-1',
          name: '林玄',
          aliases: [],
          summary: null,
          importance: 0.8,
          firstSeenChapter: 0,
          lastSeenChapter: 4,
          visibilityScore: 0.9,
        },
      ],
      total: 1,
    })
    getV4CharacterCardMock.mockResolvedValue({
      character: {
        id: 'char-1',
        name: '林玄',
        aliases: [],
        summary: null,
        importance: 0.8,
        firstSeenChapter: 0,
        lastSeenChapter: 4,
        currentStates: {},
      },
      relationshipCount: 0,
      recentChanges: [],
      evidenceAvailable: false,
    })

    const wrapper = mount(V4CharacterPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()
    await wrapper.get('[data-character-id="char-1"]').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('证据面板待接入')
    expect(wrapper.text()).not.toContain('暂无证据摘要')
  })

  it('renders an empty state via V4PanelShell', async () => {
    getV4CharactersMock.mockResolvedValue({ characters: [], total: 0 })

    const wrapper = mount(V4CharacterPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.find('.v4-empty-state').exists()).toBe(true)
  })

  it('renders an error state via V4PanelShell', async () => {
    getV4CharactersMock.mockRejectedValue(new Error('V4 characters unavailable'))

    const wrapper = mount(V4CharacterPanel, { props: { bookUrl: 'book-1' } })
    await flushPromises()

    expect(wrapper.text()).toContain('V4 characters unavailable')
  })

  it('does not import V3 AI Book boundaries', () => {
    expectNoForbiddenV3AiBookBoundaryImports('src/components/reader/V4CharacterPanel.vue')
  })
})
