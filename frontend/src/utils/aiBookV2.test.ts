import { describe, expect, it } from 'vitest'
import type { AiBookEvidence, AiBookMemoryV2, Book, BookChapter } from '../types'
import {
  createEmptyAiBookMemoryV2,
  reconcileAiBookMemoryV2,
  toAiBookDisplayMemory,
} from './aiBookV2'

const book: Book = {
  name: '山海旧事',
  author: '佚名',
  bookUrl: 'book-1',
  origin: 'source-1',
}

const chapter: BookChapter = {
  title: '第八章 北境',
  url: 'chapter-8',
  index: 7,
}

describe('aiBookV2', () => {
  it('creates an empty V2 memory', () => {
    const memory = createEmptyAiBookMemoryV2(book)

    expect(memory.schemaVersion).toBe(2)
    expect(memory.bookUrl).toBe('book-1')
    expect(memory.summary).toEqual({ current: '', recentChanges: [], openQuestions: [] })
    expect(memory.chapterDigests).toEqual([])
    expect(memory.mapState).toMatchObject({ dirty: false, nodes: [], edges: [] })
  })

  it('adapts V2 memory into the existing display shape', () => {
    const memory: AiBookMemoryV2 = {
      ...createEmptyAiBookMemoryV2(book),
      summary: {
        current: '林舟抵达北境，旧神传说仍未确认。',
        recentChanges: ['林舟离开旧村'],
        openQuestions: ['旧神是否真实存在'],
      },
      worldFacts: [{
        id: 'fact-old-god',
        category: '历史传说',
        title: '旧神传说',
        content: '北境流传旧神传说，真伪未知。',
        confidence: '推断',
        importance: 'high',
        evidence: [{ chapterIndex: 7, chapterTitle: '第八章 北境', note: '北境首次提到旧神' }],
      }],
      characters: [{
        id: 'char-lin-zhou',
        name: '林舟',
        aliases: ['阿舟'],
        importance: 'high',
        currentStatus: '抵达北境',
        currentLocationId: 'loc-north',
        statusHistory: [],
        evidence: [],
      }],
      relationships: [{
        id: 'rel-lin-shen',
        sourceCharacterId: 'char-lin-zhou',
        targetEntityId: 'char-shen-yue',
        targetKind: 'character',
        relationType: '临时同伴',
        direction: 'directed',
        description: '沈月在北境协助林舟。',
        importance: 'medium',
        evidence: [],
      }],
      locations: [{
        id: 'loc-north',
        name: '北境',
        aliases: [],
        importance: 'high',
        kind: '区域',
        scale: 'region',
        description: '寒冷边境。',
        relatedCharacterIds: ['char-lin-zhou'],
        evidence: [],
      }],
      renderArtifacts: {
        mapImageUrl: '/assets/ai-maps/north.png',
        mapImagePrompt: '绘制北境地图',
        mapFallbackReason: '图片模型不可用',
      },
    }

    const display = toAiBookDisplayMemory(memory)

    expect(display.summary).toBe('林舟抵达北境，旧神传说仍未确认。')
    expect(display.worldview[0]).toMatchObject({ category: '历史传说', title: '旧神传说' })
    expect(display.characters[0]).toMatchObject({ name: '林舟', location: '北境', status: '抵达北境' })
    expect(display.relationships[0]).toMatchObject({
      source: '林舟',
      relation: '临时同伴',
      target: 'char-shen-yue',
    })
    expect(display.locations[0]).toMatchObject({ name: '北境', description: '寒冷边境。' })
    expect(display.map).toMatchObject({
      imageUrl: '/assets/ai-maps/north.png',
      prompt: '绘制北境地图',
      fallbackReason: '图片模型不可用',
    })
  })

  it('merges character aliases into an existing character', () => {
    const first = reconcileAiBookMemoryV2(createEmptyAiBookMemoryV2(book), {
      chapterDigest: digest('林舟离开旧村。'),
      characters: [{
        id: 'char-lin-zhou',
        name: '林舟',
        aliases: [],
        importance: 'high',
        currentStatus: '离开旧村',
        evidence: evidence(),
      }],
    }, book, chapter).memory

    const next = reconcileAiBookMemoryV2(first, {
      chapterDigest: digest('林舟抵达北境。'),
      characters: [{
        id: 'char-lin-zhou',
        name: '林舟',
        aliases: ['阿舟'],
        importance: 'high',
        currentStatus: '抵达北境',
        evidence: evidence('同伴称呼林舟为阿舟'),
      }],
    }, book, chapter).memory

    expect(next.characters).toHaveLength(1)
    expect(next.characters[0].aliases).toEqual(['阿舟'])
    expect(next.characters[0].currentStatus).toBe('抵达北境')
    expect(next.characters[0].evidence.length).toBeGreaterThan(1)
  })

  it('keeps reverse directed relationships as separate facts', () => {
    const memory = reconcileAiBookMemoryV2(createEmptyAiBookMemoryV2(book), {
      chapterDigest: digest('林舟与沈月互相试探。'),
      characters: [
        { id: 'char-lin-zhou', name: '林舟', importance: 'high', currentStatus: '在北境', evidence: evidence() },
        { id: 'char-shen-yue', name: '沈月', importance: 'medium', currentStatus: '在北境', evidence: evidence() },
      ],
      relationships: [
        {
          sourceName: '林舟',
          targetName: '沈月',
          targetKind: 'character',
          relationType: '试探',
          direction: 'directed',
          description: '林舟试探沈月来意。',
          importance: 'medium',
          evidence: evidence(),
        },
        {
          sourceName: '沈月',
          targetName: '林舟',
          targetKind: 'character',
          relationType: '试探',
          direction: 'directed',
          description: '沈月试探林舟底牌。',
          importance: 'medium',
          evidence: evidence(),
        },
      ],
    }, book, chapter).memory

    expect(memory.relationships).toHaveLength(2)
    expect(memory.relationships.map((item) => item.sourceCharacterId).sort()).toEqual([
      'char-lin-zhou',
      'char-shen-yue',
    ])
  })

  it('rejects invalid location parents and records an open question', () => {
    const memory = reconcileAiBookMemoryV2(createEmptyAiBookMemoryV2(book), {
      chapterDigest: digest('阿尔托与维克托住宅出现。'),
      locations: [
        {
          name: '阿尔托',
          kind: '城市',
          scale: 'city',
          parentName: '维克托住宅',
          description: '故事当前城市。',
          importance: 'high',
          evidence: evidence(),
        },
        {
          name: '维克托住宅',
          kind: '住宅',
          scale: 'building',
          description: '维克托授课地点。',
          importance: 'medium',
          evidence: evidence(),
        },
      ],
    }, book, chapter).memory

    const alto = memory.locations.find((item) => item.name === '阿尔托')
    expect(alto?.parentId).toBeUndefined()
    expect(memory.summary.openQuestions.join('\n')).toContain('阿尔托')
  })

  it('marks map dirty when an important location is added', () => {
    const update = reconcileAiBookMemoryV2(createEmptyAiBookMemoryV2(book), {
      chapterDigest: digest('林舟抵达北境。'),
      locations: [{
        name: '北境',
        kind: '区域',
        scale: 'region',
        description: '寒冷边境。',
        importance: 'high',
        evidence: evidence(),
      }],
      mapChanges: {
        changed: true,
        reason: '新增北境区域',
        affectedLocationNames: ['北境'],
        routeHints: [],
      },
    }, book, chapter)

    expect(update.shouldRegenerateMap).toBe(true)
    expect(update.memory.mapState).toMatchObject({ dirty: true, reason: '新增北境区域' })
    expect(update.mapPrompt).toContain('北境')
  })
})

function digest(text: string) {
  return {
    chapterIndex: chapter.index,
    chapterTitle: chapter.title,
    digest: text,
    keyEvents: [text],
  }
}

function evidence(note = '本章明确提及'): AiBookEvidence[] {
  return [{ chapterIndex: chapter.index, chapterTitle: chapter.title, note }]
}
