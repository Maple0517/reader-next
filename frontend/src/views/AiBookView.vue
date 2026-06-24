<template>
  <div class="ai-book-view">
    <div v-if="loading" class="ai-loading">
      <div class="loading-spinner"></div>
      <span>加载中...</span>
    </div>

    <div v-else-if="book && memoryView" class="ai-shell">
      <header class="ai-header">
        <div class="title-stack">
          <div class="title-row">
            <button class="back-btn" @click="goBack">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="m15 18-6-6 6-6" />
              </svg>
              返回
            </button>
            <h1>{{ book.name }}</h1>
          </div>
          <p>{{ book.author || '未知作者' }} · {{ progressText }}</p>
        </div>

        <div class="header-actions">
          <label class="enable-switch">
            <input type="checkbox" :checked="memoryView.enabled" @change="toggleEnabled" />
            <span></span>
            自动更新
          </label>
          <button class="primary-btn" :disabled="generateDisabled" @click="generateCurrentChapter">
            {{ generateButtonLabel }}
          </button>
          <button class="secondary-btn" :disabled="catchupActionDisabled" @click="toggleCatchup">
            {{ catchupActionLabel }}
          </button>
          <button class="danger-btn" :disabled="aiStore.isBusy" @click="resetMemory">
            重置
          </button>
        </div>
      </header>

      <div v-if="catchupStatus" class="catchup-strip" :class="`is-${catchupStatus.status}`">
        <div class="catchup-head">
          <div class="catchup-main">
            <strong>补齐任务 · {{ catchupStatusLabel }}</strong>
            <span>{{ catchupProgressSummary }}</span>
          </div>
          <small>{{ catchupUpdatedAtText }}</small>
        </div>
        <div class="catchup-progress-track" role="progressbar" :aria-valuenow="catchupProgressPercent" aria-valuemin="0" aria-valuemax="100">
          <div class="catchup-progress-bar" :style="{ width: `${catchupProgressPercent}%` }"></div>
        </div>
        <p>{{ catchupDetailText }}</p>
      </div>

      <div v-if="statusNotice" class="status-strip" :class="{ error: statusNotice.isError }">
        <strong>{{ statusNotice.isError ? '生成失败' : '状态' }}</strong>
        <p>{{ statusNotice.summary }}</p>
        <details v-if="statusNotice.detail">
          <summary>查看详情</summary>
          <pre>{{ statusNotice.detail }}</pre>
        </details>
      </div>

      <nav class="tabs">
        <button v-for="tab in tabs" :key="tab.key" :class="{ active: activeTab === tab.key }" @click="activeTab = tab.key">
          {{ tab.label }}
        </button>
      </nav>

      <main class="ai-content">
        <section v-if="activeTab === 'overview'" class="overview-grid">
          <article class="panel-card current-summary-card">
            <div class="panel-head">
              <div>
                <h2>当前章节</h2>
                <p>{{ currentChapterLabel }}</p>
              </div>
              <span class="pill">{{ chapterGenerationStatusLabel }}</span>
            </div>
            <p class="summary">{{ currentChapterSummary }}</p>
            <ul v-if="currentKeyPoints.length" class="bullet-list">
              <li v-for="item in currentKeyPoints" :key="item">{{ item }}</li>
            </ul>
            <EmptyState v-else text="当前章节还没有摘要" />
          </article>

          <article class="panel-card">
            <div class="panel-head">
              <div>
                <h2>全书摘要</h2>
                <p>后端 V3 视图模型</p>
              </div>
              <span class="pill">{{ processedChapterLabel }}</span>
            </div>
            <p class="summary">{{ memoryView.summary.current || '暂无资料' }}</p>
            <div class="summary-columns">
              <div>
                <strong>最近变化</strong>
                <ul class="bullet-list compact">
                  <li v-for="item in memoryView.summary.recentChanges" :key="item">{{ item }}</li>
                  <li v-if="!memoryView.summary.recentChanges.length" class="muted">暂无</li>
                </ul>
              </div>
              <div>
                <strong>未解问题</strong>
                <ul class="bullet-list compact">
                  <li v-for="item in memoryView.summary.openQuestions" :key="item">{{ item }}</li>
                  <li v-if="!memoryView.summary.openQuestions.length" class="muted">暂无</li>
                </ul>
              </div>
            </div>
          </article>

          <article class="panel-card">
            <div class="panel-head">
              <div>
                <h2>角色状态</h2>
                <p>来自章节 digest.characterStates</p>
              </div>
            </div>
            <div class="state-grid">
              <article v-for="item in currentCharacterStates" :key="`${item.name}-${item.status}`" class="state-item">
                <strong>{{ item.name }}</strong>
                <p>{{ item.status || item.description || '暂无状态' }}</p>
                <small>{{ item.lastSeenChapterTitle || formatChapter(item.lastSeenChapterIndex) || '当前章节' }}</small>
              </article>
            </div>
            <EmptyState v-if="!currentCharacterStates.length" text="当前章节还没有角色状态" />
          </article>

          <article class="panel-card">
            <div class="panel-head">
              <div>
                <h2>世界观</h2>
                <p>{{ memoryView.knowledgeFacts.length }} 条</p>
              </div>
            </div>
            <div class="worldview-groups">
              <section v-for="group in worldviewGroups" :key="group.category" class="worldview-group">
                <div class="group-head">
                  <strong>{{ group.category }}</strong>
                  <span>{{ group.items.length }}</span>
                </div>
                <article v-for="note in group.items" :key="note.id" class="list-item">
                  <div class="item-title">
                    <h3>{{ note.title }}</h3>
                    <span>{{ note.confidence }}</span>
                  </div>
                  <p>{{ note.content }}</p>
                  <details v-if="hasEvidence(note.evidence)" class="evidence-block">
                    <summary>来源</summary>
                    <ul>
                      <li v-for="item in visibleEvidence(note.evidence)" :key="evidenceKey(item)">
                        <strong>{{ evidenceChapterLabel(item) }}</strong>
                        <span>{{ item.note }}</span>
                        <blockquote v-if="item.quote">{{ item.quote }}</blockquote>
                      </li>
                    </ul>
                  </details>
                </article>
              </section>
            </div>
            <EmptyState v-if="!worldviewGroups.length" text="暂无世界观资料" />
          </article>
        </section>

        <section v-else-if="activeTab === 'characters'" class="stack-panel">
          <article v-for="character in displayCharacters" :key="character.id" class="list-item">
            <div class="item-title">
              <h3>{{ character.name }}</h3>
              <span>{{ character.importance || 'unknown' }}</span>
            </div>
            <p>{{ character.currentStatus || character.description || '暂无状态' }}</p>
            <div class="meta-line">
              <span v-if="character.aliases.length">别名：{{ character.aliases.join('、') }}</span>
              <span v-if="character.lastSeenChapter">最近：{{ character.lastSeenChapter }}</span>
            </div>
            <details v-if="hasEvidence(character.evidence)" class="evidence-block">
              <summary>来源</summary>
              <ul>
                <li v-for="item in visibleEvidence(character.evidence)" :key="evidenceKey(item)">
                  <strong>{{ evidenceChapterLabel(item) }}</strong>
                  <span>{{ item.note }}</span>
                  <blockquote v-if="item.quote">{{ item.quote }}</blockquote>
                </li>
              </ul>
            </details>
          </article>
          <EmptyState v-if="!displayCharacters.length" text="暂无角色资料" />
        </section>

        <section v-else-if="activeTab === 'relationships'" class="stack-panel">
          <article v-for="relationship in displayRelationships" :key="relationship.id" class="list-item">
            <div class="item-title relation-head">
              <strong>{{ relationship.sourceName }}</strong>
              <span>{{ relationship.label }}</span>
              <strong>{{ relationship.targetName }}</strong>
            </div>
            <p>{{ relationship.summary || relationship.status || '暂无说明' }}</p>
            <div class="meta-line">
              <span>kind：{{ relationship.kind }}</span>
              <span>polarity：{{ relationship.polarity }}</span>
              <span>strength：{{ relationship.strength }}</span>
            </div>
            <details v-if="hasEvidence(relationship.evidence)" class="evidence-block">
              <summary>来源</summary>
              <ul>
                <li v-for="item in visibleEvidence(relationship.evidence)" :key="evidenceKey(item)">
                  <strong>{{ evidenceChapterLabel(item) }}</strong>
                  <span>{{ item.note }}</span>
                  <blockquote v-if="item.quote">{{ item.quote }}</blockquote>
                </li>
              </ul>
            </details>
          </article>

          <article v-if="currentChapterRelations.length" class="panel-card">
            <div class="panel-head">
              <div>
                <h2>当前章节关系变化</h2>
                <p>来自章节 digest.characterRelations</p>
              </div>
            </div>
            <ul class="bullet-list">
              <li v-for="item in currentChapterRelations" :key="`${item.source}-${item.target}-${item.kind}-${item.status}`">
                {{ item.source }} · {{ item.target }} · {{ item.kind }} · {{ item.status }}
                <span v-if="item.description"> — {{ item.description }}</span>
              </li>
            </ul>
          </article>
          <EmptyState v-if="!displayRelationships.length && !currentChapterRelations.length" text="暂无关系资料" />
        </section>

        <section v-else class="stack-panel">
          <article class="panel-card">
            <div class="panel-head map-head">
              <div>
                <h2>地图</h2>
                <p>{{ mapStatusText }}</p>
              </div>
              <button class="secondary-btn" disabled>
                暂未开放
              </button>
            </div>
            <div class="map-frame">
              <EmptyState text="V3 切换期间，地图生成与持久化暂未接入；当前页仅保留地点资料。" />
            </div>
          </article>

          <article class="panel-card">
            <div class="panel-head">
              <div>
                <h2>地点</h2>
                <p>{{ memoryView.locations.length }} 个地点</p>
              </div>
            </div>
            <article v-for="location in displayLocations" :key="location.id" class="list-item">
              <div class="item-title">
                <h3>{{ location.name }}</h3>
                <span>{{ location.kind || location.scale }}</span>
              </div>
              <p>{{ location.description || '暂无说明' }}</p>
              <div class="meta-line">
                <span v-if="location.currentStatus">状态：{{ location.currentStatus }}</span>
                <span v-if="location.parentName">上级：{{ location.parentName }}</span>
                <span v-if="location.firstSeenChapter">首次：{{ location.firstSeenChapter }}</span>
              </div>
              <details v-if="hasEvidence(location.evidence)" class="evidence-block">
                <summary>来源</summary>
                <ul>
                  <li v-for="item in visibleEvidence(location.evidence)" :key="evidenceKey(item)">
                    <strong>{{ evidenceChapterLabel(item) }}</strong>
                    <span>{{ item.note }}</span>
                    <blockquote v-if="item.quote">{{ item.quote }}</blockquote>
                  </li>
                </ul>
              </details>
            </article>
            <EmptyState v-if="!displayLocations.length" text="暂无地点资料" />
          </article>
        </section>
      </main>
    </div>

    <div v-else class="ai-empty-panel">
      <button class="back-btn" @click="goBack">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="m15 18-6-6 6-6" />
        </svg>
        返回
      </button>
      <h2>AI资料加载失败</h2>
      <p>{{ loadError || '无法加载这本书的 AI资料' }}</p>
      <small>如果刚切换过账号，当前账号书架可能没有这本书。</small>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, defineComponent, h, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getShelfBook } from '../api/bookshelf'
