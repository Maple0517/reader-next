<template>
  <div class="reader-toolbar" :style="{ background: theme.popup, color: theme.fontColor }">
    <button class="tb-btn" @click="$emit('bookmark')" title="书签">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z" /></svg>
    </button>
    <button class="tb-btn" @click="$emit('search')" title="搜索">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" /></svg>
    </button>
    <button class="tb-btn" @click="$emit('info')" title="书籍信息">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10" /><path d="M12 16v-4M12 8h.01" /></svg>
    </button>
    <button class="tb-btn" @click="$emit('ai')" title="AI资料">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M6 3h8l4 4v14H6z" />
        <path d="M14 3v5h5" />
        <path d="m8 16 2-5 2 5" />
        <path d="M8.7 14h2.6" />
        <path d="M15 11v5" />
      </svg>
    </button>
    <button class="tb-btn" :class="{ active: showChapterSummary }" @click="$emit('toggleChapterSummary')" :title="showChapterSummary ? '隐藏摘要' : '显示摘要'">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 5h16M4 12h10M4 19h16" /></svg>
    </button>
    <button class="tb-btn" :class="{ spinning: store.loading }" @click="store.refreshContent()" title="刷新">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /><path d="M3 3v5h5" /><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" /><path d="M16 16h5v5" /></svg>
    </button>
    <button class="tb-btn" :class="{ active: store.isAutoScrolling }" @click="store.toggleAutoReading()" title="自动翻页">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" /><circle cx="12" cy="12" r="3" /></svg>
    </button>
    <button class="tb-btn" :class="{ active: isSpeaking }" @click="$emit('tts')" :title="isSpeaking ? (isPaused ? '恢复' : '暂停') : '听书'">
      <svg v-if="!isSpeaking" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 18v-6a9 9 0 0 1 18 0v6" /><path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z" /></svg>
      <svg v-else-if="isPaused" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m5 3 14 9-14 9V3z" /></svg>
      <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="6" y="4" width="4" height="16" /><rect x="14" y="4" width="4" height="16" /></svg>
    </button>
    <button class="tb-btn" @click="store.toggleNight()" :title="store.isNight ? '日间模式' : '夜间模式'">
      <svg v-if="!store.isNight" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" /></svg>
      <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" /></svg>
    </button>

    <div class="tb-divider"></div>

    <button class="progress-text progress-btn" @click="$emit('progress')" title="缓存与进度">
      {{ store.readingProgress }}
    </button>

    <button class="tb-btn nav" :disabled="!store.hasPrev" @click="$emit('prev')" title="上一章">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m15 18-6-6 6-6" /></svg>
    </button>
    <button class="tb-btn nav" :disabled="!store.hasNext" @click="$emit('next')" title="下一章">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m9 18 6-6-6-6" /></svg>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReaderStore } from '../../stores/reader'
import { useAppStore } from '../../stores/app'

const store = useReaderStore()
const appStore = useAppStore()
const theme = computed(() => {
  if (store.isNight || appStore.theme === 'dark') {
    return {
      ...store.currentTheme,
      popup: 'var(--color-bg-elevated)',
    }
  }
  return store.currentTheme
})

defineProps<{
  isSpeaking?: boolean
  isPaused?: boolean
  showChapterSummary?: boolean
}>()

defineEmits<{
  bookmark: []
  search: []
  info: []
  ai: []
  toggleChapterSummary: []
  tts: []
  prev: []
  next: []
  progress: []
}>()
</script>

<style scoped>
.reader-toolbar {
  position: fixed;
  right: calc(16px + var(--reader-summary-sider-width, 0px));
  top: 50%;
  transform: translateY(-50%);
  z-index: 20;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 8px 4px;
  border-radius: 24px;
  box-shadow: 0 2px 12px rgba(0,0,0,0.1);
  border: 1px solid rgba(0,0,0,0.06);
  transition: background 0.3s;
}

.tb-btn {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  color: inherit;
  opacity: 0.55;
  transition: all 0.2s;
  flex-shrink: 0;
}

.tb-btn svg {
  width: 18px;
  height: 18px;
}

.tb-btn:hover {
  opacity: 0.9;
  background: rgba(0,0,0,0.06);
}

.tb-btn:active {
  transform: scale(0.92);
}

.tb-btn.active {
  opacity: 1;
  color: var(--color-primary, #c97f3a);
}

.tb-btn.spinning svg {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.tb-btn:disabled {
  opacity: 0.25;
  cursor: not-allowed;
}

.tb-btn.nav {
  opacity: 0.7;
}

.tb-divider {
  width: 20px;
  height: 1px;
  background: rgba(0,0,0,0.1);
  margin: 4px 0;
}

.progress-text {
  font-size: 10px;
  opacity: 0.5;
  padding: 4px 0;
  font-variant-numeric: tabular-nums;
}

.progress-btn {
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.progress-btn:hover {
  opacity: 0.9;
  color: var(--color-primary, #c97f3a);
}

@media (max-width: 768px) {
  .reader-toolbar {
    right: 8px;
    padding: 6px 3px;
  }
  .tb-btn {
    width: 32px;
    height: 32px;
  }
  .tb-btn svg {
    width: 16px;
    height: 16px;
  }
}
</style>
