<template>
  <V4PanelShell
    title="角色"
    subtitle="小说中出现的重要角色"
    :loading="listState.loading.value"
    :error="listState.error.value"
    :empty="listState.empty.value"
    empty-title="暂无角色"
    empty-message="当前资料还没有可展示的角色。"
    @retry="listState.reload()"
  >
    <template #toolbar>
      <button class="v4-character-refresh" type="button" :disabled="listState.loading.value" @click="listState.reload()">
        刷新
      </button>
    </template>

    <div class="v4-character-grid" aria-label="角色列表">
      <button
        v-for="character in visibleCharacters"
        :key="character.id"
        type="button"
        class="v4-character-card"
        :class="{ active: selectedCharacterId === character.id }"
        :data-character-id="character.id"
        @click="selectCharacter(character.id)"
      >
        <span class="v4-character-avatar">{{ character.name.charAt(0) }}</span>
        <span class="v4-character-name">{{ character.name }}</span>
        <span v-if="character.aliases.length" class="v4-character-aliases">{{ character.aliases.join('、') }}</span>
        <span class="v4-character-importance" :class="importanceClass(character.importance)">{{ importanceLabel(character.importance) }}</span>
        <span class="v4-character-chapters">{{ formatChapter(character.firstSeenChapter) }} ~ {{ formatChapter(character.lastSeenChapter) }}</span>
      </button>
      <button
        v-if="hiddenCharacterCount > 0 || showAllCharacters"
        type="button"
        class="v4-character-toggle"
        @click="showAllCharacters = !showAllCharacters"
      >
        {{ showAllCharacters ? '收起次要角色' : `显示其余 ${hiddenCharacterCount} 位角色` }}
      </button>
    </div>

    <aside class="v4-character-detail" aria-label="角色详情">
      <V4EmptyState
        v-if="!selectedCharacterId && !detailLoading"
        title="选择角色"
        message="点击卡片查看当前状态、摘要和证据情况。"
      />
      <V4LoadingState v-else-if="detailLoading" title="正在加载人物卡" />
      <V4ErrorState
        v-else-if="detailError"
        title="人物卡加载失败"
        :message="detailError"
        retry-label="重试"
        :on-retry="reloadDetail"
      />
      <article v-else-if="detail" class="v4-character-detail-card">
        <div class="v4-character-detail-head">
          <div>
            <h3>{{ detail.character.name }}</h3>
            <p v-if="detail.character.aliases.length" class="v4-character-detail-aliases">{{ detail.character.aliases.join('、') }}</p>
            <p v-if="detail.character.summary" class="v4-character-detail-summary">{{ detail.character.summary }}</p>
          </div>
          <span class="v4-character-detail-count">关系 {{ detail.relationshipCount }}</span>
        </div>

        <dl v-if="detailStates.length" class="v4-character-states">
          <div v-for="state in detailStates" :key="state.key">
            <dt>{{ state.label }}</dt>
            <dd>{{ state.value }}</dd>
            <small>{{ formatChapter(state.updatedChapter) }} · 置信 {{ formatScore(state.confidence) }}</small>
          </div>
        </dl>

        <div class="v4-character-evidence">
          {{ detail.evidenceAvailable ? '有证据可追溯' : '证据面板待接入' }}
        </div>
      </article>
    </aside>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getV4CharacterCard, getV4Characters } from '../../api/v4/book'
import type { V4CharacterCardView, V4CharacterListItem, V4CharacterResponse } from '../../types/v4'
import { useV4PanelState } from '../../composables/useV4PanelState'
import V4EmptyState from './v4/V4EmptyState.vue'
import V4ErrorState from './v4/V4ErrorState.vue'
import V4LoadingState from './v4/V4LoadingState.vue'
import V4PanelShell from './v4/V4PanelShell.vue'

const props = defineProps<{
  bookUrl: string
}>()

