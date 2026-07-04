<template>
  <section class="v4-knowledge-panel panel-card" :style="bodyStyle" role="tabpanel" aria-label="V4 知识">
    <div class="panel-head">
      <div>
        <h2>知识</h2>
        <p>{{ total }} 个知识主题</p>
      </div>
      <span class="knowledge-pill">V4</span>
    </div>

    <div v-if="loading" class="knowledge-state knowledge-loading">
      <span></span><span></span><span></span>
    </div>

    <p v-else-if="error" class="knowledge-state">知识资料加载失败。</p>

    <div v-else class="knowledge-content">
      <nav v-if="categories.length" class="category-row" aria-label="知识分类">
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

      <div class="knowledge-grid">
        <section class="card-list" aria-label="知识主题列表">
          <p v-if="!cards.length" class="knowledge-state">暂无知识主题</p>
          <article
            v-for="card in cards"
            :key="card.id"
            class="knowledge-card"
            :class="{ active: selectedCardId === card.id }"
            :data-test="`knowledge-card-${card.id}`"
            @click="loadCard(card)"
          >
            <div class="card-head">
              <strong>{{ card.topicDisplay }}</strong>
              <span>{{ confidenceLabel(card.confidence) }}</span>
            </div>
            <p>{{ card.currentSummary || '暂无当前总结' }}</p>
            <div class="knowledge-meta">
              <span>{{ categoryLabel(card.category) }}</span>
              <span>{{ card.assertionCount }} 条断言</span>
              <span>第 {{ card.firstSeenChapter }}-{{ card.lastUpdatedChapter }} 章</span>
            </div>
          </article>
        </section>

        <section class="assertion-panel" aria-label="知识断言详情">
          <p v-if="detailLoading" class="knowledge-state">断言加载中...</p>
          <p v-else-if="!selectedDetail" class="knowledge-state">选择一个知识主题查看断言历史。</p>
          <template v-else>
            <div class="detail-head">
              <div>
                <h3>{{ selectedDetail.card.topicDisplay }}</h3>
                <p>{{ selectedDetail.card.currentSummary || '暂无当前总结' }}</p>
              </div>
              <span>{{ confidenceLabel(selectedDetail.card.confidence) }}</span>
            </div>

            <section v-for="group in assertionGroups" :key="group.status" class="assertion-group">
              <div class="group-head">
                <strong>{{ statusLabel(group.status) }}</strong>
                <span>{{ group.items.length }}</span>
              </div>
              <article v-for="assertion in group.items" :key="assertion.id" class="assertion-card">
                <p>{{ assertion.assertionText }}</p>
                <div class="knowledge-meta">
                  <span>第 {{ assertion.chapterIndex }} 章</span>
                  <span>{{ confidenceLabel(assertion.confidence) }}</span>
                  <span>重要度 {{ confidenceLabel(assertion.importanceScore) }}</span>
                </div>
                <div v-if="assertion.referencedEntities.length" class="entity-chips">
                  <span v-for="entity in assertion.referencedEntities" :key="`${assertion.id}-${entity.entityId}-${entity.role}`">
                    {{ entity.displayName }} · {{ entity.role }}
                  </span>
                </div>
              </article>
            </section>
          </template>
        </section>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { CSSProperties } from 'vue'
import { getV4Knowledge, getV4KnowledgeCard, getV4KnowledgeCategory } from '../../api/v4/book'
import type {
  V4KnowledgeAssertionStatus,
  V4KnowledgeCardDetailView,
  V4KnowledgeCardListItem,
  V4KnowledgeCategorySummary,
} from '../../types/v4'

const props = defineProps<{
  bookUrl: string
  bodyStyle?: CSSProperties
}>()