import { useAiBookStore } from '../stores/aiBook'
import { useAppStore } from '../stores/app'
import { useReaderStore } from '../stores/reader'
import type { AiBookCatchupStatus, AiBookCatchupTaskStatus, AiBookEvidence, Book } from '../types'
import { describeCatchupDetail, describeCatchupProgress } from '../utils/aiBookCatchupStatus'
import { collapseWhitespace, summarizeDisplayError } from '../utils/httpError'

type AiTab = 'overview' | 'characters' | 'relationships' | 'map'

type DisplayCharacter = {
  id: string
  name: string
  aliases: string[]
  importance: string
  description: string
  currentStatus: string
  lastSeenChapter: string
  evidence: AiBookEvidence[]
}

type DisplayRelationship = {
  id: string
  sourceName: string
  targetName: string
  label: string
  summary: string
  kind: string
  polarity: string
  strength: string
  status: string
  evidence: AiBookEvidence[]
}

type DisplayLocation = {
  id: string
  name: string
  kind: string
  scale: string
  description: string
  currentStatus: string
  parentName: string
  firstSeenChapter: string
  evidence: AiBookEvidence[]
}

const EmptyState = defineComponent({
  props: { text: { type: String, required: true } },
  setup(props) {
    return () => h('div', { class: 'empty-state' }, props.text)
  },
})

