<template>
  <div class="ai-v4-view">
    <section v-if="routeError" class="v4-empty-shell" role="alert">
      <button class="v4-back-button" type="button" @click="goBack">返回</button>
      <div>
        <p class="v4-kicker">AI Book Memory V4</p>
        <h1>AI资料加载失败</h1>
        <p>{{ routeError }}</p>
      </div>
    </section>

    <section v-else class="v4-shell" aria-label="AI Book Memory V4">
      <V4ConsoleShell
        :rail-items="railItems"
        :active-key="activeDomain"
        :loading="loading"
        :error="loadError"
        @select="selectDomain"
      >
        <template #header>
          <div class="v4-console-masthead" data-test="v4-console-masthead">
            <div class="v4-title-block">
              <button class="v4-back-button" type="button" @click="goBack">返回</button>
              <div>
                <p class="v4-kicker">{{ bookTitle }} · V4 Memory Console</p>
                <p class="v4-subtitle">{{ bookAuthor }} · {{ processingSummary }}</p>
              </div>
            </div>

            <div class="v4-action-row" aria-label="V4 memory actions">
              <span class="v4-status-chip">已读 {{ formatChapterNumber(memoryStatus?.maxReadChapter) }}</span>
              <span class="v4-status-chip">已处理 {{ formatChapterNumber(memoryStatus?.maxProcessedChapter) }}</span>
              <span class="v4-status-chip">当前 {{ currentChapterLabel }}</span>
              <button
                class="v4-action-button"
                type="button"
                :disabled="isAnyActionBusy"
                @click="refreshStatus"
              >
                刷新状态
              </button>
              <button
                class="v4-action-button"
                type="button"
                :disabled="isAnyActionBusy || !hasCurrentChapter"
                @click="generateCurrentChapter"
              >
                生成当前章节
              </button>
              <button
                class="v4-action-button"
                type="button"
                :disabled="isAnyActionBusy || !hasCurrentChapter || isV4Processing"
                @click="catchupToCurrentReading"
              >
                {{ isV4Processing ? '补齐运行中' : '补齐到当前阅读' }}
              </button>
              <button
                class="v4-action-button is-danger"
                type="button"
                :disabled="isAnyActionBusy"
                @click="resetMemory"
              >
                重置 V4 资料
              </button>
            </div>
            <p v-if="actionMessage" class="v4-action-message" role="status">{{ actionMessage }}</p>
            <p v-else-if="actionError" class="v4-action-message is-error" role="alert">{{ actionError }}</p>
          </div>
        </template>

        <V4BookOverviewPanel v-if="activeDomain === 'overview'" :key="overviewRefreshKey" :book-url="bookUrl" />
        <V4TaskProgressPanel v-else-if="activeDomain === 'task'" :book-url="bookUrl" />
        <V4CharacterPanel v-else-if="activeDomain === 'characters'" :book-url="bookUrl" />
        <V4RelationshipPanel v-else-if="activeDomain === 'relationships'" :book-url="bookUrl" :body-style="{}" />
        <V4KnowledgePanel v-else-if="activeDomain === 'knowledge'" :book-url="bookUrl" />
        <V4MapPanel v-else-if="activeDomain === 'map'" :book-url="bookUrl" />
        <V4QualityPanel v-else-if="activeDomain === 'quality'" :book-url="bookUrl" :body-style="{}" />
      </V4ConsoleShell>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getShelfBook } from '../api/bookshelf'
import {
  generateV4ChapterMemory,
  getV4CatchupStatus,
  getV4MemoryStatus,
  resetV4Memory,
  startV4Catchup,
} from '../api/v4/book'
import V4BookOverviewPanel from '../components/reader/V4BookOverviewPanel.vue'
import V4CharacterPanel from '../components/reader/V4CharacterPanel.vue'
import V4KnowledgePanel from '../components/reader/V4KnowledgePanel.vue'
import V4MapPanel from '../components/reader/V4MapPanel.vue'
import V4QualityPanel from '../components/reader/V4QualityPanel.vue'
import V4RelationshipPanel from '../components/reader/V4RelationshipPanel.vue'
import V4TaskProgressPanel from '../components/reader/V4TaskProgressPanel.vue'
import V4ConsoleShell from '../components/reader/v4/V4ConsoleShell.vue'
import type { Book } from '../types'
import type { V4CatchupStatusResponse, V4MemoryStatusResponse } from '../types/v4'