const listState = useV4PanelState(
  () => getV4Characters(props.bookUrl),
  computed(() => props.bookUrl),
  { emptyCheck: (d) => d.characters.length === 0 },
)

const selectedCharacterId = ref('')
const detail = ref<V4CharacterResponse | null>(null)
const detailLoading = ref(false)
const detailError = ref('')
const showAllCharacters = ref(false)
let detailRequestId = 0
const DEFAULT_VISIBLE_CHARACTER_COUNT = 12

const characters = computed<V4CharacterListItem[]>(() => listState.data.value?.characters || [])
const visibleCharacters = computed<V4CharacterListItem[]>(() => {
  if (showAllCharacters.value || characters.value.length <= DEFAULT_VISIBLE_CHARACTER_COUNT) {
    return characters.value
  }
  return characters.value.slice(0, DEFAULT_VISIBLE_CHARACTER_COUNT)
})
const hiddenCharacterCount = computed(() =>
  Math.max(0, characters.value.length - visibleCharacters.value.length),
)
const detailStates = computed(() => {
  const currentStates: V4CharacterCardView['currentStates'] = detail.value?.character.currentStates || {}
  return Object.entries(currentStates).map(([key, value]) => ({ key, ...value }))
})

watch(
  () => props.bookUrl,
  () => {
    showAllCharacters.value = false
    selectedCharacterId.value = ''
    detail.value = null
    detailError.value = ''
  },
)

async function selectCharacter(characterId: string) {
  selectedCharacterId.value = characterId
  await loadDetail(characterId)
}

async function reloadDetail() {
  if (!selectedCharacterId.value) return
  await loadDetail(selectedCharacterId.value)
}

async function loadDetail(characterId: string) {
  detailRequestId += 1
  const currentRequestId = detailRequestId
  detail.value = null
  detailError.value = ''
  detailLoading.value = true
  try {
    const nextDetail = await getV4CharacterCard(props.bookUrl, characterId)
    if (currentRequestId === detailRequestId) {
      detail.value = nextDetail
    }
  } catch (error) {
    if (currentRequestId === detailRequestId) {
      detailError.value = summarizeError(error)
    }
  } finally {
    if (currentRequestId === detailRequestId) {
      detailLoading.value = false
    }
  }
}

function importanceLabel(value: number) {
  if (value >= 0.7) return '高'
  if (value >= 0.3) return '中'
  return '低'
}

function importanceClass(value: number) {
  if (value >= 0.7) return 'importance-high'
  if (value >= 0.3) return 'importance-mid'
  return 'importance-low'
}

function formatChapter(index?: number | null) {
  return typeof index === 'number' ? `第 ${index + 1} 章` : '不可用'
}

function formatScore(value?: number | null) {
  if (typeof value !== 'number') return '不可用'
  return value.toFixed(2)
}

function summarizeError(error: unknown) {
  return error instanceof Error && error.message ? error.message : '请求失败'
}
</script>

<style scoped>
.v4-character-refresh {
  min-height: 34px;
  padding: 0 14px;
  border: 0;
  border-radius: 999px;
  background: var(--color-primary);
  color: #fff;
  font-weight: 800;
  cursor: pointer;
  transition: transform 220ms cubic-bezier(0.32, 0.72, 0, 1), opacity 220ms cubic-bezier(0.32, 0.72, 0, 1);
}

.v4-character-refresh:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.v4-character-refresh:active:not(:disabled) {
  transform: translateY(1px) scale(0.98);
}

.v4-character-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 12px;
}

.v4-character-card {
  display: grid;
  gap: 6px;
  width: 100%;
  padding: 14px;
  text-align: center;
  border: 2px solid transparent;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-bg-sunken) 76%, transparent);
  color: var(--color-text);
  cursor: pointer;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 64%, transparent);
  transition:
    transform 220ms cubic-bezier(0.32, 0.72, 0, 1),
    background 220ms cubic-bezier(0.32, 0.72, 0, 1),
    border-color 220ms cubic-bezier(0.32, 0.72, 0, 1),
    box-shadow 220ms cubic-bezier(0.32, 0.72, 0, 1);
}