const route = useRoute()
const router = useRouter()
const aiStore = useAiBookStore()
const appStore = useAppStore()
const readerStore = useReaderStore()
const loading = ref(true)
const loadError = ref('')
const activeTab = ref<AiTab>('overview')
const book = ref<Book | null>(null)
const catchupStatus = ref<AiBookCatchupStatus | null>(null)
const catchupActionPending = ref(false)
let catchupPollTimer: number | null = null
let catchupDisposed = false

const tabs: Array<{ key: AiTab; label: string }> = [
  { key: 'overview', label: '总览' },
  { key: 'characters', label: '角色' },
  { key: 'relationships', label: '关系' },
  { key: 'map', label: '地图' },
]

const memoryView = computed(() => {
  const currentBook = book.value
  const currentMemory = aiStore.memoryView
  if (!currentBook || !currentMemory || currentMemory.bookUrl !== currentBook.bookUrl) {
    return null
  }
  return currentMemory
})
const chapterMemory = computed(() => {
  const currentBook = book.value
  const currentChapter = aiStore.chapterMemory
  if (!currentBook || !currentChapter || currentChapter.bookUrl !== currentBook.bookUrl || currentChapter.chapterIndex !== currentChapterIndex.value) {
    return null
  }
  return currentChapter
})
const currentChapterIndex = computed(() => {
  if (readerStore.book?.bookUrl === book.value?.bookUrl) {
    return Math.max(0, readerStore.currentIndex)
  }
  return Math.max(0, book.value?.durChapterIndex || 0)
})
const currentChapterLabel = computed(() => {
  const title = chapterMemory.value?.chapterTitle
    || (readerStore.book?.bookUrl === book.value?.bookUrl ? readerStore.currentChapter?.title : book.value?.durChapterTitle)
    || ''
  const prefix = formatChapter(currentChapterIndex.value)
  return title ? `${prefix} · ${title}` : prefix
})
const processedChapterLabel = computed(() => formatChapter(memoryView.value?.processedChapterIndex))
const progressText = computed(() => {
  const processed = processedChapterLabel.value
  const current = currentChapterLabel.value
  if (memoryView.value?.processedChapterIndex == null) {
    return `当前阅读 ${current}`
  }
  return `${processed} 已入库 · 当前阅读 ${current}`
})
const characterNameById = computed(() => new Map((memoryView.value?.characters || []).map((item) => [item.id, item.name])))
const locationNameById = computed(() => new Map((memoryView.value?.locations || []).map((item) => [item.id, item.name])))
const chapterStateByName = computed(() => new Map((chapterMemory.value?.digest?.characterStates || []).map((item) => [item.name, item])))
const currentChapterSummary = computed(() => chapterMemory.value?.digest?.summary || chapterMemory.value?.lastError || '当前章节还没有摘要')
const currentKeyPoints = computed(() => chapterMemory.value?.digest?.keyPoints || [])
const currentCharacterStates = computed(() => chapterMemory.value?.digest?.characterStates || [])
const currentChapterRelations = computed(() => chapterMemory.value?.digest?.characterRelations || [])
const displayCharacters = computed<DisplayCharacter[]>(() => (memoryView.value?.characters || []).map((character) => {
  const chapterState = chapterStateByName.value.get(character.name)
  return {
    id: character.id,
    name: character.name,
    aliases: character.aliases || [],
    importance: character.importance || 'unknown',
    description: character.description || '',
    currentStatus: chapterState?.status || '',
    lastSeenChapter: chapterState?.lastSeenChapterTitle || formatChapter(chapterState?.lastSeenChapterIndex ?? character.lastSeenChapterIndex ?? undefined),
    evidence: character.evidence,
  }
}))
const displayRelationships = computed<DisplayRelationship[]>(() => (memoryView.value?.relationships || []).map((relationship) => ({
  id: relationship.id,
  sourceName: characterNameById.value.get(relationship.sourceCharacterId) || relationship.sourceCharacterId,
  targetName: characterNameById.value.get(relationship.targetCharacterId) || relationship.targetCharacterId,
  label: relationship.label,
  summary: relationship.summary,
  kind: relationship.kind,
  polarity: relationship.polarity,
  strength: relationship.strength,
  status: relationship.status,
  evidence: relationship.evidence,
})))
const displayLocations = computed<DisplayLocation[]>(() => (memoryView.value?.locations || []).map((location) => ({
  id: location.id,
  name: location.name,
  kind: location.kind,
  scale: location.scale,
  description: location.description,
  currentStatus: location.currentStatus || '',
  parentName: location.parentLocationId ? locationNameById.value.get(location.parentLocationId) || '' : '',
  firstSeenChapter: formatChapter(location.firstSeenChapterIndex ?? undefined),
  evidence: location.evidence,
})))
const worldviewGroups = computed(() => {
  const groups = new Map<string, NonNullable<typeof memoryView.value>["knowledgeFacts"]>()
  for (const fact of memoryView.value?.knowledgeFacts || []) {
    const key = fact.category || 'unknown'
    const items = groups.get(key) || []
    items.push(fact)
    groups.set(key, items)
  }
  return [...groups.entries()].map(([category, items]) => ({ category, items }))
})
const mapStatusText = computed(() => '地图生成功能已暂时禁用')
const generateDisabled = computed(() => aiStore.isBusy || !book.value)
const generateButtonLabel = computed(() => aiStore.phase === 'text' ? '生成中...' : '生成当前章节')
const pollingStatuses = new Set<AiBookCatchupTaskStatus>(['running', 'canceling', 'pausing'])
const terminalStatuses = new Set<AiBookCatchupTaskStatus>(['paused', 'canceled', 'completed', 'failed'])
const isCatchupRunning = computed(() => catchupStatus.value ? pollingStatuses.has(catchupStatus.value.status) : false)
const catchupActionDisabled = computed(() => aiStore.isBusy || catchupActionPending.value || !book.value)
const catchupActionLabel = computed(() => {
  if (catchupStatus.value?.status === 'canceling' || catchupStatus.value?.status === 'pausing') return '取消中...'
  if (isCatchupRunning.value) return '取消补齐'
  return '补齐到当前进度'
})
const catchupStatusLabel = computed(() => describeCatchupStatus(catchupStatus.value?.status || 'idle'))
const catchupProgressPercent = computed(() => {
  const total = Math.max(catchupStatus.value?.totalChapters || 0, 0)
  const completed = Math.max(catchupStatus.value?.completedChapters || 0, 0)
  if (!total) return catchupStatus.value?.status === 'completed' ? 100 : 0
  return Math.round((Math.min(completed, total) / total) * 100)
})
const catchupProgressSummary = computed(() => {
  if (!catchupStatus.value) return ''
  return describeCatchupProgress(catchupStatus.value)
})
const catchupDetailText = computed(() => {
  if (!catchupStatus.value) return ''
  return describeCatchupDetail(catchupStatus.value)
})
const catchupUpdatedAtText = computed(() => catchupStatus.value?.updatedAt ? `更新于 ${formatTime(catchupStatus.value.updatedAt)}` : '')
const chapterGenerationStatusLabel = computed(() => chapterMemory.value?.generationStatus || 'idle')
const statusNotice = computed(() => {
  const errorText = [
    aiStore.statusText,
    chapterMemory.value?.lastError,
    memoryView.value?.lastError,
    catchupStatus.value?.status === 'failed' ? catchupStatus.value.error : '',
  ].find((item) => (item || '').trim()) || ''
  if (!errorText) return null
  const summary = summarizeDisplayError(errorText)
  const normalized = collapseWhitespace(errorText)
  return {
    summary,
    detail: normalized !== summary ? errorText : '',
    isError: aiStore.phase === 'error' || Boolean(chapterMemory.value?.lastError) || Boolean(memoryView.value?.lastError),
  }
})

