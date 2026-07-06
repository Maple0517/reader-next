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
      <header class="v4-hero-card">
        <div class="v4-hero-surface">
          <div class="v4-title-block">
            <button class="v4-back-button" type="button" @click="goBack">返回</button>
            <p class="v4-kicker">AI Book Memory V4</p>
            <h1>{{ bookTitle }}</h1>
            <p class="v4-subtitle">{{ bookAuthor }} · {{ processingSummary }}</p>
            <div class="v4-action-row" aria-label="V4 memory actions">
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

          <div class="v4-status-grid" aria-label="V4 processing status">
            <article class="v4-status-card">
              <span>已读边界</span>
              <strong>{{ formatChapterNumber(memoryStatus?.maxReadChapter) }}</strong>
            </article>
            <article class="v4-status-card">
              <span>已处理</span>
              <strong>{{ formatChapterNumber(memoryStatus?.maxProcessedChapter) }}</strong>
            </article>
            <article class="v4-status-card">
              <span>当前阅读</span>
              <strong>{{ currentChapterLabel }}</strong>
            </article>
          </div>
        </div>
      </header>

      <div v-if="loading" class="v4-state-card" role="status">正在加载 V4 资料...</div>
      <div v-else-if="loadError" class="v4-state-card is-error" role="alert">{{ loadError }}</div>

      <nav class="v4-tabs" aria-label="V4 memory sections">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          type="button"
          :class="{ active: activeTab === tab.key }"
          :aria-selected="activeTab === tab.key"
          @click="activeTab = tab.key"
        >
          {{ tab.label }}
        </button>
      </nav>

      <main class="v4-content">
        <V4BookOverviewPanel v-if="activeTab === 'overview'" :key="overviewRefreshKey" :book-url="bookUrl" />
        <V4CharacterPanel v-else-if="activeTab === 'characters'" :book-url="bookUrl" />
        <V4RelationshipPanel v-else-if="activeTab === 'relationships'" :book-url="bookUrl" :body-style="{}" />
        <V4KnowledgePanel v-else-if="activeTab === 'knowledge'" :book-url="bookUrl" :body-style="{}" />
        <V4MapPanel v-else-if="activeTab === 'map'" :book-url="bookUrl" :body-style="{}" />
        <V4IdentityPanel v-else-if="activeTab === 'identity'" :book-url="bookUrl" :body-style="{}" />
        <V4QualityPanel v-else-if="activeTab === 'quality'" :book-url="bookUrl" :body-style="{}" />
      </main>
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
import V4IdentityPanel from '../components/reader/V4IdentityPanel.vue'
import V4KnowledgePanel from '../components/reader/V4KnowledgePanel.vue'
import V4MapPanel from '../components/reader/V4MapPanel.vue'
import V4QualityPanel from '../components/reader/V4QualityPanel.vue'
import V4RelationshipPanel from '../components/reader/V4RelationshipPanel.vue'
import type { Book } from '../types'
import type { V4CatchupStatusResponse, V4MemoryStatusResponse } from '../types/v4'

type V4TabKey = 'overview' | 'characters' | 'relationships' | 'knowledge' | 'map' | 'identity' | 'quality'
type V4ActionKey = 'refresh' | 'generate' | 'catchup' | 'reset'

const route = useRoute()
const router = useRouter()

const tabs: Array<{ key: V4TabKey; label: string }> = [
  { key: 'overview', label: '总览' },
  { key: 'characters', label: '角色' },
  { key: 'relationships', label: '关系' },
  { key: 'knowledge', label: '知识' },
  { key: 'map', label: '地图' },
  { key: 'identity', label: '身份' },
  { key: 'quality', label: '质量' },
]

const activeTab = ref<V4TabKey>('overview')
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
  color: var(--color-text);
  background:
    radial-gradient(circle at 12% 0%, color-mix(in srgb, var(--color-primary) 10%, transparent), transparent 32%),
    linear-gradient(180deg, var(--color-bg) 0%, var(--color-bg-soft) 100%);
}

.v4-shell {
  max-width: 1180px;
  margin: 0 auto;
  padding: 22px clamp(16px, 3vw, 32px) calc(120px + var(--safe-area-bottom));
}

.v4-hero-card,
.v4-empty-shell,
.v4-state-card,
.v4-placeholder-panel {
  border-radius: 24px;
  background: color-mix(in srgb, var(--color-bg-soft) 88%, transparent);
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, #fff 34%, transparent),
    0 18px 60px color-mix(in srgb, var(--color-text) 8%, transparent);
}

