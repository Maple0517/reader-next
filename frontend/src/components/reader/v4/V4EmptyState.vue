<template>
  <div class="v4-empty-state">
    <div>
      <h3>{{ title }}</h3>
      <p>{{ message }}</p>
    </div>
    <button v-if="actionLabel" type="button" @click="onAction?.()">{{ actionLabel }}</button>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string
  message: string
  actionLabel?: string
  onAction?: () => void
}>()
</script>

<style scoped>
.v4-empty-state {
  display: flex;
  justify-content: space-between;
  gap: 14px;
  align-items: center;
  padding: 18px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-bg-sunken) 80%, transparent);
  color: var(--color-text);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 68%, transparent);
}

.v4-empty-state h3,
.v4-empty-state p {
  margin: 0;
}

.v4-empty-state h3 {
  font-size: 15px;
}

.v4-empty-state p {
  margin-top: 4px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.v4-empty-state button {
  min-height: 34px;
  padding: 0 14px;
  border: 0;
  border-radius: 999px;
  background: var(--color-primary);
  color: #fff;
  font-weight: 800;
  cursor: pointer;
  transition: transform 220ms cubic-bezier(0.32, 0.72, 0, 1);
}

.v4-empty-state button:active {
  transform: translateY(1px) scale(0.98);
}

@media (max-width: 768px) {
  .v4-empty-state {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>
