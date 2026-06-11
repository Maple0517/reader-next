<template>
  <div class="explore-view" :style="{ '--color-primary': '#c97f3a' }">
    <div class="explore-header">
      <div class="header-left">
        <h2>发现书海</h2>
        <!-- 顶部：书源切换 -->
        <div class="source-selector">
          <select :value="store.activeSourceUrl" @change="onSourceChange" v-if="store.exploreSources.length > 0">
            <option
              v-for="src in store.exploreSources"
              :key="src.bookSourceUrl"
              :value="src.bookSourceUrl"
            >
              {{ src.bookSourceName }}
            </option>
          </select>
          <span v-else class="no-sources-text">无带有发现规则的书源</span>
        </div>
      </div>
    </div>

    <div class="explore-body">
      <!-- 桌面端侧边分类 / 移动端顶部分类滑动条 -->
      <div class="categories-panel">
        <div class="categories-scroll">
          <div
            v-for="(cat, index) in store.categories"
            :key="getExploreCategoryKey(cat, index)"
            class="category-tag"
            :class="{
              active: !isExploreCategorySection(cat) && store.activeCategoryUrl === cat.url,
              section: isExploreCategorySection(cat),
            }"
            @click="handleCategoryClick(cat)"
          >
            {{ cat.title }}
          </div>
        </div>
      </div>

      <!-- 书籍列表区 -->
      <div class="content-panel" ref="scrollContainer" @scroll="handleScroll">
        <div class="books-grid-wrapper" v-if="store.books.length > 0">
          <BookGrid
            :books="store.books"
            :is-search="true"
            empty-text="暂无数据"
            @click="handleBookClick"
            @addToShelf="handleAddToShelf"
          />
        </div>
        
        <div class="loading-state" v-if="store.loading">
          <div class="spinner"></div>加载中...
        </div>
        
        <div class="end-state" v-else-if="!store.hasMore && store.books.length > 0">
          没有更多了
        </div>

        <div class="error-state" v-else-if="store.error">
          {{ store.error }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useExploreStore } from '../stores/explore'
import { useReaderStore } from '../stores/reader'
import { saveBook } from '../api/bookshelf'
import { useAppStore } from '../stores/app'
import BookGrid from '../components/BookGrid.vue'
import type { Book, SearchBook } from '../types'
import {
  getExploreCategoryKey,
  isExploreCategorySection,
  type ExploreCategory,
} from '../utils/exploreCategories'

const store = useExploreStore()
const readerStore = useReaderStore()
const appStore = useAppStore()
const router = useRouter()

const scrollContainer = ref<HTMLElement>()
const openingBookUrl = ref('')

onMounted(async () => {
  await store.init()
})

function onSourceChange(event: Event) {
  store.setSource((event.target as HTMLSelectElement).value)
}

function handleScroll() {
  const el = scrollContainer.value
  if (!el) return
  const { scrollTop, scrollHeight, clientHeight } = el
  // 触底 100px 触发加载
  if (scrollTop + clientHeight >= scrollHeight - 100) {
    store.fetchMore()
  }
}

function handleCategoryClick(category: ExploreCategory) {
  if (isExploreCategorySection(category)) return
  store.setCategory(category.url)
}

async function handleBookClick(book: Book | SearchBook) {
  const b = book as Book
  if (!b.origin || !b.bookUrl) return
  if (openingBookUrl.value === b.bookUrl) return

  openingBookUrl.value = b.bookUrl

  try {
    const loadBookTask = readerStore.loadBook(b)
    await router.push('/reader')
    await loadBookTask
    await readerStore.loadChapter(readerStore.currentIndex)
  } finally {
    openingBookUrl.value = ''
  }
}

async function handleAddToShelf(book: Book | SearchBook) {
  try {
    await saveBook({
      name: book.name,
      author: book.author,
      bookUrl: book.bookUrl,
      origin: book.origin,
      coverUrl: book.coverUrl,
    })
    appStore.showToast(`"${book.name}" 已加入书架`, 'success')
  } catch (e: unknown) {
    appStore.showToast((e as Error).message, 'error')
  }
}
</script>

<style scoped>
.explore-view {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--color-bg);
  overflow: hidden;
}

.explore-header {
  padding: 16px 24px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-bg-elevated);
}

.header-left h2 {
  font-size: 20px;
  font-weight: 700;
  margin: 0 0 8px 0;
  color: var(--color-text);
}

.source-selector select {
  padding: 6px 12px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-bg);
  color: var(--color-text);
  font-size: 14px;
  outline: none;
  cursor: pointer;
}

.no-sources-text {
  font-size: 13px;
  color: var(--color-text-tertiary);
}

.explore-body {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* 侧边栏模式 (PC端) */
.categories-panel {
  width: 200px;
  border-right: 1px solid var(--color-border);
  background: var(--color-bg-elevated);
  overflow: auto;
  min-height: 0;
}

.categories-scroll {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.category-tag {
  padding: 10px 16px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 14px;
  color: var(--color-text-secondary);
  transition: all 0.2s;
}

.category-tag.section {
  cursor: default;
  color: var(--color-text-tertiary);
}

.category-tag:hover:not(.section) {
  background: var(--color-bg-hover);
}

.category-tag.active {
  background: rgba(201, 127, 58, 0.1);
  color: var(--color-primary);
  font-weight: 600;
}

.content-panel {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 24px;
  position: relative;
}

.books-grid-wrapper {
  margin-bottom: 24px;
}

.loading-state, .end-state, .error-state {
  text-align: center;
  padding: 20px 0;
  color: var(--color-text-tertiary);
  font-size: 14px;
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 8px;
}

.error-state {
  color: #ef4444;
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 移动端适配 */
@media (max-width: 768px) {
  .explore-body {
    flex-direction: column;
  }
  
  .explore-header {
    padding: 12px 16px;
  }

  .categories-panel {
    width: 100%;
    border-right: none;
    border-bottom: 1px solid var(--color-border);
    overflow-x: auto;
    overflow-y: hidden;
    flex-shrink: 0;
  }

  .categories-scroll {
    flex-direction: row;
    padding: 8px 16px;
    gap: 8px;
  }

  .category-tag {
    white-space: nowrap;
    padding: 6px 14px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: 20px;
  }
  
  .category-tag.active {
    background: var(--color-primary);
    color: white;
    border-color: var(--color-primary);
  }

  .content-panel {
    padding: 16px;
  }
}
</style>
