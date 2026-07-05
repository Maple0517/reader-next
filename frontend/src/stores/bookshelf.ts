import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import {
  getBookshelfWithCacheInfo,
  getBookGroups,
  deleteBook as apiDeleteBook,
  deleteBooks as apiDeleteBooks,
  saveBookGroupId as apiSaveBookGroupId,
  saveBookGroup as apiSaveBookGroup,
  deleteBookGroup as apiDeleteBookGroup,
  saveBooks as apiSaveBooks,
} from '../api/bookshelf'
import type { Book, BookGroup, SearchBook } from '../types'
import { deleteBrowserBookCache, listBrowserCacheSummary } from '../utils/browserCache'
import { isLocalTxtBook } from '../utils/localBook'
import { clearRecentReadBooks, getRecentReadBookKey, loadRecentReadBooks, removeRecentReadBook } from '../utils/recentBooks'
import { syncProgressFromWebdav } from '../utils/webdavSync'

type SearchScope = 'all' | 'group' | 'source'

type SearchPreferences = {
  scope: SearchScope
  group: string
  sourceUrl: string
}

type SearchCacheParams = SearchPreferences & {
  key: string
}

type SearchCacheEntry = SearchCacheParams & {
  results: SearchBook[]
  updatedAt: number
}

const SEARCH_PREFERENCES_KEY = 'reader-search-preferences'
const SEARCH_CACHE_TTL = 30 * 60 * 1000
const SEARCH_CACHE_LIMIT = 20

function loadSearchPreferences(): SearchPreferences {
  try {
    const raw = localStorage.getItem(SEARCH_PREFERENCES_KEY)
    const parsed = raw ? JSON.parse(raw) as Partial<SearchPreferences> : {}
    const scope = parsed.scope === 'all' || parsed.scope === 'group' || parsed.scope === 'source'
      ? parsed.scope
      : 'source'
    return {
      scope,
      group: typeof parsed.group === 'string' ? parsed.group : '',
      sourceUrl: typeof parsed.sourceUrl === 'string' ? parsed.sourceUrl : '',
    }
  } catch {
    return { scope: 'source', group: '', sourceUrl: '' }
  }
}

function buildSearchCacheKey(params: SearchCacheParams) {
  return JSON.stringify({
    key: params.key.trim(),
    scope: params.scope,
    group: params.group || '',
    sourceUrl: params.sourceUrl || '',
  })
}

