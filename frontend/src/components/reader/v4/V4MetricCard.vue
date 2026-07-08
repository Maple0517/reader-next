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
  gap: 6px;
  min-height: 76px;
  padding: 14px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--color-bg-sunken) 82%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 68%, transparent);
}

.v4-metric-card-label,
.v4-metric-card-meta {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 750;
}

.v4-metric-card-value {
  color: var(--color-text);
  font-size: 20px;
  line-height: 1.1;
}

.v4-metric-card.tone-danger .v4-metric-card-value {
  color: var(--color-danger);
}

.v4-metric-card.tone-warning .v4-metric-card-value {
  color: var(--color-warning);
}

.v4-metric-card.tone-success .v4-metric-card-value {
  color: var(--color-success);
}
</style>