onMounted(async () => {
  catchupDisposed = false
  const bookUrl = String(route.query.bookUrl || '')
  if (!bookUrl) {
    loadError.value = '缺少书籍参数，无法加载 AI资料'
    loading.value = false
    return
  }
  try {
    await appStore.fetchUserInfo()
    loadError.value = ''
    book.value = await getShelfBook(bookUrl)
    await aiStore.load(book.value)
    await loadCurrentChapterMemory()
    await refreshCatchupStatus(true)
  } catch (error) {
    loadError.value = (error as Error).message || 'AI资料加载失败'
    appStore.showToast(loadError.value, 'error')
  } finally {
    loading.value = false
  }
})

onUnmounted(() => {
  catchupDisposed = true
  stopCatchupPolling()
})

watch(
  () => [book.value?.bookUrl, currentChapterIndex.value] as const,
  ([bookUrl]) => {
    if (!bookUrl || loading.value) return
    void loadCurrentChapterMemory()
  },
)

function goBack() {
  router.back()
}

async function toggleEnabled(event: Event) {
  if (!book.value) return
  const enabled = (event.target as HTMLInputElement).checked
  try {
    await aiStore.setEnabled(book.value, enabled)
    appStore.showToast(enabled ? '已开启自动更新' : '已关闭自动更新', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '设置失败', 'error')
  }
}

