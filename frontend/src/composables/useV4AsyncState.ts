import { ref } from 'vue'

export function useV4AsyncState<T>(loader: () => Promise<T>, options: { resetOnLoad?: boolean } = {}) {
  const data = ref<T | null>(null)
  const error = ref<string | null>(null)
  const loading = ref(false)
  let requestId = 0

  async function reload() {
    requestId += 1
    const currentRequestId = requestId
    loading.value = true
    error.value = null
    if (options.resetOnLoad) {
      data.value = null
    }

    try {
      const result = await loader()
      if (currentRequestId === requestId) {
        data.value = result
      }
      return result
    } catch (caughtError) {
      if (currentRequestId === requestId) {
        error.value = summarizeError(caughtError)
      }
      throw caughtError
    } finally {
      if (currentRequestId === requestId) {
        loading.value = false
      }
    }
  }

  return {
    data,
    error,
    loading,
    reload,
  }
}

function summarizeError(error: unknown) {
  return error instanceof Error && error.message ? error.message : '请求失败'
}
