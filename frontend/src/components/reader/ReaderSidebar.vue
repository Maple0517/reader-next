<template>
  <nav class="reader-sidebar" :style="{ background: theme.popup, color: theme.fontColor }">
    <div class="sidebar-items">
      <div class="sidebar-item" :class="{ active: store.activePanel === 'bookshelf' }" @click="store.togglePanel('bookshelf')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" /></svg>
        <span>书架</span>
      </div>
      <div class="sidebar-item" :class="{ active: store.activePanel === 'source' }" @click="store.togglePanel('source')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect width="7" height="7" x="3" y="3" rx="1" /><rect width="7" height="7" x="14" y="3" rx="1" /><rect width="7" height="7" x="3" y="14" rx="1" /><rect width="7" height="7" x="14" y="14" rx="1" /></svg>
        <span>书源</span>
      </div>
      <div class="sidebar-item" :class="{ active: store.activePanel === 'catalog' }" @click="store.togglePanel('catalog')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 12h18M3 6h18M3 18h18" /></svg>
        <span>目录</span>
      </div>
      <div class="sidebar-item" :class="{ active: store.activePanel === 'settings' }" @click="store.togglePanel('settings')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" /><circle cx="12" cy="12" r="3" /></svg>
        <span>设置</span>
      </div>

      <div
        class="sidebar-item"
        :class="{ active: chapterSummaryAuto }"
        :title="chapterSummaryAuto ? '关闭自动摘要' : '开启自动摘要'"
        @click="$emit('toggleChapterSummaryAuto')"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 5h16M4 12h10M4 19h16" /><path d="m17 9 3 3-3 3" /></svg>
        <span>摘要</span>
      </div>
    </div>

    <div class="sidebar-items bottom">
      <div class="sidebar-item" @click="$emit('goHome')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m15 18-6-6 6-6" /></svg>
        <span>首页</span>
      </div>
      <div class="sidebar-item" @click="$emit('scrollTop')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m18 15-6-6-6 6" /></svg>
        <span>顶部</span>
      </div>
      <div class="sidebar-item" @click="$emit('scrollBottom')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m6 9 6 6 6-6" /></svg>
        <span>底部</span>
      </div>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReaderStore } from '../../stores/reader'
import { useAppStore } from '../../stores/app'

const store = useReaderStore()
const appStore = useAppStore()

withDefaults(defineProps<{
  chapterSummaryAuto?: boolean
}>(), {
  chapterSummaryAuto: false,
})

const theme = computed(() => {
  if (store.isNight || appStore.theme === 'dark') {
    return {
      ...store.currentTheme,
      popup: 'var(--color-bg-elevated)',
    }
  }
  return store.currentTheme
})

defineEmits<{
  goHome: []
  scrollTop: []
  scrollBottom: []
  toggleChapterSummaryAuto: []
}>()
</script>

<style scoped>
.reader-sidebar {
  position: fixed;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 64px;
  z-index: 20;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 32px;
  padding: 24px 0;
  border-radius: 0 16px 16px 0;
  box-shadow: 2px 0 16px rgba(0,0,0,0.06);
  border: 1px solid rgba(0,0,0,0.06);
  border-left: none;
  transition: background 0.3s;
}

.sidebar-items {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.sidebar-items.bottom {
  gap: 4px;
}

.sidebar-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 10px 6px;
  cursor: pointer;
  border-radius: 8px;
  transition: all 0.2s;
  color: inherit;
  opacity: 0.6;
  width: 48px;
}

.sidebar-item svg {
  width: 20px;
  height: 20px;
}

.sidebar-item span {
  font-size: 10px;
  font-weight: 500;
  white-space: nowrap;
}

.sidebar-item:hover {
  opacity: 0.9;
  background: rgba(0,0,0,0.05);
}

.sidebar-item.active {
  opacity: 1;
  color: var(--color-primary, #c97f3a);
}


</style>