async function loadCurrentChapterMemory() {
  if (!book.value) return
  try {
    await aiStore.loadChapterMemory(book.value.bookUrl, currentChapterIndex.value)
  } catch {
    // 章节视图允许缺失，页面继续渲染全书视图
  }
}

async function generateCurrentChapter() {
  if (!book.value) return
  try {
    await aiStore.generateChapterMemory({
      bookUrl: book.value.bookUrl,
      chapterIndex: currentChapterIndex.value,
      mode: 'manual',
    })
    appStore.showToast('当前章节 AI资料已生成', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '当前章节生成失败', 'error')
  }
}

async function toggleCatchup() {
  if (!book.value) return
  if (isCatchupRunning.value) {
    await cancelCatchupTask()
    return
  }
  await startCatchupTask()
}

async function startCatchupTask() {
  if (!book.value) return
  catchupActionPending.value = true
  try {
    const status = await aiStore.startCatchup({
      bookUrl: book.value.bookUrl,
      targetChapterIndex: currentChapterIndex.value,
    })
    await applyCatchupStatus(status)
    appStore.showToast(status.status === 'completed' ? '补齐完成' : '已启动补齐任务', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '补齐任务启动失败', 'error')
  } finally {
    catchupActionPending.value = false
  }
}

async function cancelCatchupTask() {
  if (!book.value) return
  catchupActionPending.value = true
  try {
    const status = await aiStore.cancelCatchup(book.value.bookUrl)
    await applyCatchupStatus(status)
    appStore.showToast('补齐任务已取消', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '补齐任务取消失败', 'error')
  } finally {
    catchupActionPending.value = false
  }
}

