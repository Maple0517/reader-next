import { describe, expect, it, vi } from 'vitest'
import { useV4AsyncState } from './useV4AsyncState'

describe('useV4AsyncState', () => {
  it('keeps the newest response when older requests resolve later', async () => {
    const first = createDeferred<string>()
    const second = createDeferred<string>()
    const loader = vi.fn<() => Promise<string>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const state = useV4AsyncState(loader)

    const firstRun = state.reload()
    const secondRun = state.reload()

    second.resolve('second')
    await secondRun
    expect(state.data.value).toBe('second')
    expect(state.loading.value).toBe(false)

    first.resolve('first')
    await firstRun
    expect(state.data.value).toBe('second')
    expect(state.error.value).toBeNull()
  })

  it('sets an error only for the latest failed request', async () => {
    const first = createDeferred<string>()
    const second = createDeferred<string>()
    const loader = vi.fn<() => Promise<string>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const state = useV4AsyncState(loader)

    const firstRun = state.reload()
    const secondRun = state.reload()

    second.reject(new Error('latest failed'))
    await secondRun.catch(() => undefined)
    expect(state.error.value).toBe('latest failed')

    first.resolve('stale success')
    await firstRun
    expect(state.data.value).toBeNull()
    expect(state.error.value).toBe('latest failed')
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
