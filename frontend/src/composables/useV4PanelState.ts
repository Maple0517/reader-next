import { ref, computed, watch, toValue } from 'vue'
import type { Ref, ComputedRef, MaybeRef } from 'vue'

export interface V4PanelStateOptions<T> {
  emptyCheck?: (data: T) => boolean
  watchBookUrl?: boolean // default: true
}

export function useV4PanelState<T>(
  fetcher: () => Promise<T>,
  bookUrl: MaybeRef<string>,
  options?: V4PanelStateOptions<T>
): {
  loading: Ref<boolean>
  error: Ref<string | null>
  data: Ref<T | null>
  empty: ComputedRef<boolean>
  reload: () => Promise<void>
} {
  const data = ref<T | null>(null) as Ref<T | null>
  const error = ref<string | null>(null)
  const loading = ref(false)
  const empty = computed(() => data.value !== null && (options?.emptyCheck?.(data.value as T) ?? false))
  let requestId = 0

  async function reload() {
    requestId += 1
    const currentRequestId = requestId
    loading.value = true
    error.value = null

    try {
      const result = await fetcher()
      if (currentRequestId === requestId) {
        data.value = result
      }
    } catch (caughtError) {
      if (currentRequestId === requestId) {
        error.value = summarizeError(caughtError)
      }
    } finally {
      if (currentRequestId === requestId) {
        loading.value = false
      }
    }
  }

  if (options?.watchBookUrl !== false) {
    watch(() => toValue(bookUrl), () => { reload() }, { immediate: true })
  }

  return { loading, error, data, empty, reload }
}

function summarizeError(error: unknown) {
  return error instanceof Error && error.message ? error.message : '请求失败'
}