async function resetMemory() {
  if (!book.value) return
  if (!confirm('确定重置当前书的 AI资料？')) return
  try {
    await aiStore.reset(book.value)
    await loadCurrentChapterMemory()
    catchupStatus.value = null
    appStore.showToast('AI资料已重置', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '重置失败', 'error')
  }
}

async function refreshCatchupStatus(silent = false) {
  if (!book.value || catchupDisposed) return
  try {
    const status = await aiStore.loadCatchupStatus(book.value.bookUrl)
    await applyCatchupStatus(status)
  } catch (error) {
    if (!silent) {
      appStore.showToast((error as Error).message || '补齐任务状态获取失败', 'error')
    }
  }
}

async function applyCatchupStatus(status: AiBookCatchupStatus | null) {
  if (!status || catchupDisposed) return
  catchupStatus.value = status
  if (pollingStatuses.has(status.status)) {
    scheduleCatchupPoll()
    return
  }
  stopCatchupPolling()
  if (terminalStatuses.has(status.status) && book.value) {
    await aiStore.load(book.value)
    await loadCurrentChapterMemory()
  }
}

function scheduleCatchupPoll() {
  stopCatchupPolling()
  if (catchupDisposed || !catchupStatus.value || !pollingStatuses.has(catchupStatus.value.status)) return
  catchupPollTimer = window.setTimeout(() => {
    void refreshCatchupStatus(true)
  }, 2000)
}

