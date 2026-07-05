import { describe, it, expect } from 'vitest'
import JSZip from 'jszip'
import { parseLegadoBackupZip, isLegadoBackupZip, tryParseLegadoBackupZip } from './legadoImport'

// ─── Test fixtures ───

const sampleLegadoBook = {
  name: '测试书籍',
  author: '测试作者',
  bookUrl: 'https://example.com/book/1',
  origin: 'https://example.com',
  originName: '测试书源',
  coverUrl: 'https://example.com/cover.jpg',
  durChapterIndex: 10,
  durChapterPos: 500,
  durChapterTime: 1700000000000,
  durChapterTitle: '第十章',
  group: 0,
  canUpdate: true,
  totalChapterNum: 100,
  type: 0,
  order: 0,
}

const sampleBookSource = {
  bookSourceName: '测试书源',
  bookSourceUrl: 'https://example.com',
  bookSourceGroup: '测试',
  bookSourceType: 0,
  enabled: true,
  enabledExplore: true,
  searchUrl: 'https://example.com/search?keyword={{key}}',
  ruleSearch: { bookList: 'div.book-item', bookName: 'h3' },
  ruleBookInfo: { name: 'h1', author: '.author' },
  ruleToc: { chapterList: 'li.chapter', chapterName: 'a' },
  ruleContent: { content: '#content' },
}

const sampleBookGroup = {
  groupId: 1,
  groupName: '收藏',
  order: 0,
  show: true,
  enableRefresh: true,
  bookSort: -1,
  isPrivate: false,
}

const sampleBookmark = {
  bookName: '测试书籍',
  bookAuthor: '测试作者',
  chapterIndex: 5,
  chapterPos: 200,
  chapterName: '第五章',
  bookText: '这是书签标记的文字',
  content: '这是书签标记的文字',
  time: 1700000000000,
}

const sampleReplaceRule = {
  id: 1,
  name: '净化规则',
  group: '默认',
  pattern: '广告内容',
  replacement: '',
  scopeContent: true,
  scopeTitle: false,
  isEnabled: true,
  isRegex: false,
  order: 0,
}

const sampleRssSource = {
  sourceUrl: 'https://example.com/rss',
  sourceName: '测试RSS',
  sourceGroup: '新闻',
  enabled: true,
  ruleArticles: 'article',
  ruleTitle: 'h2',
  ruleLink: 'a[href]',
}

// Helper to create a zip blob from JSON files
async function createTestZip(files: Record<string, unknown>): Promise<Blob> {
  const zip = new JSZip()
  for (const [name, data] of Object.entries(files)) {
    zip.file(name, JSON.stringify(data))
  }
  return zip.generateAsync({ type: 'blob' })
}

// ─── Tests ───

describe('isLegadoBackupZip', () => {
  it('returns true for valid Legado zip with bookshelf.json', async () => {
    const blob = await createTestZip({ 'bookshelf.json': [sampleLegadoBook] })
    expect(await isLegadoBackupZip(blob)).toBe(true)
  })

  it('returns true for valid Legado zip with bookSource.json only', async () => {
    const blob = await createTestZip({ 'bookSource.json': [sampleBookSource] })
    expect(await isLegadoBackupZip(blob)).toBe(true)
  })

  it('returns false for zip without known files', async () => {
    const blob = await createTestZip({ 'random.json': {} })
    expect(await isLegadoBackupZip(blob)).toBe(false)
  })

  it('returns false for non-zip data', async () => {
    const blob = new Blob(['not a zip'])
    expect(await isLegadoBackupZip(blob)).toBe(false)
  })
})

