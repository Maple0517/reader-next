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
  padding: 5px;
  border-radius: 22px;
  background: color-mix(in srgb, var(--color-bg-sunken) 82%, transparent);
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, #fff 24%, transparent),
    0 14px 45px color-mix(in srgb, var(--color-text) 7%, transparent);
}

.v4-panel-shell-inner {
  min-height: 100%;
  padding: clamp(16px, 2.2vw, 22px);
  border-radius: 17px;
  background: color-mix(in srgb, var(--color-bg) 94%, var(--color-primary) 6%);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 68%, transparent);
}

.v4-panel-shell-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
  margin-bottom: 16px;
}

.v4-panel-shell-header h2,
.v4-panel-shell-header p {
  margin: 0;
}

.v4-panel-shell-header h2 {
  color: var(--color-text);
  font-size: 20px;
  letter-spacing: -0.02em;
}

.v4-panel-shell-header p {
  margin-top: 5px;
  color: var(--color-text-secondary);
  line-height: 1.6;
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
  }
}
</style>
