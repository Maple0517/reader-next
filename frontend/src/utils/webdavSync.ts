/**
 * Legado-compatible reading progress sync via WebDAV.
 *
 * Legado stores per-book progress as JSON files under:
 *   /legado/bookProgress/{bookName}_{author}.json
 *
 * This module reads those files, merges with the local bookshelf
 * (chapter index > position wins), and writes progress back after reading.
 */
import {
  getWebdavFileList,
  getWebdavFileText,
  uploadTextToWebdav,
} from '../api/webdav'
import { saveBooks } from '../api/bookshelf'
import type { Book } from '../types'

const PROGRESS_DIR = '/legado/bookProgress'

interface LegadoProgress {
  name: string
  author: string
  durChapterIndex: number
  durChapterPos: number
  durChapterTime: number
  durChapterTitle?: string
}

/**
 * Build a Legado-compatible filename from book name and author.
 * Matches the pattern: {name}_{author}.json
 */
function buildProgressFilename(name: string, author: string): string {
  const safe = (s: string) => s.replace(/[/\\:*?"<>|]/g, '_').trim()
  const n = safe(name || '未知')
  const a = safe(author || '未知')
  return `${n}_${a}.json`
}

/**
 * Parse a Legado progress JSON file content.
 */
function parseProgressJson(raw: string): LegadoProgress | null {
  try {
    const data = JSON.parse(raw)
    if (typeof data.name === 'string' && typeof data.durChapterIndex === 'number') {
      return data as LegadoProgress
    }
    return null
  } catch {
    return null
  }
}

/**
 * Pull reading progress from WebDAV bookProgress/ and merge into bookshelf.
 * For each remote progress entry, if the remote chapter index (then position)
 * is ahead of the local book's, update the local book's progress.
 *
 * Returns the number of books updated.
 */
export async function syncProgressFromWebdav(books: Book[]): Promise<{
  updated: number
  books: Book[]
}> {
  // List files in the progress directory
  let entries
  try {
    entries = await getWebdavFileList(PROGRESS_DIR)
  } catch {
    // Directory doesn't exist yet — no remote progress to sync
    return { updated: 0, books }
  }

  const jsonEntries = entries.filter(
    (e) => !e.isDirectory && e.name.endsWith('.json'),
  )
  if (jsonEntries.length === 0) {
    return { updated: 0, books }
  }

  // Read all progress files
  const remoteProgressList: LegadoProgress[] = []
  await Promise.all(
    jsonEntries.map(async (entry) => {
      try {
        const raw = await getWebdavFileText(entry.path)
        const progress = parseProgressJson(raw)
        if (progress) {
          remoteProgressList.push(progress)
        }
      } catch {
        // Skip unreadable files
      }
    }),
  )

  if (remoteProgressList.length === 0) {
    return { updated: 0, books }
  }

  // Build lookup: "name|author" → remote progress
  const remoteMap = new Map<string, LegadoProgress>()
  for (const p of remoteProgressList) {
    const key = `${p.name}|${p.author}`
    const existing = remoteMap.get(key)
    // Keep the newer one if duplicates
    const pIdx = p.durChapterIndex ?? 0
    const eIdx = existing?.durChapterIndex ?? 0
    const pPos = p.durChapterPos ?? 0
    const ePos = existing?.durChapterPos ?? 0
    if (!existing || pIdx > eIdx || (pIdx === eIdx && pPos > ePos)) {
      remoteMap.set(key, p)
    }
  }

  // Merge: update local books where remote is newer
  let updated = 0
  const updatedBooks = books.map((book) => {
    const key = `${book.name}|${book.author}`
    const remote = remoteMap.get(key)
    if (!remote) return book

    // Remote has more progress — update local book
    // Compare by chapter index first, then position within chapter
    const localIndex = book.durChapterIndex ?? 0
    const remoteIndex = remote.durChapterIndex ?? 0
    const localPos = book.durChapterPos ?? 0
    const remotePos = remote.durChapterPos ?? 0

    const remoteIsNewer = remoteIndex > localIndex
      || (remoteIndex === localIndex && remotePos > localPos)

    if (remoteIsNewer) {
      updated++
      return {
        ...book,
        durChapterIndex: remote.durChapterIndex,
        durChapterPos: remote.durChapterPos,
        durChapterTime: remote.durChapterTime,
        durChapterTitle: remote.durChapterTitle ?? book.durChapterTitle,
      }
    }
    return book
  })

  // Persist updated books to server
  if (updated > 0) {
    // Only save books that actually changed
    const changed = updatedBooks.filter((book, i) => book !== books[i])
    if (changed.length > 0) {
      await saveBooks(changed)
    }
  }

  return { updated, books: updatedBooks }
}

/**
 * Write a single book's reading progress to WebDAV in Legado format.
 * Call this after the user finishes a reading session.
 */
export async function writeProgressToWebdav(book: Book): Promise<void> {
  if (!book.name) return

  const progress: LegadoProgress = {
    name: book.name,
    author: book.author || '',
    durChapterIndex: book.durChapterIndex ?? 0,
    durChapterPos: book.durChapterPos ?? 0,
    durChapterTime: book.durChapterTime ?? Date.now(),
    durChapterTitle: book.durChapterTitle,
  }

  const filename = buildProgressFilename(book.name, book.author || '')
  const content = JSON.stringify(progress, null, 2)

  try {
    await uploadTextToWebdav(content, filename, PROGRESS_DIR)
  } catch (err) {
    console.warn('[webdavSync] Failed to write progress:', err)
  }
}

/**
 * Push all books' current progress to WebDAV.
 * Useful for initial sync or after bulk import.
 */
export async function writeAllProgressToWebdav(books: Book[]): Promise<number> {
  let written = 0
  // Write sequentially to avoid overwhelming the server
  for (const book of books) {
    if (book.durChapterIndex && book.durChapterIndex > 0) {
      await writeProgressToWebdav(book)
      written++
    }
  }
  return written
}
