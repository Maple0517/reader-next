<template>
  <V4PanelShell
    title="知识"
    :subtitle="subtitleText"
    :loading="listState.loading.value || categoryLoading"
    :error="listState.error.value || categoryError"
    :empty="listState.empty.value"
    empty-title="暂无知识"
    empty-message="当前资料还没有可展示的知识主题。"
    @retry="listState.reload()"
  >
    <template #toolbar>
      <button class="v4-knowledge-refresh" type="button" :disabled="listState.loading.value" @click="loadOverview()">
        刷新
      </button>
    </template>

    <section class="v4-knowledge-atlas" data-test="knowledge-atlas">
      <div class="v4-knowledge-atlas-head">
        <div>
          <span>Knowledge Atlas</span>
          <strong>按主题浏览世界知识</strong>
        </div>
      </div>

      <nav v-if="categories.length" class="v4-knowledge-categories" aria-label="知识分类">
        <button
          type="button"
          :class="{ active: activeCategory === '' }"
          data-test="knowledge-category-all"
          @click="loadOverview"
        >
          全部 · {{ overviewTotal }}
        </button>
        <button
          v-for="item in categories"
          :key="item.category"
          type="button"
          :class="{ active: activeCategory === item.category }"
          :data-test="`knowledge-category-${item.category}`"
          @click="loadCategory(item.category)"
        >
          {{ categoryLabel(item.category) }} · {{ item.count }}
        </button>
      </nav>
    </section>

    <div class="v4-knowledge-grid">
      <section class="v4-knowledge-card-list" aria-label="知识主题列表" data-test="knowledge-topic-cards">
        <p v-if="!cards.length" class="v4-knowledge-empty-text">暂无知识主题</p>
        <article
          v-for="card in cards"
          :key="card.id"
          class="v4-knowledge-card"
          :class="{ active: selectedCardId === card.id }"
          :data-test="`knowledge-card-${card.id}`"
          @click="loadCard(card)"
        >
          <div class="v4-knowledge-card-head">
            <strong>{{ card.topicDisplay }}</strong>
            <span class="v4-knowledge-badge">{{ confidenceLabel(card.confidence) }}</span>
          </div>
          <p>{{ card.currentSummary || '暂无当前总结' }}</p>
          <div class="v4-knowledge-card-meta">
            <span>{{ categoryLabel(card.category) }}</span>
            <span>{{ card.assertionCount }} 条断言</span>
            <span>第 {{ card.firstSeenChapter }}-{{ card.lastUpdatedChapter }} 章</span>
          </div>
        </article>
      </section>

      <section class="v4-knowledge-assertions" aria-label="知识断言详情" data-test="knowledge-topic-inspector">
        <p v-if="detailLoading" class="v4-knowledge-detail-state">断言加载中...</p>
        <p v-else-if="!selectedDetail" class="v4-knowledge-detail-state">选择一个知识主题查看断言历史。</p>
        <template v-else>
          <div class="v4-knowledge-detail-head">
            <div>
              <h3>{{ selectedDetail.card.topicDisplay }}</h3>
              <p>{{ selectedDetail.card.currentSummary || '暂无当前总结' }}</p>
            </div>
            <span class="v4-knowledge-badge">{{ confidenceLabel(selectedDetail.card.confidence) }}</span>
          </div>

          <section v-for="group in assertionGroups" :key="group.status" class="v4-knowledge-assertion-group">
            <div class="v4-knowledge-group-head">
              <strong>{{ statusLabel(group.status) }}</strong>
              <span>{{ group.items.length }}</span>
            </div>
            <article v-for="assertion in group.items" :key="assertion.id" class="v4-knowledge-assertion-card">
              <p>{{ assertion.assertionText }}</p>
              <div class="v4-knowledge-card-meta">
                <span>第 {{ assertion.chapterIndex }} 章</span>
                <span>{{ confidenceLabel(assertion.confidence) }}</span>
                <span>重要度 {{ confidenceLabel(assertion.importanceScore) }}</span>
              </div>
              <div v-if="assertion.referencedEntities.length" class="v4-knowledge-entity-chips">
                <span v-for="entity in assertion.referencedEntities" :key="`${assertion.id}-${entity.entityId}-${entity.role}`">
                  {{ entity.displayName }} · {{ entity.role }}
                </span>
              </div>
            </article>
          </section>
        </template>
      </section>
    </div>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getV4Knowledge, getV4KnowledgeCard, getV4KnowledgeCategory } from '../../api/v4/book'
