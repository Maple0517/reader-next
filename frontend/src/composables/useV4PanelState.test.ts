import { describe, expect, it, vi } from 'vitest'
import { ref, nextTick } from 'vue'
import { useV4PanelState } from './useV4PanelState'

describe('useV4PanelState', () => {
  it('sets loading true while fetcher is pending', async () => {
    const deferred = createDeferred<string>()
    const fetcher = vi.fn(() => deferred.promise)
    const bookUrl = ref('http://example.com/book')
    const state = useV4PanelState(fetcher, bookUrl, { watchBookUrl: false })

    // reload triggers but does not await
    const promise = state.reload()
    expect(state.loading.value).toBe(true)

    deferred.resolve('data')
    await promise
    expect(state.loading.value).toBe(false)
  })

  it('sets data on successful fetcher', async () => {
    const fetcher = vi.fn(async () => ({ items: [1, 2, 3] }))
    const bookUrl = ref('http://example.com/book')
    const state = useV4PanelState(fetcher, bookUrl, { watchBookUrl: false })

    await state.reload()
    expect(state.data.value).toEqual({ items: [1, 2, 3] })
    expect(state.error.value).toBeNull()
    expect(state.loading.value).toBe(false)
  })

  it('sets error on failed fetcher without re-throwing', async () => {
    const fetcher = vi.fn(async () => { throw new Error('network down') })
    const bookUrl = ref('http://example.com/book')
    const state = useV4PanelState(fetcher, bookUrl, { watchBookUrl: false })

    // useV4PanelState swallows the error (no re-throw)
    await state.reload()
    expect(state.error.value).toBe('network down')
    expect(state.data.value).toBeNull()
    expect(state.loading.value).toBe(false)
  })

  it('derives empty from emptyCheck', async () => {
    const fetcher = vi.fn(async () => ({ characters: [] as number[] }))
    const bookUrl = ref('http://example.com/book')
    const state = useV4PanelState(fetcher, bookUrl, {
      watchBookUrl: false,
      emptyCheck: (d) => d.characters.length === 0,
    })

    // Before any data, empty should be false
    expect(state.empty.value).toBe(false)

    await state.reload()
    expect(state.empty.value).toBe(true)
  })

  it('empty is false when emptyCheck returns false', async () => {
    const fetcher = vi.fn(async () => ({ characters: [1, 2] }))
    const bookUrl = ref('http://example.com/book')
    const state = useV4PanelState(fetcher, bookUrl, {
      watchBookUrl: false,
      emptyCheck: (d) => d.characters.length === 0,
    })

    await state.reload()
    expect(state.empty.value).toBe(false)
  })

  it('reload resets state and calls fetcher again', async () => {
    let counter = 0
    const fetcher = vi.fn(async () => ++counter)
    const bookUrl = ref('http://example.com/book')
    const state = useV4PanelState(fetcher, bookUrl, { watchBookUrl: false })

    await state.reload()
    expect(state.data.value).toBe(1)

    await state.reload()
    expect(state.data.value).toBe(2)
    expect(fetcher).toHaveBeenCalledTimes(2)
  })

  it('discards stale requestId results', async () => {
    const first = createDeferred<string>()
    const second = createDeferred<string>()
    const fetcher = vi.fn<() => Promise<string>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const bookUrl = ref('http://example.com/book')
    const state = useV4PanelState(fetcher, bookUrl, { watchBookUrl: false })

    const firstRun = state.reload()
    const secondRun = state.reload()

    // second resolves first
    second.resolve('second')
    await secondRun
    expect(state.data.value).toBe('second')

    // stale first resolves later — should be discarded
    first.resolve('first')
    await firstRun
    expect(state.data.value).toBe('second')
  })

  it('auto-reloads when bookUrl changes (watchBookUrl: true)', async () => {
    let counter = 0
    const fetcher = vi.fn(async () => ++counter)
    const bookUrl = ref('http://example.com/book1')
    useV4PanelState(fetcher, bookUrl, { watchBookUrl: true })

    // immediate: true triggers reload on setup
    await nextTick()
    await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1))

    bookUrl.value = 'http://example.com/book2'
    await nextTick()
    await vi.waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2))
  })

  it('does not auto-reload when watchBookUrl is false', async () => {
    const fetcher = vi.fn(async () => 'data')
    const bookUrl = ref('http://example.com/book1')
    useV4PanelState(fetcher, bookUrl, { watchBookUrl: false })

    await nextTick()
    await new Promise(r => setTimeout(r, 50))
    expect(fetcher).not.toHaveBeenCalled()
  })
})

function createDeferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
