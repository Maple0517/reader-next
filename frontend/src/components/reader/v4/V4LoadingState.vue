<template>
  <div class="v4-loading-state" role="status" :aria-label="title">
    <div class="v4-loading-copy">
      <h3>{{ title }}</h3>
      <p v-if="message">{{ message }}</p>
    </div>
    <div class="v4-loading-lines" aria-hidden="true">
      <span v-for="index in rows" :key="index"></span>
    </div>
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string
    message?: string
    rows?: number
  }>(),
  {
    title: '正在加载',
    message: '',
    rows: 3,
  },
)
</script>

<style scoped>
.v4-loading-state {
  display: grid;
  gap: 14px;
  padding: 18px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--color-bg-sunken) 80%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 64%, transparent);
}

.v4-loading-copy h3,
.v4-loading-copy p {
  margin: 0;
}

.v4-loading-copy h3 {
  color: var(--color-text);
  font-size: 15px;
}

.v4-loading-copy p {
  margin-top: 4px;
  color: var(--color-text-secondary);
}

.v4-loading-lines {
  display: grid;
  gap: 9px;
}

.v4-loading-lines span {
  height: 12px;
  border-radius: 999px;
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--color-border) 50%, transparent),
    color-mix(in srgb, var(--color-primary) 18%, transparent),
    color-mix(in srgb, var(--color-border) 50%, transparent)
  );
  background-size: 180% 100%;
}

.v4-loading-lines span:nth-child(2) {
  width: 82%;
}

.v4-loading-lines span:nth-child(3) {
  width: 64%;
}

@media (prefers-reduced-motion: no-preference) {
  .v4-loading-lines span {
    animation: v4-loading-sheen 1.4s cubic-bezier(0.32, 0.72, 0, 1) infinite;
  }
}

@keyframes v4-loading-sheen {
  from {
    background-position: 120% 0;
  }

  to {
    background-position: -80% 0;
  }
}
</style>