function stopCatchupPolling() {
  if (catchupPollTimer != null) {
    window.clearTimeout(catchupPollTimer)
    catchupPollTimer = null
  }
}

function describeCatchupStatus(status: AiBookCatchupTaskStatus) {
  switch (status) {
    case 'running':
      return '运行中'
    case 'canceling':
    case 'pausing':
      return '取消中'
    case 'canceled':
      return '已取消'
    case 'paused':
      return '已暂停'
    case 'completed':
      return '已完成'
    case 'failed':
      return '失败'
    default:
      return '未开始'
  }
}

function formatChapter(index?: number | null) {
  if (typeof index !== 'number') return '当前章节'
  return `第 ${index + 1} 章`
}

function formatTime(value: number | string) {
  return new Date(value).toLocaleString()
}

function hasEvidence(evidence: AiBookEvidence[] | undefined) {
  return Boolean(evidence?.length)
}

function visibleEvidence(evidence: AiBookEvidence[] | undefined) {
  return (evidence || []).slice(-3).reverse()
}

function evidenceKey(evidence: AiBookEvidence) {
  return `${evidence.chapterIndex}-${evidence.chapterTitle}-${evidence.note}-${evidence.quote || ''}`
}

function evidenceChapterLabel(evidence: AiBookEvidence) {
  return evidence.chapterTitle || formatChapter(evidence.chapterIndex)
}

</script>

<style scoped>
.ai-book-view {
  height: 100%;
  overflow: hidden;
  background: var(--color-bg);
  color: var(--color-text);
}

.ai-loading {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--color-text-secondary);
}

.ai-shell {
  height: 100%;
  display: flex;
  flex-direction: column;
  max-width: 1240px;
  margin: 0 auto;
  padding: 16px 24px 22px;
  box-sizing: border-box;
  min-height: 0;
}

.ai-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: center;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--color-border-light);
}

.title-stack,
.title-row {
  min-width: 0;
}

.title-row {
  display: flex;
  gap: 12px;
  align-items: center;
}

.ai-header h1,
.ai-empty-panel h2,
.panel-head h2,
.item-title h3 {
  margin: 0;
}

.ai-header p,
.panel-head p,
.ai-empty-panel p,
small,
.meta-line {
  color: var(--color-text-tertiary);
}

.back-btn,
.primary-btn,
.secondary-btn,
.danger-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 34px;
  padding: 0 13px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
  font-weight: 600;
  cursor: pointer;
}

.back-btn svg {
  width: 16px;
  height: 16px;
}