type V4DomainKey = 'overview' | 'task' | 'characters' | 'relationships' | 'knowledge' | 'map' | 'quality'
type V4ActionKey = 'refresh' | 'generate' | 'catchup' | 'reset'

const route = useRoute()
const router = useRouter()

const railItems: Array<{ key: V4DomainKey; label: string }> = [
  { key: 'overview', label: '总览' },
  { key: 'task', label: '任务' },
  { key: 'characters', label: '角色' },
  { key: 'relationships', label: '关系' },
  { key: 'knowledge', label: '知识' },
  { key: 'map', label: '地点' },
  { key: 'quality', label: '质量' },
]

const activeDomain = ref<V4DomainKey>('overview')
const book = ref<Book | null>(null)
const memoryStatus = ref<V4MemoryStatusResponse | null>(null)
const catchupStatus = ref<V4CatchupStatusResponse | null>(null)
const loading = ref(false)
const loadError = ref('')
const activeAction = ref<V4ActionKey | null>(null)
const actionMessage = ref('')
const actionError = ref('')
const overviewRefreshKey = ref(0)
let requestId = 0

const bookUrl = computed(() => normalizeQueryValue(route.query.bookUrl))
const routeChapterIndex = computed(() => {
  const rawValue = normalizeQueryValue(route.query.chapterIndex)
  if (!rawValue) return null
  const parsed = Number(rawValue)
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : null
})
const routeError = computed(() => (bookUrl.value ? '' : '缺少 bookUrl，无法加载 V4 AI资料。'))
const bookTitle = computed(() => book.value?.name || 'AI资料')
const bookAuthor = computed(() => book.value?.author || '未知作者')
const currentChapterIndex = computed(() => routeChapterIndex.value ?? book.value?.durChapterIndex ?? null)
const currentChapterLabel = computed(() => formatChapterNumber(currentChapterIndex.value))
const hasCurrentChapter = computed(() => typeof currentChapterIndex.value === 'number' && currentChapterIndex.value >= 0)
const isV4Processing = computed(() => memoryStatus.value?.processing || catchupStatus.value?.status === 'running')
const isAnyActionBusy = computed(() => activeAction.value !== null)
const processingSummary = computed(() => {
  if (memoryStatus.value?.processing) return 'V4 正在处理已读章节'
  if (catchupStatus.value?.status === 'running') return 'V4 补齐任务运行中'
  if (memoryStatus.value?.lastError) return 'V4 最近处理失败'
  return 'V4 安全资料面板'
})

watch(
  bookUrl,
  (nextBookUrl) => {
    void loadShell(nextBookUrl)
  },
  { immediate: true },
)

async function loadShell(nextBookUrl: string) {
  requestId += 1
  const currentRequestId = requestId
  book.value = null
  memoryStatus.value = null
  catchupStatus.value = null
  loadError.value = ''

  if (!nextBookUrl) {
    loading.value = false
    return
  }

  loading.value = true
  try {
    const [bookResult, statusResult, catchupResult] = await Promise.allSettled([
      getShelfBook(nextBookUrl),
      getV4MemoryStatus(nextBookUrl),
      getV4CatchupStatus(nextBookUrl),
    ])

    if (currentRequestId !== requestId) return

    if (bookResult.status === 'fulfilled') {
      book.value = bookResult.value
    } else {
      loadError.value = summarizeError(bookResult.reason, '书籍信息加载失败')
    }

    if (statusResult.status === 'fulfilled') {
      memoryStatus.value = statusResult.value
    } else if (!loadError.value) {
      loadError.value = summarizeError(statusResult.reason, 'V4 处理状态加载失败')
    }

    if (catchupResult.status === 'fulfilled') {
      catchupStatus.value = catchupResult.value
    }
  } finally {
    if (currentRequestId === requestId) {
      loading.value = false
    }
  }
}

async function refreshStatus() {
  await runAction('refresh', 'V4 状态已刷新。', async () => {
    await refreshV4Status()
  })
}

async function generateCurrentChapter() {
  if (!hasCurrentChapter.value) return
  const chapterIndex = currentChapterIndex.value as number
  await runAction('generate', '当前章节已提交生成。', async () => {
    await generateV4ChapterMemory({ bookUrl: bookUrl.value, chapterIndex })
    await refreshV4Status({ refreshOverview: true })
  })
}

