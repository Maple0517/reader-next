<template>
  <article :class="['v4-metric-card', `tone-${tone ?? 'neutral'}`]">
    <span class="v4-metric-card-label">{{ label }}</span>
    <strong class="v4-metric-card-value">{{ value }}</strong>
    <span v-if="meta" class="v4-metric-card-meta">{{ meta }}</span>
  </article>
</template>

<script setup lang="ts">
defineProps<{
  label: string
  value: string | number
  meta?: string
  tone?: 'neutral' | 'active' | 'warning' | 'danger' | 'success'
}>()
</script>

<style scoped>
.v4-metric-card {
  display: grid;
  gap: 4px;
  min-height: 56px;
  padding: 8px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
}

.v4-metric-card-label,
.v4-metric-card-meta {
  color: var(--v4-muted);
  font-size: 10px;
  font-weight: 750;
}

.v4-metric-card-value {
  color: var(--v4-ink);
  font-size: 17px;
  line-height: 1;
  font-weight: 850;
}

.v4-metric-card.tone-danger .v4-metric-card-value {
  color: var(--v4-danger);
}

.v4-metric-card.tone-warning .v4-metric-card-value {
  color: var(--v4-warn);
}

.v4-metric-card.tone-success .v4-metric-card-value {
  color: var(--v4-success);
}
</style>