.primary-btn {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

.danger-btn {
  color: var(--color-danger, #d14b4b);
}

button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.header-actions,
.catchup-head,
.catchup-main,
.panel-head,
.item-title,
.relation-head,
.map-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.header-actions {
  flex-wrap: wrap;
  justify-content: flex-end;
}

.enable-switch {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  color: var(--color-text-secondary);
}

.enable-switch input {
  display: none;
}

.enable-switch span {
  width: 36px;
  height: 20px;
  border-radius: 999px;
  background: var(--color-border);
  position: relative;
}

.enable-switch span::after {
  content: "";
  position: absolute;
  width: 16px;
  height: 16px;
  left: 2px;
  top: 2px;
  border-radius: 50%;
  background: #fff;
  transition: transform var(--duration-fast);
}

.enable-switch input:checked + span {
  background: var(--color-primary);
}

.enable-switch input:checked + span::after {
  transform: translateX(16px);
}

.catchup-strip,
.status-strip,
.panel-card,
.list-item,
.ai-empty-panel {
  border: 1px solid var(--color-border-light);
  border-radius: 12px;
  background: var(--color-bg-elevated);
}

.catchup-strip,
.status-strip {
  margin-top: 14px;
  padding: 12px 14px;
}

.catchup-strip {
  display: grid;
  gap: 8px;
  background: rgba(68, 140, 255, 0.08);
}

.catchup-strip.is-completed,
.catchup-strip.is-canceled {
  background: rgba(58, 181, 115, 0.1);
}

.catchup-strip.is-failed {
  background: rgba(209, 75, 75, 0.1);
}

.catchup-strip.is-canceling,
.catchup-strip.is-pausing {
  background: rgba(201, 127, 58, 0.1);
}

.catchup-progress-track {
  position: relative;
  overflow: hidden;
  border-radius: 999px;
  height: 8px;
  background: rgba(255, 255, 255, 0.45);
}

.catchup-progress-bar {
  height: 100%;
  background: var(--color-primary);
}

.status-strip {
  display: grid;
  gap: 8px;
  font-size: 13px;
}

.status-strip.error {
  background: rgba(209, 75, 75, 0.12);
}

.status-strip p,
.list-item p,
.summary {
  margin: 0;
  line-height: 1.7;
  color: var(--color-text-secondary);
}

.status-strip pre {
  margin: 8px 0 0;
  padding: 10px;
  overflow: auto;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.46);
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 12px;
}

.tabs {
  display: flex;
  gap: 4px;
  margin-top: 10px;
  border-bottom: 1px solid var(--color-border-light);
}

.tabs button {
  padding: 10px 15px;
  color: var(--color-text-tertiary);
  font-weight: 600;
  border-bottom: 2px solid transparent;
}

.tabs button.active {
  color: var(--color-primary);
  border-color: var(--color-primary);
}

.ai-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 14px 0 28px;
}

.overview-grid,
.stack-panel,
.worldview-groups,
.worldview-group,
.state-grid,
.summary-columns {
  display: grid;
  gap: 14px;
}

.summary-columns {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.state-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.panel-card,
.list-item {
  padding: 14px;
}

.pill {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 10px;
  border-radius: 999px;
  background: var(--color-bg-sunken);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 700;
}

.bullet-list {
  margin: 0;
  padding-left: 18px;
  color: var(--color-text-secondary);
  line-height: 1.7;
}

.bullet-list.compact {
  display: grid;
  gap: 6px;
}

.state-item {
  padding: 12px;
  border-radius: 10px;
  background: var(--color-bg-sunken);
}

.state-item p,
.state-item small {
  margin-top: 6px;
}

.group-head strong,
.item-title strong {
  color: var(--color-text);
}

.item-title span,
.group-head span {
  color: var(--color-primary);
  font-size: 12px;
}

.meta-line {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 10px;
  font-size: 12px;
}

.evidence-block {
  margin-top: 10px;
  font-size: 12px;
}

.evidence-block summary {
  cursor: pointer;
  color: var(--color-primary);
  font-weight: 700;
}

.evidence-block ul {
  margin: 8px 0 0;
  padding-left: 16px;
  display: grid;
  gap: 8px;
}

.evidence-block strong {
  display: block;
}

.evidence-block blockquote {
  margin: 4px 0 0;
  padding-left: 8px;
  border-left: 2px solid var(--color-border);
  color: var(--color-text-muted);
}

.map-frame {
  min-height: min(50vh, 480px);
  margin-top: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid var(--color-border-light);
  background: #1f2522;
}

.map-frame img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.empty-state,
.muted {
  color: var(--color-text-tertiary);
}

.ai-empty-panel {
  max-width: 520px;
  margin: 72px auto;
  padding: 24px;
}

@media (max-width: 768px) {
  .ai-shell {
    padding: 16px;
  }

  .ai-header,
  .title-row,
  .header-actions,
  .catchup-head,
  .catchup-main,
  .panel-head,
  .item-title,
  .relation-head,
  .map-head,
  .summary-columns,
  .state-grid {
    grid-template-columns: 1fr;
    flex-direction: column;
    align-items: stretch;
  }

  .tabs {
    overflow-x: auto;
  }

  .tabs button {
    flex: 0 0 auto;
  }
}
</style>
