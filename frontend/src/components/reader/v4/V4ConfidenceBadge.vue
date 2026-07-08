<template>
  <span :class="['v4-confidence-badge', `tier-${tier}`]">
    {{ label }}
    <span v-if="showValue && formattedValue" class="v4-confidence-badge-value">{{ formattedValue }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  value?: number | null
  showValue?: boolean
}>()

const tier = computed(() => {
  if (typeof props.value !== 'number' || Number.isNaN(props.value)) return 'unknown'
  if (props.value >= 0.75) return 'high'
  if (props.value >= 0.45) return 'medium'
  return 'low'
})

const label = computed(() => {
  if (tier.value === 'high') return '高可信'
  if (tier.value === 'medium') return '中可信'
  if (tier.value === 'low') return '低可信'
  return '未知可信度'
})

const formattedValue = computed(() => {
  if (typeof props.value !== 'number' || Number.isNaN(props.value)) return ''
  return `${Math.round(props.value * 100)}%`
})
</script>

<style scoped>
.v4-confidence-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 24px;
  padding: 0 9px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-bg-sunken) 82%, transparent);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 850;
}

.v4-confidence-badge.tier-high {
  background: color-mix(in srgb, var(--color-success) 13%, var(--color-bg-sunken));
  color: var(--color-success);
}

.v4-confidence-badge.tier-medium {
  background: color-mix(in srgb, var(--color-warning) 13%, var(--color-bg-sunken));
  color: var(--color-warning);
}

.v4-confidence-badge.tier-low {
  background: color-mix(in srgb, var(--color-danger) 11%, var(--color-bg-sunken));
  color: var(--color-danger);
}

.v4-confidence-badge-value {
  color: var(--color-text-tertiary);
}
</style>
