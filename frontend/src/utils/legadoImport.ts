/**
 * Legado (阅读 App) backup ZIP importer.
 *
 * Parses a Legado WebDAV backup .zip file and converts its contents
 * into reader-next's native data format, ready to call existing save APIs.
 *
 * Supported Legado files inside the ZIP:
 *   bookshelf.json   → Book[]        (with field mapping + filtering)
 *   bookSource.json  → BookSource[]  (near-direct mapping)
 *   bookGroup.json   → BookGroup[]   (groupId remapping)
 *   bookmark.json    → Bookmark[]
 *   replaceRule.json → ReplaceRule[]
 *   rssSources.json  → RssSource[]
 */
import JSZip from 'jszip'
import type { Book, BookGroup, BookSource, Bookmark, ReplaceRule, RssSource } from '../types'

// ─── Legado raw types (subset of actual fields) ───

interface LegadoBook {
  name: string
  author: string
  bookUrl: string
  origin: string
  originName?: string
  coverUrl?: string
  tocUrl?: string
  charset?: string
  customCoverUrl?: string
  canUpdate?: boolean
  durChapterIndex?: number
  durChapterPos?: number
  durChapterTime?: number
  durChapterTitle?: string
  group?: number
  lastCheckTime?: number
  latestChapterTitle?: string
  totalChapterNum?: number
  type?: number
  order?: number
  [key: string]: unknown
}

interface LegadoBookSource {
  bookSourceName: string
  bookSourceUrl: string
  bookSourceGroup?: string
  bookSourceType?: number
  bookSourceComment?: string
  enabled?: boolean
  enabledExplore?: boolean
  enabledCookieJar?: boolean
  customOrder?: number
  weight?: number
  searchUrl?: string
  exploreUrl?: string
  loginUrl?: string
  loginUi?: string
  jsLib?: string
  header?: string
  bookUrlPattern?: string
  ruleSearch?: Record<string, unknown>
  ruleExplore?: Record<string, unknown>
  ruleBookInfo?: Record<string, unknown>
  ruleToc?: Record<string, unknown>
  ruleContent?: Record<string, unknown>
  [key: string]: unknown
}

interface LegadoBookGroup {
  groupId: number
  groupName: string
  order?: number
  show?: boolean
  enableRefresh?: boolean
  bookSort?: number
  isPrivate?: boolean
}

interface LegadoBookmark {
  bookUrl?: string
  bookName: string
  bookAuthor?: string
  chapterIndex?: number
  chapterPos?: number
  chapterName?: string
  bookText?: string
  content?: string
  time?: number
}

interface LegadoReplaceRule {
  id?: number
  name: string
  group?: string
  pattern: string
  replacement: string
  scopeContent?: boolean
  scopeTitle?: boolean
  isEnabled?: boolean
  isRegex?: boolean
  order?: number
  timeoutMillisecond?: number
}

interface LegadoRssSource {
  sourceUrl: string
  sourceName: string
  sourceIcon?: string
  sourceGroup?: string
  sourceComment?: string
  enabled?: boolean
  enabledCookieJar?: boolean
  concurrentRate?: string
  header?: string
  loginUrl?: string
  loginCheckJs?: string
  sortUrl?: string
  singleUrl?: boolean
  articleStyle?: number
  ruleArticles?: string
  ruleNextPage?: string
  ruleTitle?: string
  rulePubDate?: string
  ruleDescription?: string
  ruleImage?: string
  ruleLink?: string
  ruleContent?: string
  style?: string
  enableJs?: boolean
  loadWithBaseUrl?: boolean
  customOrder?: number
  lastUpdateTime?: number
}

// ─── Import result types ───

export interface LegadoImportResult {
  books: Book[]
  skippedBooks: number
  incompleteBooks: number  // data: URI decoded books with no resolvable bookUrl
  bookSources: BookSource[]
  bookGroups: BookGroup[]
  bookmarks: Bookmark[]
  replaceRules: ReplaceRule[]
  rssSources: RssSource[]
}