.v4-hero-card {
  padding: 6px;
}

.v4-hero-surface {
  display: grid;
  grid-template-columns: minmax(0, 1.25fr) minmax(280px, 0.75fr);
  gap: 18px;
  align-items: stretch;
  min-height: 210px;
  padding: clamp(20px, 3vw, 30px);
  border-radius: 19px;
  background: color-mix(in srgb, var(--color-bg) 92%, var(--color-primary) 8%);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 70%, transparent);
}

.v4-title-block {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  gap: 10px;
}

.v4-kicker {
  margin: 0;
  color: var(--color-primary);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.v4-title-block h1,
.v4-empty-shell h1,
.v4-placeholder-panel h2 {
  margin: 0;
  color: var(--color-text);
  line-height: 1.08;
}

.v4-title-block h1 {
  font-size: clamp(32px, 5vw, 58px);
  letter-spacing: -0.045em;
}

.v4-subtitle,
.v4-placeholder-panel p,
.v4-empty-shell p {
  margin: 0;
  color: var(--color-text-secondary);
  line-height: 1.7;
}

.v4-back-button,
.v4-action-button,
.v4-tabs button {
  border: 0;
  cursor: pointer;
  transition:
    transform 220ms cubic-bezier(0.32, 0.72, 0, 1),
    background 220ms cubic-bezier(0.32, 0.72, 0, 1),
    color 220ms cubic-bezier(0.32, 0.72, 0, 1);
}

.v4-back-button {
  display: inline-flex;
  align-items: center;
  min-height: 34px;
  padding: 0 14px;
  border-radius: 999px;
  background: var(--color-bg-sunken);
  color: var(--color-text-secondary);
  font-weight: 700;
}

.v4-back-button:hover,
.v4-tabs button:hover {
  transform: translateY(-1px);
}

.v4-back-button:active,
.v4-action-button:active,
.v4-tabs button:active {
  transform: translateY(1px) scale(0.99);
}

.v4-action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 4px;
}

.v4-action-button {
  min-height: 36px;
  padding: 0 13px;
  border-radius: 999px;
  background: var(--color-bg-sunken);
  color: var(--color-text);
  font-weight: 800;
}

.v4-action-button:hover:not(:disabled) {
  transform: translateY(-1px);
  background: color-mix(in srgb, var(--color-primary) 12%, var(--color-bg-sunken));
}

.v4-action-button.is-danger {
  color: var(--color-danger);
  background: color-mix(in srgb, var(--color-danger) 9%, var(--color-bg-sunken));
}

.v4-action-button:disabled {
  cursor: not-allowed;
  opacity: 0.52;
}

.v4-action-message {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 13px;
  font-weight: 700;
}

.v4-action-message.is-error {
  color: var(--color-danger);
}

.v4-status-grid {
  display: grid;
  gap: 10px;
}

.v4-status-card {
  display: grid;
  align-content: center;
  gap: 6px;
  min-height: 58px;
  padding: 14px;
  border-radius: 16px;
  background: color-mix(in srgb, var(--color-bg-sunken) 82%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 70%, transparent);
}

.v4-status-card span {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 700;
}

.v4-status-card strong {
  color: var(--color-text);
  font-size: 18px;
}

.v4-state-card,
.v4-placeholder-panel,
.v4-empty-shell {
  margin-top: 16px;
  padding: 22px;
}

.v4-state-card.is-error {
  color: var(--color-danger);
}

.v4-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 18px;
  padding: 6px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-bg-sunken) 88%, transparent);
}

.v4-tabs button {
  min-height: 38px;
  padding: 0 14px;
  border-radius: 999px;
  background: transparent;
  color: var(--color-text-secondary);
  font-weight: 800;
}

.v4-tabs button.active {
  background: var(--color-primary);
  color: #fff;
}

.v4-content {
  margin-top: 16px;
}

.v4-empty-shell {
  max-width: 560px;
  margin: 72px auto;
  display: grid;
  gap: 16px;
}

@media (max-width: 768px) {
  .v4-shell {
    padding: 16px 16px calc(112px + var(--safe-area-bottom));
  }

  .v4-hero-surface {
    grid-template-columns: 1fr;
  }

  .v4-title-block h1 {
    font-size: 34px;
  }
}
</style>