async function catchupToCurrentReading() {
  if (!hasCurrentChapter.value || isV4Processing.value) return
  const targetChapterIndex = currentChapterIndex.value as number
  await runAction('catchup', '已开始补齐到当前阅读。', async () => {
    await startV4Catchup({ bookUrl: bookUrl.value, targetChapterIndex })
    await refreshV4Status()
  })
}

async function resetMemory() {
  const confirmed = window.confirm('确认重置这本书的 V4 资料？重置后可以重新生成。')
  if (!confirmed) return
  await runAction('reset', 'V4 资料已重置。', async () => {
    await resetV4Memory(bookUrl.value)
    await refreshV4Status({ refreshOverview: true })
  })
}

async function runAction(action: V4ActionKey, successMessage: string, callback: () => Promise<void>) {
  if (!bookUrl.value || activeAction.value) return
  activeAction.value = action
  actionMessage.value = ''
  actionError.value = ''
  try {
    await callback()
    actionMessage.value = successMessage
  } catch (error) {
    actionError.value = summarizeError(error, 'V4 操作失败，请稍后重试。')
  } finally {
    activeAction.value = null
  }
}

async function refreshV4Status(options: { refreshOverview?: boolean } = {}) {
  const nextBookUrl = bookUrl.value
  if (!nextBookUrl) return
  const [statusResult, catchupResult] = await Promise.allSettled([
    getV4MemoryStatus(nextBookUrl),
    getV4CatchupStatus(nextBookUrl),
  ])

  if (statusResult.status === 'fulfilled') {
    memoryStatus.value = statusResult.value
  } else {
    throw statusResult.reason
  }

  if (catchupResult.status === 'fulfilled') {
    catchupStatus.value = catchupResult.value
  } else {
    throw catchupResult.reason
  }

  if (options.refreshOverview) {
    overviewRefreshKey.value += 1
  }
}

function goBack() {
  router.back()
}

function selectDomain(key: string) {
  if (railItems.some((item) => item.key === key)) {
    activeDomain.value = key as V4DomainKey
  }
}

function normalizeQueryValue(value: unknown) {
  if (Array.isArray(value)) return typeof value[0] === 'string' ? value[0] : ''
  return typeof value === 'string' ? value : ''
}

function formatChapterNumber(index?: number | null) {
  return typeof index === 'number' ? `第 ${index + 1} 章` : '不可用'
}

function summarizeError(error: unknown, fallback: string) {
  return error instanceof Error && error.message ? error.message : fallback
}
</script>

<style scoped>
.ai-v4-view {
  height: 100%;
  min-height: 100%;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: var(--v4-bg);
  color: var(--v4-ink);
}

.v4-shell {
  min-height: 100%;
}

.v4-console-masthead {
  display: grid;
  gap: 10px;
  padding: 10px;
  border-bottom: 1px solid var(--v4-line);
  background: var(--v4-shell-top-bg);
}

.v4-title-block {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.v4-kicker {
  margin: 0;
  color: var(--v4-accent);
  font-size: 14px;
  font-weight: 850;
}

.v4-subtitle {
  margin: 0;
  color: var(--v4-muted);
  font-size: 12px;
}

.v4-back-button {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 8px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  color: var(--v4-muted);
  font-size: 11px;
  font-weight: 700;
  cursor: pointer;
}

.v4-action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.v4-action-button {
  min-height: 24px;
  padding: 0 8px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  color: var(--v4-ink);
  font-size: 11px;
  font-weight: 800;
  cursor: pointer;
}

.v4-action-button:hover:not(:disabled) {
  background: var(--v4-soft);
}

.v4-action-button.is-danger {
  color: var(--v4-danger);
}

.v4-action-button:disabled {
  cursor: not-allowed;
  opacity: 0.52;
}

.v4-status-chip {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 8px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  color: var(--v4-muted);
  font-size: 11px;
  font-weight: 800;
}

.v4-action-message {
  margin: 0;
  color: var(--v4-muted);
  font-size: 12px;
}

.v4-action-message.is-error {
  color: var(--v4-danger);
}

.v4-empty-shell {
  max-width: 560px;
  margin: 72px auto;
  display: grid;
  gap: 16px;
  padding: 22px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
}

.v4-empty-shell h1 {
  margin: 0;
  color: var(--v4-ink);
  line-height: 1.08;
}

.v4-empty-shell p {
  margin: 0;
  color: var(--v4-muted);
  line-height: 1.7;
}

@media (max-width: 768px) {
  .v4-title-block {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