.v4-character-card:hover {
  transform: translateY(-1px);
  background: color-mix(in srgb, var(--color-primary) 10%, var(--color-bg));
}

.v4-character-card.active {
  border-color: var(--color-primary);
  background: color-mix(in srgb, var(--color-primary) 10%, var(--color-bg));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-primary) 28%, transparent);
}

.v4-character-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  margin: 0 auto;
  border-radius: 50%;
  background: color-mix(in srgb, var(--color-primary) 15%, var(--color-bg));
  color: var(--color-primary);
  font-size: 20px;
  font-weight: 900;
}

.v4-character-name {
  font-size: 15px;
  font-weight: 900;
  letter-spacing: -0.02em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.v4-character-aliases {
  color: var(--color-text-secondary);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.v4-character-importance {
  display: inline-block;
  margin: 0 auto;
  padding: 2px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 800;
}

.importance-high {
  background: rgba(40, 167, 69, 0.12);
  color: #28a745;
}

.importance-mid {
  background: color-mix(in srgb, var(--color-primary) 12%, var(--color-bg));
  color: var(--color-primary);
}

.importance-low {
  background: var(--color-bg-sunken);
  color: var(--color-text-tertiary);
}

.v4-character-chapters {
  color: var(--color-text-secondary);
  font-size: 12px;
}

.v4-character-toggle {
  grid-column: 1 / -1;
  min-height: 38px;
  padding: 0 14px;
  border: 0;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-primary) 10%, var(--color-bg));
  color: var(--color-primary);
  font-weight: 800;
  cursor: pointer;
}

.v4-character-detail {
  margin-top: 20px;
}

.v4-character-detail-card {
  display: grid;
  gap: 14px;
  padding: 16px;
  border-radius: 20px;
  background: color-mix(in srgb, var(--color-bg) 92%, var(--color-primary) 8%);
  box-shadow:
    inset 0 0 0 1px color-mix(in srgb, var(--color-border) 68%, transparent),
    inset 0 1px 0 color-mix(in srgb, #fff 26%, transparent);
}

.v4-character-detail-head {
  display: flex;
  justify-content: space-between;
  gap: 14px;
  align-items: flex-start;
}

.v4-character-detail-head h3,
.v4-character-detail-head p {
  margin: 0;
}

.v4-character-detail-head h3 {
  font-size: 24px;
  letter-spacing: -0.03em;
}

.v4-character-detail-aliases {
  margin-top: 4px;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.v4-character-detail-summary {
  margin-top: 6px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.v4-character-detail-count {
  flex: 0 0 auto;
  min-height: 30px;
  padding: 5px 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-primary) 12%, var(--color-bg));
  color: var(--color-primary);
  font-size: 12px;
  font-weight: 900;
}

.v4-character-states {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin: 0;
}

.v4-character-states div {
  padding: 12px;
  border-radius: 16px;
  background: color-mix(in srgb, var(--color-bg-sunken) 82%, transparent);
}

.v4-character-states dt,
.v4-character-states dd {
  margin: 0;
}

.v4-character-states dt,
.v4-character-states small {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.v4-character-states dd {
  margin-top: 4px;
  color: var(--color-text);
  font-weight: 900;
}

.v4-character-states small {
  display: block;
  margin-top: 5px;
}

.v4-character-evidence {
  padding: 12px;
  border-radius: 16px;
  background: color-mix(in srgb, var(--color-primary) 9%, var(--color-bg));
  color: var(--color-text-secondary);
  font-weight: 800;
}

@media (max-width: 768px) {
  .v4-character-grid {
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
  }

  .v4-character-states {
    grid-template-columns: 1fr;
  }

  .v4-character-detail-head {
    flex-direction: column;
  }
}
</style>
