<template>
  <section class="v4-console" aria-label="AI Book Memory V4 console">
    <div class="v4-top-bar">
      <slot name="header" />
    </div>

    <div class="v4-console-body">
      <aside class="v4-console-rail">
        <slot name="rail-before" />
        <V4DomainRail :items="railItems" :active-key="activeKey" @select="emit('select', $event)" />
        <slot name="rail-after" />
      </aside>

      <div class="v4-console-main">
        <div v-if="loading" class="v4-console-state" role="status">正在加载 V4 资料...</div>
        <div v-else-if="error" class="v4-console-state is-error" role="alert">{{ error }}</div>

        <main class="v4-console-content">
          <slot />
        </main>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { VNode } from 'vue'
import V4DomainRail from './V4DomainRail.vue'

interface V4DomainRailItem {
  key: string
  label: string
  meta?: string
  tone?: 'neutral' | 'active' | 'warning' | 'danger' | 'success'
}

defineProps<{
  railItems: V4DomainRailItem[]
  activeKey: string
  loading?: boolean
  error?: string
}>()

defineSlots<{
  'rail-before'?: () => VNode[]
  'rail-after'?: () => VNode[]
  header?: () => VNode[]
  default: () => VNode[]
}>()

const emit = defineEmits<{
  select: [key: string]
}>()
</script>

<style scoped>
.v4-console {
  display: grid;
  grid-template-rows: auto 1fr;
  min-height: 100%;
  background: var(--v4-bg);
  color: var(--v4-ink);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  line-height: var(--v4-line-height);
}

.v4-top-bar {
  display: grid;
  gap: 8px;
  padding: 10px;
  border-bottom: 1px solid var(--v4-line);
  background: var(--v4-shell-top-bg);
}

.v4-console-body {
  display: grid;
  grid-template-columns: 116px 1fr;
  min-height: 0;
}

.v4-console-rail {
  border-right: 1px solid var(--v4-line);
  background: var(--v4-rail-bg);
  padding: 8px;
  display: grid;
  align-content: start;
  gap: 6px;
  position: sticky;
  top: 0;
  min-height: 100%;
}

.v4-console-main {
  min-width: 0;
  padding: 10px;
  background: var(--v4-bg);
}

.v4-console-state {
  margin-bottom: 10px;
  padding: 10px;
  color: var(--v4-muted);
  font-weight: 750;
  font-size: 13px;
}

.v4-console-state.is-error {
  color: var(--v4-danger);
}

.v4-console-content {
  min-width: 0;
}

@media (max-width: 820px) {
  .v4-console-body {
    grid-template-columns: 1fr;
  }

  .v4-console-rail {
    position: static;
    overflow: hidden;
    border-right: 0;
    border-bottom: 1px solid var(--v4-line);
  }
}
</style>
