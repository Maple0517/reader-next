<template>
  <nav class="v4-domain-rail" aria-label="V4 memory domains" data-test="v4-domain-rail">
    <button
      v-for="item in items"
      :key="item.key"
      type="button"
      :class="['v4-domain-rail-item', `tone-${item.tone ?? 'neutral'}`, { active: activeKey === item.key }]"
      :aria-current="activeKey === item.key ? 'page' : undefined"
      @click="emit('select', item.key)"
    >
      <span class="v4-domain-rail-label">{{ item.label }}</span>
      <span v-if="item.meta" class="v4-domain-rail-meta">{{ item.meta }}</span>
    </button>
  </nav>
</template>

<script setup lang="ts">
interface V4DomainRailItem {
  key: string
  label: string
  meta?: string
  tone?: 'neutral' | 'active' | 'warning' | 'danger' | 'success'
}

defineProps<{
  items: V4DomainRailItem[]
  activeKey: string
}>()

const emit = defineEmits<{
  select: [key: string]
}>()
</script>

<style scoped>
.v4-domain-rail {
  display: grid;
  align-content: start;
  gap: 6px;
}

.v4-domain-rail-item {
  display: grid;
  gap: 1px;
  width: 100%;
  min-height: 38px;
  padding: 6px 7px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  text-align: left;
  cursor: pointer;
}

.v4-domain-rail-item.active {
  border-color: var(--v4-accent);
  color: var(--v4-accent);
  font-weight: 850;
}

.v4-domain-rail-item:hover:not(.active) {
  background: var(--v4-soft);
}

.v4-domain-rail-label {
  font-size: 12px;
  font-weight: 800;
  line-height: 1.2;
}

.v4-domain-rail-meta {
  overflow: hidden;
  color: var(--v4-muted);
  font-size: 10px;
  font-weight: 500;
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.v4-domain-rail-item.active .v4-domain-rail-meta {
  color: var(--v4-accent);
  opacity: 0.8;
}

.v4-domain-rail-item.tone-warning .v4-domain-rail-meta { color: var(--v4-warn); }
.v4-domain-rail-item.tone-danger .v4-domain-rail-meta { color: var(--v4-danger); }
.v4-domain-rail-item.tone-success .v4-domain-rail-meta { color: var(--v4-success); }
.v4-domain-rail-item.tone-active .v4-domain-rail-meta { color: var(--v4-accent); }

@media (max-width: 820px) {
  .v4-domain-rail {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .v4-domain-rail-item {
    min-width: 80px;
  }
}
</style>