// ─── Core import logic ───

function isUsableBookUrl(url: string): boolean {
  if (!url) return false
  // content:// URIs are Android local files — no file content in backup
  if (url.startsWith('content://')) return false
  // data: URIs need source resolution that may not be available
  if (url.startsWith('data:')) return false
  return true
}

/**
 * Decode a data: URI book (晴天融合VIP4.0 style inline book references).
 * Returns a usable book entry with name/author only, or null if unresolvable.
 */
function decodeDataUriBook(legado: LegadoBook): Book | null {
  try {
    const url = legado.bookUrl
    const b64 = url.split(',', 2)[1]
    if (!b64) return null
    const json = JSON.parse(atob(b64))
    // data: URI books have { book_id, sources, tab, url }
    // Use the book name + author as a searchable reference
    return {
      name: legado.name ?? '',
      author: legado.author ?? '',
      bookUrl: '',  // no resolvable URL
      origin: legado.origin ?? json.sources ?? '',
      originName: legado.originName ?? json.sources,
      coverUrl: legado.coverUrl,
      customCoverUrl: legado.customCoverUrl,
      canUpdate: false,  // can't auto-update without source
      durChapterIndex: legado.durChapterIndex ?? 0,
      durChapterPos: legado.durChapterPos ?? 0,
      durChapterTime: legado.durChapterTime,
      durChapterTitle: legado.durChapterTitle,
      latestChapterTitle: legado.latestChapterTitle,
      lastCheckTime: legado.lastCheckTime,
      totalChapterNum: legado.totalChapterNum,
      type: legado.type,
      group: legado.group,
    }
  } catch {
    return null
  }
}

function convertBook(legado: LegadoBook): Book {
  return {
    name: legado.name ?? '',
    author: legado.author ?? '',
    bookUrl: legado.bookUrl,
    origin: legado.origin ?? '',
    originName: legado.originName,
    coverUrl: legado.coverUrl,
    tocUrl: legado.tocUrl,
    charset: legado.charset,
    customCoverUrl: legado.customCoverUrl,
    canUpdate: legado.canUpdate ?? true,
    durChapterIndex: legado.durChapterIndex ?? 0,
    durChapterPos: legado.durChapterPos ?? 0,
    durChapterTime: legado.durChapterTime,
    durChapterTitle: legado.durChapterTitle,
    latestChapterTitle: legado.latestChapterTitle,
    lastCheckTime: legado.lastCheckTime,
    totalChapterNum: legado.totalChapterNum,
    type: legado.type,
    group: legado.group,
  }
}

function convertBookSource(legado: LegadoBookSource): BookSource {
  return {
    bookSourceName: legado.bookSourceName ?? '',
    bookSourceUrl: legado.bookSourceUrl ?? '',
    bookSourceGroup: legado.bookSourceGroup,
    bookSourceType: legado.bookSourceType,
    enabled: legado.enabled ?? true,
    enabledExplore: legado.enabledExplore ?? false,
    enabledCookieJar: legado.enabledCookieJar,
    customOrder: legado.customOrder,
    weight: legado.weight,
    searchUrl: legado.searchUrl,
    exploreUrl: legado.exploreUrl,
    header: legado.header,
    loginUrl: legado.loginUrl,
    ruleSearch: legado.ruleSearch as Record<string, string>,
    ruleExplore: legado.ruleExplore as Record<string, string>,
    ruleBookInfo: legado.ruleBookInfo as Record<string, string>,
    ruleToc: legado.ruleToc as Record<string, string>,
    ruleContent: legado.ruleContent as Record<string, string>,
  }
}

function convertBookGroup(legado: LegadoBookGroup): BookGroup {
  return {
    groupId: legado.groupId,
    groupName: legado.groupName ?? '',
    orderNo: legado.order,
  }
}

