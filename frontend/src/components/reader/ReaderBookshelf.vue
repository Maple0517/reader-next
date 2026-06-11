<template>
  <div class="reader-bookshelf" :style="{ background: theme.popup, color: theme.fontColor }">
    <div class="shelf-header">
      <h3>书架 ({{ store.books.length }})</h3>
      <button class="close-btn" @click="readerStore.closePanel()">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12" /></svg>
      </button>
    </div>
    
    <div class="shelf-list">
      <div
        v-for="book in store.books"
        :key="book.bookUrl"
        class="shelf-item"
        :class="{ current: book.bookUrl === readerStore.book?.bookUrl }"
        @click="openBook(book)"
      >
        <img v-if="getCoverUrl(book.coverUrl)" :src="getCoverUrl(book.coverUrl)" class="book-cover" />
        <div v-else class="book-cover placeholder">无封面</div>
        
        <div class="book-info">
          <div class="book-title">{{ book.name }}</div>
          <div class="book-author">{{ book.author }}</div>
          <div class="book-progress">
            {{ book.durChapterTitle || '未读' }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useBookshelfStore } from '../../stores/bookshelf'
import { useReaderStore } from '../../stores/reader'
import { getCoverUrl } from '../../api/bookshelf'
import type { Book } from '../../types'
const store = useBookshelfStore()
const readerStore = useReaderStore()
const theme = computed(() => readerStore.currentTheme)

if (!store.books.length) {
  store.fetchBooks()
}

async function openBook(book: Book) {
  if (book.bookUrl !== readerStore.book?.bookUrl) {
    readerStore.clear()
    await readerStore.loadBook(book)
    await readerStore.loadChapter(readerStore.currentIndex)
  }
  readerStore.closePanel()
}
</script>

<style scoped>
.reader-bookshelf {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.shelf-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(0,0,0,0.06);
  flex-shrink: 0;
}

.shelf-header h3 {
  font-size: 16px;
  font-weight: 600;
  margin: 0;
}

.close-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  color: inherit;
  opacity: 0.6;
  background: transparent;
  border: none;
  cursor: pointer;
}

.close-btn:hover {
  opacity: 1;
  background: rgba(0,0,0,0.05);
}

.close-btn svg {
  width: 18px;
  height: 18px;
}

.shelf-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 0;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
}

.shelf-item {
  display: flex;
  gap: 12px;
  padding: 10px 20px;
  cursor: pointer;
  transition: background 0.2s;
}

.shelf-item:hover {
  background: rgba(0,0,0,0.03);
}

.shelf-item.current {
  background: rgba(201, 127, 58, 0.08); /* primary slight tint */
}

.book-cover {
  width: 48px;
  height: 64px;
  object-fit: cover;
  border-radius: 4px;
  flex-shrink: 0;
  box-shadow: 0 2px 6px rgba(0,0,0,0.1);
}

.book-cover.placeholder {
  background: rgba(0,0,0,0.1);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: rgba(0,0,0,0.4);
}

.book-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 2px 0;
}

.book-title {
  font-size: 15px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.book-author {
  font-size: 12px;
  opacity: 0.6;
}

.book-progress {
  font-size: 11px;
  opacity: 0.8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--color-primary, #c97f3a);
}
</style>
