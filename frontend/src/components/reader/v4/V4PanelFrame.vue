<template>
  <section class="v4-panel-frame">
    <div class="v4-panel-inner">
      <header class="v4-panel-header">
        <div>
          <h2>{{ title }}</h2>
          <p v-if="subtitle">{{ subtitle }}</p>
        </div>
        <div v-if="$slots.action" class="v4-panel-action">
          <slot name="action" />
        </div>
      </header>
      <div class="v4-panel-body">
        <slot />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  title: string
  subtitle?: string
}>()
</script>

<style scoped>
.v4-panel-frame {
  padding: 5px;
  border-radius: 22px;
  background: color-mix(in srgb, var(--color-bg-sunken) 82%, transparent);
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, #fff 24%, transparent),
    0 14px 45px color-mix(in srgb, var(--color-text) 7%, transparent);
}

.v4-panel-inner {
  min-height: 100%;
  padding: clamp(16px, 2.2vw, 22px);
  border-radius: 17px;
  background: color-mix(in srgb, var(--color-bg) 94%, var(--color-primary) 6%);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 68%, transparent);
}

.v4-panel-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
}

.v4-panel-header h2,
.v4-panel-header p {
  margin: 0;
}

.v4-panel-header h2 {
  color: var(--color-text);
  font-size: 20px;
  letter-spacing: -0.02em;
}

.v4-panel-header p {
  margin-top: 5px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.v4-panel-action {
  flex: 0 0 auto;
}

.v4-panel-body {
  margin-top: 16px;
}

@media (max-width: 768px) {
  .v4-panel-header {
    flex-direction: column;
  }
}
</style>