function convertBookmark(legado: LegadoBookmark): Bookmark {
  return {
    bookName: legado.bookName ?? '',
    bookAuthor: legado.bookAuthor ?? '',
    chapterIndex: legado.chapterIndex,
    chapterPos: legado.chapterPos,
    chapterName: legado.chapterName,
    bookText: legado.bookText ?? legado.content,
    content: legado.content ?? legado.bookText,
    time: legado.time,
  }
}

function convertReplaceRule(legado: LegadoReplaceRule): ReplaceRule {
  let scope: string | undefined
  if (legado.scopeContent && legado.scopeTitle) {
    scope = 'all'
  } else if (legado.scopeContent) {
    scope = 'content'
  } else if (legado.scopeTitle) {
    scope = 'title'
  }
  return {
    id: legado.id ?? 0,
    name: legado.name ?? '',
    group: legado.group,
    pattern: legado.pattern ?? '',
    replacement: legado.replacement ?? '',
    scope,
    isEnabled: legado.isEnabled ?? true,
    isRegex: legado.isRegex ?? false,
    order: legado.order ?? 0,
  }
}

function convertRssSource(legado: LegadoRssSource): RssSource {
  return {
    sourceUrl: legado.sourceUrl ?? '',
    sourceName: legado.sourceName ?? '',
    sourceIcon: legado.sourceIcon,
    sourceGroup: legado.sourceGroup,
    sourceComment: legado.sourceComment,
    enabled: legado.enabled ?? true,
    enabledCookieJar: legado.enabledCookieJar,
    concurrentRate: legado.concurrentRate,
    header: legado.header,
    loginUrl: legado.loginUrl,
    loginCheckJs: legado.loginCheckJs,
    sortUrl: legado.sortUrl,
    singleUrl: legado.singleUrl,
    articleStyle: legado.articleStyle,
    ruleArticles: legado.ruleArticles,
    ruleNextPage: legado.ruleNextPage,
    ruleTitle: legado.ruleTitle,
    rulePubDate: legado.rulePubDate,
    ruleDescription: legado.ruleDescription,
    ruleImage: legado.ruleImage,
    ruleLink: legado.ruleLink,
    ruleContent: legado.ruleContent,
    style: legado.style,
    enableJs: legado.enableJs,
    loadWithBaseUrl: legado.loadWithBaseUrl,
    customOrder: legado.customOrder,
    lastUpdateTime: legado.lastUpdateTime,
  }
}

// ─── Safe JSON parsing helpers ───

function parseJsonArray<T>(text: string, filename: string): T[] {
  try {
    const parsed = JSON.parse(text)
    if (!Array.isArray(parsed)) {
      console.warn(`[legadoImport] ${filename} is not an array, skipping`)
      return []
    }
    return parsed as T[]
  } catch (err) {
    console.warn(`[legadoImport] Failed to parse ${filename}:`, err)
    return []
  }
}

// ─── Public API ───

/**
 * Parse a Legado backup ZIP file (as Blob, ArrayBuffer, or File) and
 * return converted data ready to import into reader-next.
 */
export async function parseLegadoBackupZip(
  source: Blob | ArrayBuffer | File,
): Promise<LegadoImportResult> {
  const zip = await JSZip.loadAsync(source)
  return parseLegadoBackupZipFromZip(zip)
}

/**
 * Internal: parse from an already-loaded JSZip instance.
 */
