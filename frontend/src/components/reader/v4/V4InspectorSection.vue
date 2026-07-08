<template>
  <section class="v4-inspector-section">
    <button
      v-if="collapsible"
      class="v4-inspector-section-toggle"
      type="button"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <span>{{ title }}</span>
      <span class="v4-inspector-section-indicator">{{ expanded ? '收起' : '展开' }}</span>
    </button>
    <header v-else class="v4-inspector-section-head">
      <h3>{{ title }}</h3>
    </header>

    <div v-if="!collapsible || expanded" class="v4-inspector-section-body">
      <slot />
    </div>
  </section>
</template>

<script setup lang="ts">
import { shallowRef } from 'vue'

const props = withDefaults(
  defineProps<{
    title: string
    collapsible?: boolean
    defaultOpen?: boolean
  }>(),
  {
    collapsible: false,
    defaultOpen: true,
  },
)

const expanded = shallowRef(props.defaultOpen)
</script>

<style scoped>
.v4-inspector-section {
  display: grid;
  gap: 10px;
}

.v4-inspector-section-head h3 {
  margin: 0;
  color: var(--color-text);
  font-size: 15px;
}

.v4-inspector-section-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  min-height: 36px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-text);
  font-weight: 850;
  text-align: left;
  cursor: pointer;
}

.v4-inspector-section-indicator {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.v4-inspector-section-body {
  color: var(--color-text-secondary);
}
</style>
