<template>
  <V4PanelShell
    title="身份线索"
    :subtitle="subtitle"
    :loading="listState.loading.value"
    :error="listState.error.value"
    :empty="listState.empty.value"
    empty-title="暂无身份数据"
    empty-message="当前资料还没有可展示的身份链接或合并操作。"
    @retry="listState.reload()"
  >
    <template #toolbar>
      <button class="v4-identity-refresh" type="button" :disabled="listState.loading.value" @click="listState.reload()">
        刷新
      </button>
    </template>

    <div class="v4-identity-content">
      <section class="v4-identity-section">
        <div class="v4-identity-section-head">
          <strong>身份链接</strong>
          <span>{{ totalLinks }}</span>
        </div>
        <p v-if="!links.length" class="v4-identity-empty">暂无身份链接</p>
        <article v-for="link in links" :key="link.id" class="v4-identity-link-card">
          <div class="v4-identity-link-head">
            <span :class="['v4-identity-link-type', `v4-identity-link-type-${link.linkType}`]">{{ link.linkType }}</span>
            <strong>{{ confidenceLabel(link.confidence) }}</strong>
          </div>
          <div class="v4-identity-entity-pair">
            <code>{{ link.entityAId }}</code>
            <span>{{ link.linkType === 'redirect' ? '→' : '↔' }}</span>
            <code>{{ link.entityBId }}</code>
          </div>
          <div class="v4-identity-meta">
            <span>状态：{{ link.status }}</span>
            <span>来源：{{ link.sourceClaimId }}</span>
            <span v-if="link.redirectTargetId">重定向目标：{{ link.redirectTargetId }}</span>
          </div>
        </article>
      </section>

      <section class="v4-identity-section">
        <div class="v4-identity-section-head">
          <strong>合并操作</strong>
          <span>{{ totalOperations }}</span>
        </div>
        <p v-if="!operations.length" class="v4-identity-empty">暂无合并操作</p>
        <article v-for="operation in operations" :key="operation.id" class="v4-identity-merge-card">
          <div class="v4-identity-link-head">
            <span class="v4-identity-link-type">{{ operation.status }}</span>
            <strong>{{ confidenceLabel(operation.confidence) }}</strong>
          </div>
          <div class="v4-identity-entity-pair">
            <code>{{ operation.victimEntityId }}</code>
            <span>→</span>
            <code>{{ operation.survivorEntityId }}</code>
          </div>
          <div class="v4-identity-meta">
            <span>原因：{{ operation.reasonCode }}</span>
            <span>来源链接：{{ operation.sourceIdentityLinkId }}</span>
            <span>关系合并：{{ operation.relationshipMergeCount }}</span>
            <span>属性冲突：{{ operation.propertyConflictCount }}</span>
          </div>
        </article>
      </section>
    </div>
  </V4PanelShell>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { getV4IdentityLinks, getV4MergeOperations } from '../../api/v4/book'
import { useV4PanelState } from '../../composables/useV4PanelState'
import type { V4IdentityLink, V4MergeOperation } from '../../types/v4'
import V4PanelShell from './v4/V4PanelShell.vue'

interface IdentityData {
  links: V4IdentityLink[]
  operations: V4MergeOperation[]
}

const props = defineProps<{
  bookUrl: string
}>()

const listState = useV4PanelState<IdentityData>(
  async () => {
    const [linkData, operationData] = await Promise.all([
      getV4IdentityLinks(props.bookUrl),
      getV4MergeOperations(props.bookUrl),
    ])
    return {
      links: linkData.identityLinks || [],
      operations: operationData.mergeOperations || [],
    }
  },
  computed(() => props.bookUrl),
  { emptyCheck: (d) => d.links.length === 0 && d.operations.length === 0 },
)

const links = computed<V4IdentityLink[]>(() => listState.data.value?.links ?? [])
const operations = computed<V4MergeOperation[]>(() => listState.data.value?.operations ?? [])
const totalLinks = computed(() => links.value.length)
const totalOperations = computed(() => operations.value.length)

const subtitle = computed(() => {
  if (totalLinks.value > 0 || totalOperations.value > 0) {
    return `${totalLinks.value} 条身份链接，${totalOperations.value} 个合并操作`
  }
  return undefined
})

function confidenceLabel(value: number): string {
  return `${Math.round(value * 100)}%`
}
</script>

<style scoped>
.v4-identity-content,
.v4-identity-section {
  display: grid;
  gap: 10px;
}

.v4-identity-section-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  color: var(--color-text-secondary);
  font-size: 0.84rem;
}

.v4-identity-link-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.v4-identity-link-type {
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

.v4-identity-link-card,
.v4-identity-merge-card {
  display: grid;
  gap: 8px;
  padding: 10px;
  border: 1px solid color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, currentColor 2%, transparent);
}

.v4-identity-entity-pair {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.v4-identity-entity-pair code {
  padding: 2px 6px;
  border-radius: 6px;
  background: var(--color-bg-sunken);
  color: var(--color-text);
  font-size: 0.78rem;
}

.v4-identity-entity-pair span {
  color: var(--color-text-tertiary);
}

.v4-identity-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  color: var(--color-text-tertiary);
  font-size: 0.76rem;
}

.v4-identity-empty {
  margin: 0;
  color: var(--color-text-tertiary);
  font-size: 0.84rem;
}
</style>