import type {
  V4KnowledgeAssertionStatus,
  V4KnowledgeCardDetailView,
  V4KnowledgeCardListItem,
  V4KnowledgeCategorySummary,
} from '../../types/v4'
import { useV4PanelState } from '../../composables/useV4PanelState'
import V4PanelShell from './v4/V4PanelShell.vue'

const props = defineProps<{
  bookUrl: string
}>()

const statusOrder: V4KnowledgeAssertionStatus[] = ['active', 'rumor', 'uncertain', 'revised', 'contradicted', 'false_in_world']

const listState = useV4PanelState(
  () => getV4Knowledge(props.bookUrl),
  computed(() => props.bookUrl),
  { emptyCheck: (d) => d.cards.length === 0 && d.categories.length === 0 },
)

const detailLoading = ref(false)
const categoryLoading = ref(false)
const categoryError = ref<string | null>(null)
const overviewTotal = ref(0)
const activeCategory = ref('')
const selectedCardId = ref('')
const categories = ref<V4KnowledgeCategorySummary[]>([])
const cards = ref<V4KnowledgeCardListItem[]>([])
const selectedDetail = ref<V4KnowledgeCardDetailView | null>(null)
let detailRequestId = 0

const subtitleText = computed(() => {
  const d = listState.data.value
  if (!d) return ''
  return `${d.total} 条知识，${d.categories.length} 个分类`
})

const assertionGroups = computed(() => {
  const grouped = selectedDetail.value?.assertionsByStatus || {}
  return statusOrder
    .map((status) => ({ status, items: grouped[status] || [] }))
    .filter((group) => group.items.length > 0)
})

function confidenceLabel(value: number): string {
  return `${Math.round(value * 100)}%`
}

function categoryLabel(category: string): string {
  const labels: Record<string, string> = {
    power_system: '力量体系',
    faction_structure: '势力结构',
    world_rule: '世界规则',
    history: '历史',
    secret: '秘密',
    prophecy: '预言',
    politics: '政治',
    geography: '地理',
    custom: '自定义',
  }
  return labels[category] || category
}

function statusLabel(status: V4KnowledgeAssertionStatus): string {
  const labels: Record<V4KnowledgeAssertionStatus, string> = {
    active: '已确认',
    rumor: '传闻',
    uncertain: '不确定',
    revised: '已修正',
    contradicted: '被推翻',
    false_in_world: '世界内错误认知',
  }
  return labels[status]
}

function resetDetail() {
  selectedCardId.value = ''
  selectedDetail.value = null
}

function loadOverview() {
  activeCategory.value = ''
  categoryError.value = null
  resetDetail()
  void listState.reload()
}

async function loadCategory(category: string) {
  if (!props.bookUrl) return
  activeCategory.value = category
  resetDetail()
  categoryLoading.value = true
  categoryError.value = null
  try {
    const data = await getV4KnowledgeCategory(props.bookUrl, category)
    cards.value = data.cards || []
    // Update total from category response
    const listData = listState.data.value
    if (listData) {
      listData.total = data.total || listData.total
    }
  } catch (caughtError) {
    categoryError.value = caughtError instanceof Error && caughtError.message ? caughtError.message : '请求失败'
    cards.value = []
  } finally {
    categoryLoading.value = false
  }
}

