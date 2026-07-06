<template>
  <div class="v4-error-state" role="alert">
    <div>
      <h3>{{ title }}</h3>
      <p>{{ message }}</p>
    </div>
    <button v-if="retryLabel" type="button" @click="onRetry?.()">{{ retryLabel }}</button>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string
  message: string
  retryLabel?: string
  onRetry?: () => void
}>()
</script>

<style scoped>
.v4-error-state {
  display: flex;
  justify-content: space-between;
  gap: 14px;
  align-items: center;
  padding: 16px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-danger) 9%, var(--color-bg));
  color: var(--color-text);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-danger) 24%, transparent);
}

.v4-error-state h3,
.v4-error-state p {
  margin: 0;
}

.v4-error-state h3 {
  color: var(--color-danger);
  font-size: 15px;
}

.v4-error-state p {
  margin-top: 4px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.v4-error-state button {
  min-height: 34px;
  padding: 0 14px;
  border: 0;
  border-radius: 999px;
  background: var(--color-danger);
  color: #fff;
  font-weight: 800;
  cursor: pointer;
  transition: transform 220ms cubic-bezier(0.32, 0.72, 0, 1);
}

.v4-error-state button:active {
  transform: translateY(1px) scale(0.98);
}

@media (max-width: 768px) {
  .v4-error-state {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>