export const useBookshelfStore = defineStore('bookshelf', () => {
  // ─── Bookshelf ───
  const books = ref<Book[]>([])
  const recentBooks = ref<Book[]>([])
  const loading = ref(false)
  const refreshing = ref(false)
  const sorting = ref(false)

  async function refreshRecentBooks() {
    const browserSummaries = await listBrowserCacheSummary().catch(() => [])
    const browserMap = new Map(browserSummaries.map((item) => [item.bookUrl, item.cachedChapterCount]))
    const shelfMap = new Map(books.value.map((book) => [getRecentReadBookKey(book), book]))
    recentBooks.value = loadRecentReadBooks().map((entry) => {
      const shelfBook = shelfMap.get(getRecentReadBookKey(entry))
      const merged = shelfBook
        ? {
            ...entry,
            ...shelfBook,
            recentReadAt: entry.recentReadAt,
            durChapterTime: entry.recentReadAt,
          }
        : entry
      return {
        ...merged,
        browserCachedChapterCount: isLocalTxtBook(merged) ? 0 : browserMap.get(merged.bookUrl) || merged.browserCachedChapterCount || 0,
      }
    })
  }

  async function removeRecentBook(book: Pick<Book, 'bookUrl' | 'origin'>) {
    removeRecentReadBook(book)
    await refreshRecentBooks()
  }

  async function clearAllRecentBooks() {
    clearRecentReadBooks()
    await refreshRecentBooks()
  }

  async function fetchBooks() {
    loading.value = true
    try {
      const [serverBooks, browserSummaries] = await Promise.all([
        getBookshelfWithCacheInfo(),
        listBrowserCacheSummary().catch(() => []),
      ])
      const browserMap = new Map(browserSummaries.map((item) => [item.bookUrl, item.cachedChapterCount]))
      books.value = serverBooks.map((book) => ({
        ...book,
        browserCachedChapterCount: isLocalTxtBook(book) ? 0 : browserMap.get(book.bookUrl) || 0,
      }))
      await refreshRecentBooks()

      // Sync reading progress from WebDAV (Legado compatible, fire-and-forget)
      syncProgressFromWebdav(books.value).then((result) => {
        if (result.updated > 0) {
          books.value = result.books.map((book) => ({
            ...book,
            browserCachedChapterCount: isLocalTxtBook(book) ? 0 : browserMap.get(book.bookUrl) || 0,
          }))
        }
      }).catch(() => undefined)
    } finally {
      loading.value = false
    }
  }

  async function refreshBooks() {
    refreshing.value = true
    try {
      const [serverBooks, browserSummaries] = await Promise.all([
        getBookshelfWithCacheInfo(),
        listBrowserCacheSummary().catch(() => []),
      ])
      const browserMap = new Map(browserSummaries.map((item) => [item.bookUrl, item.cachedChapterCount]))
      books.value = serverBooks.map((book) => ({
        ...book,
        browserCachedChapterCount: isLocalTxtBook(book) ? 0 : browserMap.get(book.bookUrl) || 0,
      }))
      await refreshRecentBooks()
    } finally {
      refreshing.value = false
    }
  }

  async function removeBook(book: Book) {
    await apiDeleteBook(book)
    await deleteBrowserBookCache(book.bookUrl).catch(() => undefined)
    books.value = books.value.filter((b) => b.bookUrl !== book.bookUrl)
    await refreshRecentBooks()
  }

  // ─── Groups ───
  const groups = ref<BookGroup[]>([])
  const activeGroupId = ref<number>(-1) // -1 = all

  const displayGroups = computed(() => {
    const all: BookGroup = { groupId: -1, groupName: '全部' }
    const ungrouped: BookGroup = { groupId: 0, groupName: '未分组' }
    return [all, ...groups.value, ungrouped]
  })

  const filteredBooks = computed(() => {
    if (activeGroupId.value === -1) return books.value
    if (activeGroupId.value === 0) {
      return books.value.filter((b) => !b.group || b.group === 0)
    }
    return books.value.filter(
      (b) => b.group && (b.group & activeGroupId.value) !== 0
    )
  })

  async function fetchGroups() {
    try {
      groups.value = await getBookGroups()
    } catch {
      groups.value = []
    }
  }

  async function saveGroup(groupName: string, groupId = 0) {
    await apiSaveBookGroup({
      groupId,
      groupName,
      orderNo: groups.value.length,
    })
    await fetchGroups()
    return groups.value.find((group) => group.groupName === groupName)?.groupId || groupId
  }

  async function removeGroup(groupId: number) {
    await apiDeleteBookGroup(groupId)
    groups.value = groups.value.filter((group) => group.groupId !== groupId)
    books.value = books.value.map((book) => {
      if (book.group && (book.group & groupId) !== 0) {
        return { ...book, group: book.group & ~groupId }
      }
      return book
    })
  }

  // ─── Search ───
  const searchResults = ref<SearchBook[]>([])
  const isSearching = ref(false)
  const searchKey = ref('')
  const searchPreferences = loadSearchPreferences()
  const searchScope = ref<SearchScope>(searchPreferences.scope)
  const searchGroup = ref('')
  const searchSourceUrl = ref('')
  const searchCache = ref<SearchCacheEntry[]>([])

  function persistSearchPreferences() {
    const preferences: SearchPreferences = {
      scope: searchScope.value,
      group: searchGroup.value,
      sourceUrl: searchSourceUrl.value,
    }
    localStorage.setItem(SEARCH_PREFERENCES_KEY, JSON.stringify(preferences))
  }

  function startSearch(key: string, options: {
    scope?: SearchScope
    group?: string
    sourceUrl?: string
  } = {}) {
    const nextKey = key.trim()
    if (!nextKey) {
      clearSearch()
      return
    }

    const saved = loadSearchPreferences()
    searchScope.value = options.scope || saved.scope
    searchGroup.value = options.group ?? (searchScope.value === 'group' ? saved.group : '')
    searchSourceUrl.value = options.sourceUrl ?? (searchScope.value === 'source' ? saved.sourceUrl : '')
    searchKey.value = nextKey
    persistSearchPreferences()
  }

  function clearSearch() {
    searchResults.value = []
    searchKey.value = ''
    isSearching.value = false
  }

  function getCachedSearchResults(params: SearchCacheParams) {
    const cacheKey = buildSearchCacheKey(params)
    const now = Date.now()
    const entry = searchCache.value.find((item) => buildSearchCacheKey(item) === cacheKey)
    if (!entry) return null
    if (now - entry.updatedAt > SEARCH_CACHE_TTL) {
      searchCache.value = searchCache.value.filter((item) => item !== entry)
      return null
    }
    return entry.results.slice()
  }

  function cacheSearchResults(params: SearchCacheParams & { results: SearchBook[] }) {
    const entry: SearchCacheEntry = {
      key: params.key.trim(),
      scope: params.scope,
      group: params.group || '',
      sourceUrl: params.sourceUrl || '',
      results: params.results.slice(),
      updatedAt: Date.now(),
    }
    const cacheKey = buildSearchCacheKey(entry)
    searchCache.value = [
      entry,
      ...searchCache.value.filter((item) => buildSearchCacheKey(item) !== cacheKey),
    ].slice(0, SEARCH_CACHE_LIMIT)
  }

  const isSearchMode = computed(() => searchKey.value.length > 0)

  // ─── Edit mode and Selection ───
  const editMode = ref(false)
  const selectedBookUrls = ref<Set<string>>(new Set())

  function toggleSelection(url: string) {
    if (selectedBookUrls.value.has(url)) {
      selectedBookUrls.value.delete(url)
    } else {
      selectedBookUrls.value.add(url)
    }
  }

  function selectAll() {
    filteredBooks.value.forEach(b => selectedBookUrls.value.add(b.bookUrl))
  }

  function clearSelection() {
    selectedBookUrls.value.clear()
  }

  async function bulkDelete() {
    const toDelete = books.value
      .filter(b => selectedBookUrls.value.has(b.bookUrl))
      .map(b => ({ bookUrl: b.bookUrl, origin: b.origin }))
    
    if (toDelete.length === 0) return
    await apiDeleteBooks(toDelete as Book[])
    await Promise.all(toDelete.map((book) => deleteBrowserBookCache(book.bookUrl).catch(() => undefined)))
    books.value = books.value.filter(b => !selectedBookUrls.value.has(b.bookUrl))
    clearSelection()
  }

  async function bulkSetGroup(groupId: number) {
    const urls = Array.from(selectedBookUrls.value)
    for (const url of urls) {
      await apiSaveBookGroupId(url, groupId)
    }
    // Refresh to get updated groups
    await fetchBooks()
    clearSelection()
  }

  async function reorderBooks(draggedUrl: string, targetUrl: string) {
    if (!draggedUrl || !targetUrl || draggedUrl === targetUrl) return

    const snapshot = books.value.slice()
    const fromIndex = snapshot.findIndex((book) => book.bookUrl === draggedUrl)
    const toIndex = snapshot.findIndex((book) => book.bookUrl === targetUrl)
    if (fromIndex === -1 || toIndex === -1 || fromIndex === toIndex) return

    const next = snapshot.slice()
    const [moved] = next.splice(fromIndex, 1)
    next.splice(toIndex, 0, moved)

    books.value = next
    sorting.value = true
    try {
      await apiSaveBooks(next)
    } catch (error) {
      books.value = snapshot
      throw error
    } finally {
      sorting.value = false
    }
  }

  async function moveBookToFront(bookUrl: string) {
    if (!bookUrl || books.value.length <= 1) return

    const snapshot = books.value.slice()
    const fromIndex = snapshot.findIndex((book) => book.bookUrl === bookUrl)
    if (fromIndex <= 0) return

    const next = snapshot.slice()
    const [moved] = next.splice(fromIndex, 1)
    next.unshift(moved)

    books.value = next
    sorting.value = true
    try {
      await apiSaveBooks(next)
    } catch (error) {
      books.value = snapshot
      throw error
    } finally {
      sorting.value = false
    }
  }

  return {
    books, recentBooks, loading, refreshing, sorting,
    fetchBooks, refreshBooks, removeBook,
    refreshRecentBooks, removeRecentBook, clearAllRecentBooks,
    groups, activeGroupId, displayGroups, filteredBooks,
    fetchGroups, saveGroup, removeGroup,
    searchResults, isSearching, searchKey,
    searchScope, searchGroup, searchSourceUrl, startSearch, clearSearch, isSearchMode,
    persistSearchPreferences, getCachedSearchResults, cacheSearchResults,
    editMode,
    selectedBookUrls, toggleSelection, selectAll, clearSelection,
    bulkDelete, bulkSetGroup, reorderBooks, moveBookToFront,
  }
})
