import { describe, expect, it } from 'vitest'
import {
  buildAiBookLocationRows,
  groupAiBookWorldview,
} from './aiBookPresentation'

describe('aiBookPresentation', () => {
  it('groups worldview notes and marks collapsed categories', () => {
    const groups = groupAiBookWorldview([
      { category: '基础设定', title: '穿越机制', content: '未知原因穿越。' },
      { category: '基础设定', title: '鲁恩王国', content: '北大陆国家。' },
      { category: '当前事件', title: '枪伤', content: '额角存在枪伤。' },
    ], new Set(['基础设定']))

    expect(groups).toEqual([
      {
        category: '基础设定',
        collapsed: true,
        items: [
          { category: '基础设定', title: '穿越机制', content: '未知原因穿越。' },
          { category: '基础设定', title: '鲁恩王国', content: '北大陆国家。' },
        ],
      },
      {
        category: '当前事件',
        collapsed: false,
        items: [
          { category: '当前事件', title: '枪伤', content: '额角存在枪伤。' },
        ],
      },
    ])
  })

  it('builds location rows from explicit parent names and collapsed parents', () => {
    const rows = buildAiBookLocationRows([
      { name: '廷根市', kind: '城市', description: '鲁恩王国城市。' },
      { name: '莫雷蒂家公寓', parentName: '廷根市', kind: '住宅', description: '克莱恩的住所。' },
    ], new Set(['廷根市']))

    expect(rows).toEqual([
      {
        location: { name: '廷根市', kind: '城市', description: '鲁恩王国城市。' },
        depth: 0,
        hasChildren: true,
      },
    ])
  })

  it('infers a building-like location under the only city when parentName is missing', () => {
    const rows = buildAiBookLocationRows([
      { name: '廷根市', kind: '城市', description: '克莱恩居住的城市。' },
      { name: '莫雷蒂家公寓', kind: '住宅', description: '克莱恩的住所，包含书桌、高低床、橱柜等设施。' },
    ], new Set())

    expect(rows.map((row) => ({
      name: row.location.name,
      parentName: row.location.parentName,
      depth: row.depth,
      hasChildren: row.hasChildren,
    }))).toEqual([
      { name: '廷根市', parentName: undefined, depth: 0, hasChildren: true },
      { name: '莫雷蒂家公寓', parentName: '廷根市', depth: 1, hasChildren: false },
    ])
  })

  it('repairs reversed model parent names by place scale', () => {
    const rows = buildAiBookLocationRows([
      { name: '廷根技术学校', kind: '学校', description: '中等教育机构，梅丽莎在此就读。' },
      { name: '廷根市', parentName: '廷根技术学校', kind: '城市', description: '克莱恩居住的城市，拥有霍伊大学、廷根技术学校、大教堂等建筑。' },
      { name: '鲁恩王国', parentName: '廷根市', kind: '国家', description: '位于北大陆，拥有廷根市、阿霍瓦郡等地。' },
      { name: '廷根大教堂', kind: '宗教建筑', description: '位于廷根市，用于报时。' },
      { name: '莫雷蒂家公寓', kind: '住宅', description: '莫雷蒂三兄妹的住所。' },
    ], new Set())

    const simplified = rows.map((row) => ({
      name: row.location.name,
      parentName: row.location.parentName,
      depth: row.depth,
    }))

    expect(simplified.slice(0, 2)).toEqual([
      { name: '鲁恩王国', parentName: undefined, depth: 0 },
      { name: '廷根市', parentName: '鲁恩王国', depth: 1 },
    ])
    expect(simplified.slice(2)).toEqual(expect.arrayContaining([
      { name: '廷根大教堂', parentName: '廷根市', depth: 2 },
      { name: '廷根技术学校', parentName: '廷根市', depth: 2 },
      { name: '莫雷蒂家公寓', parentName: '廷根市', depth: 2 },
    ]))
  })

  it('allows facilities and sites to nest under a school kind', () => {
    const rows = buildAiBookLocationRows([
      { name: 'North Academy', kind: 'school', description: '主校区。' },
      { name: 'Training Grounds', parentName: 'North Academy', kind: 'facility', description: '学院训练设施。' },
      { name: 'Old Library', parentName: 'North Academy', kind: 'site', description: '旧馆遗址。' },
    ], new Set())

    const simplified = rows.map((row) => ({
      name: row.location.name,
      parentName: row.location.parentName,
      depth: row.depth,
    }))

    expect(simplified[0]).toEqual({ name: 'North Academy', parentName: undefined, depth: 0 })
    expect(simplified.slice(1)).toEqual(expect.arrayContaining([
      { name: 'Old Library', parentName: 'North Academy', depth: 1 },
      { name: 'Training Grounds', parentName: 'North Academy', depth: 1 },
    ]))
  })
})