const statusOrder: V4KnowledgeAssertionStatus[] = ['active', 'rumor', 'uncertain', 'revised', 'contradicted', 'false_in_world']
const loading = ref(false)
const detailLoading = ref(false)
const error = ref(false)
const total = ref(0)
const overviewTotal = ref(0)
const activeCategory = ref('')
const selectedCardId = ref('')
const categories = ref<V4KnowledgeCategorySummary[]>([])
const cards = ref<V4KnowledgeCardListItem[]>([])
const selectedDetail = ref<V4KnowledgeCardDetailView | null>(null)
let requestId = 0
let detailRequestId = 0

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

async function loadOverview() {
  if (!props.bookUrl) return
  const req = ++requestId
  loading.value = true
  error.value = false
  activeCategory.value = ''
  resetDetail()
  try {
    const data = await getV4Knowledge(props.bookUrl)
    if (req !== requestId) return
    cards.value = data.cards || []
    categories.value = data.categories || []
    total.value = data.total || 0
    overviewTotal.value = data.total || 0
  } catch {
    if (req !== requestId) return
    error.value = true
    cards.value = []
  } finally {
    if (req === requestId) loading.value = false
  }
}

async function loadCategory(category: string) {
  if (!props.bookUrl) return
  const req = ++requestId
  loading.value = true
  error.value = false
  activeCategory.value = category
  resetDetail()
  try {
    const data = await getV4KnowledgeCategory(props.bookUrl, category)
    if (req !== requestId) return
    cards.value = data.cards || []
    total.value = data.total || total.value
  } catch {
    if (req !== requestId) return
    error.value = true
    cards.value = []
  } finally {
    if (req === requestId) loading.value = false
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

watch(() => props.bookUrl, () => {
  void loadOverview()
}, { immediate: true })

defineExpose({ reload: loadOverview })
</script>

<style scoped>
.v4-knowledge-panel,
.knowledge-content,
.card-list,
.assertion-panel,
.assertion-group {
  display: grid;
  gap: 14px;
}

.panel-head,
.card-head,
.detail-head,
.group-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.panel-head h2,
.detail-head h3 {
  margin: 0;
}

.panel-head p,
.detail-head p,
.knowledge-card p,
.assertion-card p {
  margin: 4px 0 0;
}

.knowledge-pill,
.card-head span,
.detail-head > span,
.group-head span {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 9px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 10%, transparent);
  color: var(--color-primary, #c97f3a);
  font-size: 0.72rem;
  font-weight: 700;
}

.category-row,
.knowledge-meta,
.entity-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.category-row button {
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  border-radius: 999px;
  background: transparent;
  color: var(--color-text-secondary);
  padding: 6px 10px;
  cursor: pointer;
}

.category-row button.active {
  border-color: var(--color-primary, #c97f3a);
  color: var(--color-primary, #c97f3a);
}

.knowledge-grid {
  display: grid;
  grid-template-columns: minmax(220px, 0.9fr) minmax(280px, 1.1fr);
  gap: 16px;
}

.knowledge-card,
.assertion-card {
  display: grid;
  gap: 8px;
  padding: 12px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, currentColor 2%, transparent);
}

.knowledge-card {
  cursor: pointer;
}

.knowledge-card.active {
  border-color: var(--color-primary, #c97f3a);
}

.knowledge-meta {
  color: var(--color-text-tertiary);
  font-size: 0.76rem;
}

.entity-chips span {
  border-radius: 999px;
  background: color-mix(in srgb, currentColor 7%, transparent);
  color: var(--color-text-secondary);
  padding: 3px 8px;
  font-size: 0.74rem;
}

.knowledge-state {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: 0.84rem;
}

.knowledge-loading {
  display: inline-flex;
  gap: 5px;
  align-items: center;
}

.knowledge-loading span {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: pulse 0.9s ease-in-out infinite;
}

.knowledge-loading span:nth-child(2) {
  animation-delay: 0.12s;
}

.knowledge-loading span:nth-child(3) {
  animation-delay: 0.24s;
}

@media (max-width: 780px) {
  .knowledge-grid {
    grid-template-columns: 1fr;
  }
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.25;
  }

  50% {
    opacity: 1;
  }
}
</style>