async function loadCard(card: V4KnowledgeCardListItem) {
  if (!props.bookUrl) return
  const req = ++detailRequestId
  detailLoading.value = true
  selectedCardId.value = card.id
  try {
    const data = await getV4KnowledgeCard(props.bookUrl, card.id)
    if (req !== detailRequestId) return
    selectedDetail.value = data
  } catch {
    if (req !== detailRequestId) return
    selectedDetail.value = null
  } finally {
    if (req === detailRequestId) detailLoading.value = false
  }
}

// Sync composable data into local refs for category filtering
watch(() => listState.data.value, (data) => {
  if (data) {
    cards.value = data.cards || []
    categories.value = data.categories || []
    overviewTotal.value = data.total || 0
  }
}, { immediate: true })

// Reset on bookUrl change
watch(() => props.bookUrl, () => {
  activeCategory.value = ''
  categoryError.value = null
  resetDetail()
})

defineExpose({ reload: () => listState.reload() })
</script>

<style scoped>
.v4-knowledge-refresh {
  min-height: 34px;
  padding: 0 14px;
  border: 1px solid var(--v4-line);
  background: var(--v4-accent);
  color: #fff;
  font-weight: 800;
  cursor: pointer;
}

.v4-knowledge-refresh:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.v4-knowledge-categories,
.v4-knowledge-card-meta,
.v4-knowledge-entity-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.v4-knowledge-atlas {
  display: grid;
  gap: 12px;
  padding: 14px;
  background: var(--v4-soft);
  border: 1px solid var(--v4-line);
}

.v4-knowledge-atlas-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.v4-knowledge-atlas-head div {
  display: grid;
  gap: 4px;
}

.v4-knowledge-atlas-head span {
  color: var(--v4-muted);
  font-size: 12px;
  font-weight: 850;
}

.v4-knowledge-atlas-head strong {
  color: var(--v4-ink);
  font-size: 18px;
}

.v4-knowledge-categories button {
  border: 1px solid var(--v4-line);
  background: transparent;
  color: var(--v4-muted);
  padding: 6px 10px;
  cursor: pointer;
}

.v4-knowledge-categories button.active {
  border-color: var(--v4-accent);
  color: var(--v4-accent);
}

.v4-knowledge-grid {
  display: grid;
  grid-template-columns: minmax(220px, 0.9fr) minmax(280px, 1.1fr);
  gap: 16px;
  margin-top: 14px;
}

.v4-knowledge-card-list,
.v4-knowledge-assertions,
.v4-knowledge-assertion-group {
  display: grid;
  gap: 14px;
}

.v4-knowledge-card,
.v4-knowledge-assertion-card {
  display: grid;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--v4-line);
  background: var(--v4-soft);
}

.v4-knowledge-card {
  cursor: pointer;
}

.v4-knowledge-card.active {
  border-color: var(--v4-accent);
}

.v4-knowledge-card-head,
.v4-knowledge-detail-head,
.v4-knowledge-group-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.v4-knowledge-detail-head h3 {
  margin: 0;
}

.v4-knowledge-detail-head p {
  margin: 4px 0 0;
}

.v4-knowledge-card p {
  margin: 4px 0 0;
}

.v4-knowledge-badge {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 9px;
  background: var(--v4-soft);
  color: var(--v4-accent);
  font-size: 0.72rem;
  font-weight: 700;
  border: 1px solid var(--v4-line);
}

.v4-knowledge-card-meta {
  color: var(--v4-muted);
  font-size: 0.76rem;
}

.v4-knowledge-entity-chips span {
  background: var(--v4-soft);
  color: var(--v4-muted);
  padding: 3px 8px;
  font-size: 0.74rem;
  border: 1px solid var(--v4-line);
}

.v4-knowledge-empty-text,
.v4-knowledge-detail-state {
  margin: 0;
  color: var(--v4-muted);
  font-size: 0.84rem;
}

.v4-knowledge-assertion-card p {
  margin: 0;
}

@media (max-width: 780px) {
  .v4-knowledge-grid {
    grid-template-columns: 1fr;
  }
}
</style>