describe('parseLegadoBackupZip', () => {
  it('parses a complete Legado backup', async () => {
    const blob = await createTestZip({
      'bookshelf.json': [sampleLegadoBook],
      'bookSource.json': [sampleBookSource],
      'bookGroup.json': [sampleBookGroup],
      'bookmark.json': [sampleBookmark],
      'replaceRule.json': [sampleReplaceRule],
      'rssSources.json': [sampleRssSource],
    })

    const result = await parseLegadoBackupZip(blob)

    expect(result.books).toHaveLength(1)
    expect(result.books[0].name).toBe('测试书籍')
    expect(result.books[0].bookUrl).toBe('https://example.com/book/1')
    expect(result.books[0].durChapterIndex).toBe(10)
    expect(result.skippedBooks).toBe(0)

    expect(result.bookSources).toHaveLength(1)
    expect(result.bookSources[0].bookSourceName).toBe('测试书源')

    expect(result.bookGroups).toHaveLength(1)
    expect(result.bookGroups[0].groupName).toBe('收藏')

    expect(result.bookmarks).toHaveLength(1)
    expect(result.bookmarks[0].bookName).toBe('测试书籍')

    expect(result.replaceRules).toHaveLength(1)
    expect(result.replaceRules[0].name).toBe('净化规则')
    expect(result.replaceRules[0].scope).toBe('content')

    expect(result.rssSources).toHaveLength(1)
    expect(result.rssSources[0].sourceName).toBe('测试RSS')
  })

  it('filters out content:// books', async () => {
    const blob = await createTestZip({
      'bookshelf.json': [
        sampleLegadoBook,
        { ...sampleLegadoBook, name: '本地书', bookUrl: 'content://com.example/book' },
        { ...sampleLegadoBook, name: '内联书', bookUrl: 'data:;base64,abc123' },
      ],
    })

    const result = await parseLegadoBackupZip(blob)
    expect(result.books).toHaveLength(1)
    expect(result.books[0].name).toBe('测试书籍')
    expect(result.skippedBooks).toBe(2)
  })

  it('handles empty or missing files gracefully', async () => {
    const blob = await createTestZip({})
    const result = await parseLegadoBackupZip(blob)

    expect(result.books).toHaveLength(0)
    expect(result.bookSources).toHaveLength(0)
    expect(result.bookGroups).toHaveLength(0)
    expect(result.bookmarks).toHaveLength(0)
    expect(result.replaceRules).toHaveLength(0)
    expect(result.rssSources).toHaveLength(0)
    expect(result.skippedBooks).toBe(0)
  })

  it('handles malformed JSON gracefully', async () => {
    const zip = new JSZip()
    zip.file('bookshelf.json', 'not valid json{{{')
    zip.file('bookSource.json', JSON.stringify([sampleBookSource]))
    const blob = await zip.generateAsync({ type: 'blob' })

    const result = await parseLegadoBackupZip(blob)
    expect(result.books).toHaveLength(0)
    expect(result.bookSources).toHaveLength(1)
  })

  it('converts replace rule scope correctly', async () => {
    const blob = await createTestZip({
      'replaceRule.json': [
        { ...sampleReplaceRule, scopeContent: true, scopeTitle: true },
        { ...sampleReplaceRule, id: 2, name: '标题规则', scopeContent: false, scopeTitle: true },
        { ...sampleReplaceRule, id: 3, name: '内容规则', scopeContent: true, scopeTitle: false },
        { ...sampleReplaceRule, id: 4, name: '无scope', scopeContent: false, scopeTitle: false },
      ],
    })

    const result = await parseLegadoBackupZip(blob)
    expect(result.replaceRules[0].scope).toBe('all')
    expect(result.replaceRules[1].scope).toBe('title')
    expect(result.replaceRules[2].scope).toBe('content')
    expect(result.replaceRules[3].scope).toBeUndefined()
  })

  it('sorts books by reading progress descending', async () => {
    const blob = await createTestZip({
      'bookshelf.json': [
        { ...sampleLegadoBook, name: '旧书', durChapterIndex: 10, durChapterPos: 0, durChapterTime: 1700000000000 },
        { ...sampleLegadoBook, name: '新书', durChapterIndex: 100, durChapterPos: 500, durChapterTime: 1700100000000 },
        { ...sampleLegadoBook, name: '中间', durChapterIndex: 50, durChapterPos: 0, durChapterTime: 1700050000000 },
      ],
    })
    const result = await parseLegadoBackupZip(blob)
    expect(result.books[0].name).toBe('新书')   // chapter 100
    expect(result.books[1].name).toBe('中间')   // chapter 50
    expect(result.books[2].name).toBe('旧书')   // chapter 10
  })

  it('maps Legado book fields to reader-next format', async () => {
    const legadoBook = {
      ...sampleLegadoBook,
      latestChapterTitle: '最新章节',
      lastCheckTime: 1700000000000,
      tocUrl: 'https://example.com/toc',
      charset: 'utf-8',
    }
    const blob = await createTestZip({ 'bookshelf.json': [legadoBook] })
    const result = await parseLegadoBackupZip(blob)

    const book = result.books[0]
    expect(book.name).toBe('测试书籍')
    expect(book.author).toBe('测试作者')
    expect(book.origin).toBe('https://example.com')
    expect(book.latestChapterTitle).toBe('最新章节')
    expect(book.lastCheckTime).toBe(1700000000000)
    expect(book.tocUrl).toBe('https://example.com/toc')
    expect(book.charset).toBe('utf-8')
    expect(book.canUpdate).toBe(true)
  })

  it('handles bookmark with only content field (no bookText)', async () => {
    const blob = await createTestZip({
      'bookmark.json': [{
        bookName: '书名',
        bookAuthor: '作者',
        content: '标记内容',
      }],
    })

    const result = await parseLegadoBackupZip(blob)
    expect(result.bookmarks[0].bookText).toBe('标记内容')
    expect(result.bookmarks[0].content).toBe('标记内容')
  })
})
describe('tryParseLegadoBackupZip', () => {
  it('returns LegadoImportResult for valid zip', async () => {
    const blob = await createTestZip({
      'bookshelf.json': [sampleLegadoBook],
      'bookSource.json': [sampleBookSource],
    })
    const result = await tryParseLegadoBackupZip(blob)
    expect(result).not.toBeNull()
    expect(result!.books).toHaveLength(1)
    expect(result!.bookSources).toHaveLength(1)
  })

  it('returns null for non-Legado zip', async () => {
    const blob = await createTestZip({ 'random.json': {} })
    const result = await tryParseLegadoBackupZip(blob)
    expect(result).toBeNull()
  })

  it('returns null for non-zip data', async () => {
    const blob = new Blob(['not a zip'])
    const result = await tryParseLegadoBackupZip(blob)
    expect(result).toBeNull()
  })
})

