import http from './http'
import type { Book, BookChapter, BookGroup } from '../types'

export function getBookshelf() {
  return http.get<Book[]>('/getBookshelf').then((r) => r.data)
}

export function getBookshelfWithCacheInfo() {
  return http.get<Book[]>('/getShelfBookWithCacheInfo').then((r) => r.data)
}

export function getShelfBook(url: string) {
  return http.post<Book>('/getShelfBook', { url }).then((r) => r.data)
}

export function saveBook(book: Partial<Book>) {
  return http.post<Book>('/saveBook', book).then((r) => r.data)
}

export function saveBooks(books: Partial<Book>[]) {
  return http.post<Book[]>('/saveBooks', books).then((r) => r.data)
}

export function uploadTxtBook(file: File) {
  const formData = new FormData()
  formData.append('file', file)
  return http.post<Book>('/uploadTxtBook', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  }).then((r) => r.data)
}

export function deleteBook(book: Partial<Book>) {
  return http.post<string>('/deleteBook', book).then((r) => r.data)
}

export function deleteBooks(books: Partial<Book>[]) {
  return http.post<{ deleted: number }>('/deleteBooks', books).then((r) => r.data)
}

export function getBookInfo(url: string, origin?: string) {
  return http.post<Book>('/getBookInfo', { url, bookSourceUrl: origin }).then((r) => r.data)
}

export function getChapterList(params: {
  bookUrl?: string
  tocUrl?: string
  bookSourceUrl?: string
  refresh?: number
}) {
  return http.post<BookChapter[]>('/getChapterList', params).then((r) => r.data)
}

export function getBookContent(params: {
  chapterUrl?: string
  bookSourceUrl?: string
  index?: number
  refresh?: number
}) {
  return http.post<string>('/getBookContent', params).then((r) => r.data)
}

export function saveBookProgress(params: {
  bookUrl: string
  index: number
  position?: number
}) {
  return http.post<string>('/saveBookProgress', params).then((r) => r.data)
}

export function deleteBookCache(bookUrl: string) {
  return http.post('/deleteBookCache', { bookUrl }).then((r) => r.data)
}

// ─── Groups ───
export function getBookGroups() {
  return http.get<BookGroup[]>('/getBookGroups').then((r) => r.data)
}

export function saveBookGroup(group: BookGroup) {
  return http.post<string>('/saveBookGroup', group).then((r) => r.data)
}

export function deleteBookGroup(groupId: number) {
  return http.post<string>('/deleteBookGroup', { groupId }).then((r) => r.data)
}

export function saveBookGroupId(bookUrl: string, groupId: number) {
  return http.post<string>('/saveBookGroupId', { bookUrl, groupId }).then((r) => r.data)
}

export function setBookSource(params: {
  bookUrl: string
  newUrl: string
  bookSourceUrl: string
}) {
  return http.post<Book>('/setBookSource', params).then((r) => r.data)
}

// ─── Cover helper ───
export function getCoverUrl(coverUrl?: string) {
  if (!coverUrl) return ''
  if (coverUrl.startsWith('http') || coverUrl.startsWith('/')) {
    return `/reader3/cover?path=${encodeURIComponent(coverUrl)}`
  }
  return coverUrl
}
