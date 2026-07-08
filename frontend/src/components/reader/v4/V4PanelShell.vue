<template>
  <section class="v4-panel-shell" role="tabpanel">
    <div class="v4-panel-shell-inner">
      <header class="v4-panel-shell-header">
        <div>
          <h2>{{ title }}</h2>
          <p v-if="subtitle">{{ subtitle }}</p>
        </div>
        <div v-if="$slots.toolbar" class="v4-panel-shell-toolbar">
          <slot name="toolbar" />
        </div>
      </header>

      <div class="v4-panel-shell-body">
        <V4LoadingState v-if="loading" />
        <V4ErrorState
          v-else-if="error"
          :title="error"
          message=""
          retry-label="重试"
          :on-retry="() => emit('retry')"
        />
        <V4EmptyState
          v-else-if="empty"
          :title="emptyTitle ?? '暂无数据'"
          :message="emptyMessage ?? ''"
        />
        <slot v-else />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { VNode } from 'vue'
import V4EmptyState from './V4EmptyState.vue'
import V4ErrorState from './V4ErrorState.vue'
import V4LoadingState from './V4LoadingState.vue'

defineProps<{
  title: string
  subtitle?: string
  loading?: boolean
  error?: string | null
  empty?: boolean
  emptyTitle?: string
  emptyMessage?: string
}>()

defineSlots<{
  toolbar?: () => VNode[]
  default: () => VNode[]
}>()

const emit = defineEmits<{
  retry: []
}>()
</script>

<style scoped>
.v4-panel-shell {
  min-height: 100%;
  background: var(--v4-bg);
  color: var(--v4-ink);
}

.v4-panel-shell-inner {
  min-height: 100%;
  padding: 14px;
}

.v4-panel-shell-header {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 14px;
  align-items: center;
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--v4-line);
}

.v4-panel-shell-header h2,
.v4-panel-shell-header p {
  margin: 0;
}

.v4-panel-shell-header h2 {
  color: var(--v4-ink);
  font-size: 18px;
  font-weight: 850;
  letter-spacing: 0;
}

.v4-panel-shell-header p {
  margin-top: 5px;
  color: var(--v4-muted);
  line-height: 1.6;
  font-size: 13px;
}

.v4-panel-shell-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

@media (max-width: 768px) {
  .v4-panel-shell-header {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