async function parseLegadoBackupZipFromZip(
  zip: JSZip,
): Promise<LegadoImportResult> {
  // Helper to read a file from the zip
  async function readZipFile(name: string): Promise<string | null> {
    const file = zip.file(name)
    if (!file) return null
    return file.async('text')
  }

  // Parse each known file
  const [
    bookshelfText,
    bookSourcesText,
    bookGroupsText,
    bookmarksText,
    replaceRulesText,
    rssSourcesText,
  ] = await Promise.all([
    readZipFile('bookshelf.json'),
    readZipFile('bookSource.json'),
    readZipFile('bookGroup.json'),
    readZipFile('bookmark.json'),
    readZipFile('replaceRule.json'),
    readZipFile('rssSources.json'),
  ])

  // Convert books (filter out unusable URLs)
  const rawBooks = bookshelfText
    ? parseJsonArray<LegadoBook>(bookshelfText, 'bookshelf.json')
    : []
  const usableBooks: Book[] = []
  let skippedBooks = 0
  let incompleteBooks = 0
  for (const raw of rawBooks) {
    if (isUsableBookUrl(raw.bookUrl)) {
      usableBooks.push(convertBook(raw))
    } else if (raw.bookUrl && raw.bookUrl.startsWith('data:')) {
      const decoded = decodeDataUriBook(raw)
      if (decoded) {
        usableBooks.push(decoded)
        incompleteBooks++
      } else {
        skippedBooks++
      }
    } else {
      skippedBooks++
    }
  }
  // Sort by reading progress descending — furthest read first, then by last read time
  usableBooks.sort((a, b) => {
    const aIdx = a.durChapterIndex ?? 0
    const bIdx = b.durChapterIndex ?? 0
    if (bIdx !== aIdx) return bIdx - aIdx
    const aPos = a.durChapterPos ?? 0
    const bPos = b.durChapterPos ?? 0
    if (bPos !== aPos) return bPos - aPos
    return (b.durChapterTime ?? 0) - (a.durChapterTime ?? 0)
  })

  // Convert book sources
  const rawSources = bookSourcesText
    ? parseJsonArray<LegadoBookSource>(bookSourcesText, 'bookSource.json')
    : []
  const bookSources = rawSources.map(convertBookSource)

  // Convert book groups
  const rawGroups = bookGroupsText
    ? parseJsonArray<LegadoBookGroup>(bookGroupsText, 'bookGroup.json')
    : []
  const bookGroups = rawGroups.map(convertBookGroup)

  // Convert bookmarks
  const rawBookmarks = bookmarksText
    ? parseJsonArray<LegadoBookmark>(bookmarksText, 'bookmark.json')
    : []
  const bookmarks = rawBookmarks.map(convertBookmark)

  // Convert replace rules
  const rawRules = replaceRulesText
    ? parseJsonArray<LegadoReplaceRule>(replaceRulesText, 'replaceRule.json')
    : []
  const replaceRules = rawRules.map(convertReplaceRule)

  // Convert RSS sources
  const rawRss = rssSourcesText
    ? parseJsonArray<LegadoRssSource>(rssSourcesText, 'rssSources.json')
    : []
  const rssSources = rawRss.map(convertRssSource)

  return {
    books: usableBooks,
    skippedBooks,
    incompleteBooks,
    bookSources,
    bookGroups,
    bookmarks,
    replaceRules,
    rssSources,
  }
}

/**
 * Quick validation: check if a ZIP file looks like a Legado backup
 * by looking for known filenames.
 */
export async function isLegadoBackupZip(source: Blob | ArrayBuffer | File): Promise<boolean> {
  try {
    const zip = await JSZip.loadAsync(source)
    // A Legado backup must contain at least bookshelf.json or bookSource.json
    return !!zip.file('bookshelf.json') || !!zip.file('bookSource.json')
  } catch {
    return false
  }
}

/**
 * Parse a Legado backup ZIP and return null if it's not a valid Legado backup.
 * Combines validation + parsing in a single zip load to avoid double-reads.
 */
export async function tryParseLegadoBackupZip(
  source: Blob | ArrayBuffer | File,
): Promise<LegadoImportResult | null> {
  try {
    const zip = await JSZip.loadAsync(source)
    if (!zip.file('bookshelf.json') && !zip.file('bookSource.json')) {
      return null
    }
    return await parseLegadoBackupZipFromZip(zip)
  } catch {
    return null
  }
}
