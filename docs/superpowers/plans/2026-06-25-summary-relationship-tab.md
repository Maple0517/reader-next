# AI Summary Relationship Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one `人物关系` tab inside the existing AI summary window, showing a tasteful protagonist-centered relationship graph plus a concise relationship list.

**Architecture:** Reuse existing AI Book memory API data. Add one focused selector utility that converts `AiBookMemoryViewModel` into a small protagonist graph view model, one Vue component that renders the graph/list, then wire that component into the three existing `ReaderView.vue` summary panels. No backend schema changes and no chart dependency unless manual UI verification proves the hand-rolled visual is not good enough.

**Tech Stack:** Vue 3, TypeScript, Vitest, existing Reader CSS, existing `getAiBookMemory(bookUrl)` API client, native HTML/CSS plus SVG connector layer.

---

## File Structure

- Create `/Users/maple/Documents/reader/frontend/src/utils/summaryRelationshipGraph.ts`
  - Responsibility: pure selector only. Identify protagonist, rank direct relationships, aggregate labels/summaries, calculate simple radial positions.
- Create `/Users/maple/Documents/reader/frontend/src/utils/summaryRelationshipGraph.test.ts`
  - Responsibility: TDD coverage for protagonist detection, direct-neighbor filtering, relationship aggregation, recency/strength sorting, and limit behavior.
- Create `/Users/maple/Documents/reader/frontend/src/components/reader/ChapterSummaryRelationshipPanel.vue`
  - Responsibility: render loading/empty/error states, protagonist graph, and relationship list with tasteful Reader-compatible UI.
- Modify `/Users/maple/Documents/reader/frontend/src/views/ReaderView.vue`
  - Responsibility: load AI Book memory for the current book, add the `人物关系` tab beside existing summary tabs, pass selector output to the component in inline/continuous/side summary panels.

---

### Task 1: Relationship graph selector

**Files:**
- Create: `/Users/maple/Documents/reader/frontend/src/utils/summaryRelationshipGraph.test.ts`
- Create: `/Users/maple/Documents/reader/frontend/src/utils/summaryRelationshipGraph.ts`

- [ ] **Step 1: Write the failing test**

Create `/Users/maple/Documents/reader/frontend/src/utils/summaryRelationshipGraph.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { buildSummaryRelationshipGraph } from './summaryRelationshipGraph'
import type { AiBookMemoryViewModel } from '../types'

const memory = {
  bookUrl: 'book',
  enabled: true,
  updatedAt: 1,
  summary: { current: 'summary', recentChanges: [], openQuestions: [] },
  characters: [
    { id: 'hero', name: '张宇', aliases: [], importance: 'high', description: '主角', lastSeenChapterIndex: 10, evidence: [{ chapterIndex: 10, chapterTitle: '第十章', note: '出现' }] },
    { id: 'ally', name: '李青', aliases: [], importance: 'medium', description: '盟友', lastSeenChapterIndex: 10, evidence: [] },
    { id: 'enemy', name: '城主府', aliases: [], importance: 'medium', description: '压力来源', lastSeenChapterIndex: 9, evidence: [] },
    { id: 'mother', name: '张宇母亲', aliases: [], importance: 'low', description: '背景亲属', lastSeenChapterIndex: 1, evidence: [] },
    { id: 'stranger', name: '路人甲', aliases: [], importance: 'low', description: '路人', lastSeenChapterIndex: 10, evidence: [] },
  ],
  relationships: [
    {
      id: 'r1', sourceCharacterId: 'hero', targetCharacterId: 'ally', kind: 'alliance', label: '盟友', polarity: 'positive', strength: 'strong', status: 'developing', direction: 'grouped', summary: '共同调查遗迹', currentDynamics: ['互相信任上升'], facets: [], lastUpdatedChapterIndex: 10, evidence: [], history: [],
    },
    {
      id: 'r2', sourceCharacterId: 'hero', targetCharacterId: 'ally', kind: 'friendship', label: '信任', polarity: 'positive', strength: 'moderate', status: 'active', direction: 'grouped', summary: '私下交换线索', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 9, evidence: [], history: [],
    },
    {
      id: 'r3', sourceCharacterId: 'enemy', targetCharacterId: 'hero', kind: 'conflict', label: '压力', polarity: 'negative', strength: 'critical', status: 'active', direction: 'grouped', summary: '开始关注张宇', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 9, evidence: [], history: [],
    },
    {
      id: 'r4', sourceCharacterId: 'hero', targetCharacterId: 'mother', kind: 'family', label: '家族', polarity: 'positive', strength: 'weak', status: 'distant', direction: 'grouped', summary: '背景牵引', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 1, evidence: [], history: [],
    },
    {
      id: 'r5', sourceCharacterId: 'stranger', targetCharacterId: 'mother', kind: 'unknown', label: '路人关系', polarity: 'neutral', strength: 'weak', status: 'active', direction: 'grouped', summary: '不应进入主角图', currentDynamics: [], facets: [], lastUpdatedChapterIndex: 10, evidence: [], history: [],
    },
  ],
  knowledgeFacts: [],
  locations: [],
  cleanup: { droppedFactsCount: 0, droppedByReason: {}, oldSchemaBackedUp: false },
} satisfies AiBookMemoryViewModel

describe('buildSummaryRelationshipGraph', () => {
  it('centers the most-connected protagonist and aggregates direct relationships', () => {
    const graph = buildSummaryRelationshipGraph({ memory, currentChapterIndex: 10, limit: 6 })

    expect(graph.protagonist?.name).toBe('张宇')
    expect(graph.nodes.map((node) => node.name)).toEqual(['张宇', '李青', '城主府', '张宇母亲'])
    expect(graph.links).toHaveLength(3)
    expect(graph.links[0]).toMatchObject({ targetId: 'ally', label: '盟友 / 信任', summary: '互相信任上升' })
    expect(graph.links[1]).toMatchObject({ targetId: 'enemy', label: '压力', summary: '开始关注张宇' })
    expect(graph.links.find((link) => link.targetId === 'mother')?.tone).toBe('family')
  })

  it('returns an empty reason when memory has no usable relationships', () => {
    const graph = buildSummaryRelationshipGraph({
      memory: { ...memory, relationships: [] },
      currentChapterIndex: 10,
    })

    expect(graph.protagonist).toBeNull()
    expect(graph.emptyReason).toBe('人物关系不足，继续阅读后会补全。')
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cd /Users/maple/Documents/reader/frontend
npm test -- --run summaryRelationshipGraph
```

Expected: FAIL because `summaryRelationshipGraph.ts` does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `/Users/maple/Documents/reader/frontend/src/utils/summaryRelationshipGraph.ts`:

```ts
import type { AiBookMemoryViewModel, AiBookRelationKind, AiBookRelationPolarity, AiBookRelationStrength, AiBookRelationView } from '../types'

export type SummaryRelationshipTone = 'family' | 'romance' | 'ally' | 'conflict' | 'affiliation' | 'neutral'

export interface SummaryRelationshipGraphNode {
  id: string
  name: string
  description: string
  isProtagonist: boolean
  x: number
  y: number
}

export interface SummaryRelationshipGraphLink {
  id: string
  sourceId: string
  targetId: string
  label: string
  summary: string
  tone: SummaryRelationshipTone
  strength: AiBookRelationStrength
  path: string
}

export interface SummaryRelationshipGraphView {
  protagonist: SummaryRelationshipGraphNode | null
  nodes: SummaryRelationshipGraphNode[]
  links: SummaryRelationshipGraphLink[]
  rows: Array<{ id: string; name: string; label: string; summary: string; tone: SummaryRelationshipTone }>
  emptyReason: string
}

export function buildSummaryRelationshipGraph(input: {
  memory: AiBookMemoryViewModel | null
  currentChapterIndex: number
  limit?: number
}): SummaryRelationshipGraphView {
  const memory = input.memory
  if (!memory || memory.relationships.length === 0 || memory.characters.length === 0) return empty('暂无人物关系资料，可先生成 AI资料。')

  const characterById = new Map(memory.characters.map((item) => [item.id, item]))
  const protagonistId = findProtagonistId(memory, input.currentChapterIndex)
  if (!protagonistId) return empty('人物关系不足，继续阅读后会补全。')

  const grouped = new Map<string, AiBookRelationView[]>()
  for (const relation of memory.relationships) {
    const otherId = relation.sourceCharacterId === protagonistId
      ? relation.targetCharacterId
      : relation.targetCharacterId === protagonistId
        ? relation.sourceCharacterId
        : ''
    if (!otherId || !characterById.has(otherId)) continue
    grouped.set(otherId, [...(grouped.get(otherId) || []), relation])
  }

  const related = [...grouped.entries()]
    .map(([characterId, relations]) => ({ characterId, relations, score: relationshipScore(relations, input.currentChapterIndex) }))
    .sort((a, b) => b.score - a.score)
    .slice(0, input.limit ?? 6)

  if (related.length === 0) return empty('人物关系不足，继续阅读后会补全。')

  const protagonist = characterById.get(protagonistId)!
  const center: SummaryRelationshipGraphNode = {
    id: protagonist.id,
    name: protagonist.name,
    description: protagonist.description || '主角',
    isProtagonist: true,
    x: 50,
    y: 50,
  }

  const outerNodes = related.map((item, index) => {
    const character = characterById.get(item.characterId)!
    const angle = -90 + (360 / related.length) * index
    const radius = 34
    const rad = angle * Math.PI / 180
    return {
      id: character.id,
      name: character.name,
      description: character.description || '',
      isProtagonist: false,
      x: Math.round((50 + Math.cos(rad) * radius) * 10) / 10,
      y: Math.round((50 + Math.sin(rad) * radius) * 10) / 10,
    }
  })

  const nodeById = new Map([center, ...outerNodes].map((node) => [node.id, node]))
  const links = related.map((item) => {
    const node = nodeById.get(item.characterId)!
    const label = aggregateLabel(item.relations)
    const summary = aggregateSummary(item.relations)
    return {
      id: item.characterId,
      sourceId: protagonistId,
      targetId: item.characterId,
      label,
      summary,
      tone: toneFor(item.relations),
      strength: strongest(item.relations.map((relation) => relation.strength)),
      path: `M 50 50 L ${node.x} ${node.y}`,
    }
  })

  return {
    protagonist: center,
    nodes: [center, ...outerNodes],
    links,
    rows: links.map((link) => ({ id: link.id, name: nodeById.get(link.targetId)?.name || link.targetId, label: link.label, summary: link.summary, tone: link.tone })),
    emptyReason: '',
  }
}

function empty(emptyReason: string): SummaryRelationshipGraphView {
  return { protagonist: null, nodes: [], links: [], rows: [], emptyReason }
}

function findProtagonistId(memory: AiBookMemoryViewModel, currentChapterIndex: number) {
  const scores = new Map<string, number>()
  for (const character of memory.characters) {
    scores.set(character.id, (character.importance === 'high' ? 4 : 0) + recencyScore(character.lastSeenChapterIndex, currentChapterIndex))
  }
  for (const relation of memory.relationships) {
    scores.set(relation.sourceCharacterId, (scores.get(relation.sourceCharacterId) || 0) + 10)
    scores.set(relation.targetCharacterId, (scores.get(relation.targetCharacterId) || 0) + 10)
  }
  return [...scores.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] || ''
}

function relationshipScore(relations: AiBookRelationView[], currentChapterIndex: number) {
  return relations.reduce((score, relation) => score
    + strengthScore(relation.strength)
    + (relation.status === 'developing' ? 6 : 0)
    + recencyScore(relation.lastUpdatedChapterIndex, currentChapterIndex)
    + relation.currentDynamics.length
    + relation.history.length
    + relation.evidence.length, 0)
}

function recencyScore(index: number | null | undefined, currentChapterIndex: number) {
  if (index == null) return 0
  return Math.max(0, 8 - Math.abs(currentChapterIndex - index))
}

function strengthScore(strength: AiBookRelationStrength) {
  return { critical: 8, strong: 6, moderate: 4, weak: 2, unknown: 0 }[strength] || 0
}

function aggregateLabel(relations: AiBookRelationView[]) {
  return unique(relations.flatMap((relation) => [
    ...relation.facets.map((facet) => facet.subtype || labelForKind(facet.kind)),
    relation.label,
    labelForKind(relation.kind),
  ])).slice(0, 3).join(' / ')
}

function aggregateSummary(relations: AiBookRelationView[]) {
  return relations.flatMap((relation) => [...relation.currentDynamics, relation.summary]).find(Boolean) || '关系仍在发展'
}

function toneFor(relations: AiBookRelationView[]): SummaryRelationshipTone {
  if (relations.some((relation) => relation.kind === 'family')) return 'family'
  if (relations.some((relation) => relation.kind === 'romance')) return 'romance'
  if (relations.some((relation) => relation.kind === 'conflict' || relation.kind === 'rivalry' || relation.polarity === 'negative')) return 'conflict'
  if (relations.some((relation) => relation.kind === 'alliance' || relation.kind === 'friendship' || relation.polarity === 'positive')) return 'ally'
  if (relations.some((relation) => relation.kind === 'affiliation' || relation.kind === 'supervision')) return 'affiliation'
  return 'neutral'
}

function strongest(strengths: AiBookRelationStrength[]) {
  return strengths.sort((a, b) => strengthScore(b) - strengthScore(a))[0] || 'unknown'
}

function labelForKind(kind: AiBookRelationKind) {
  return {
    family: '家族', romance: '情感', friendship: '友情', rivalry: '竞争', alliance: '盟友', conflict: '冲突', affiliation: '阵营', supervision: '师承', unknown: '关联',
  }[kind]
}

function unique(values: string[]) {
  return [...new Set(values.filter(Boolean))]
}
```

- [ ] **Step 4: Run selector test**

Run:

```bash
cd /Users/maple/Documents/reader/frontend
npm test -- --run summaryRelationshipGraph
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/maple/Documents/reader
git add frontend/src/utils/summaryRelationshipGraph.ts frontend/src/utils/summaryRelationshipGraph.test.ts
git commit -m "feat(summary): add relationship graph selector"
```

---

### Task 2: Relationship graph panel component

**Files:**
- Create: `/Users/maple/Documents/reader/frontend/src/components/reader/ChapterSummaryRelationshipPanel.vue`

- [ ] **Step 1: Create component**

Create `/Users/maple/Documents/reader/frontend/src/components/reader/ChapterSummaryRelationshipPanel.vue`:

```vue
<template>
  <section class="summary-relationship-panel" :style="bodyStyle" role="tabpanel">
    <div v-if="status === 'loading'" class="relationship-loading" aria-label="人物关系加载中">
      <span></span><span></span><span></span>
    </div>
    <p v-else-if="status === 'error'" class="relationship-empty">人物关系暂时加载失败。</p>
    <p v-else-if="!graph.protagonist" class="relationship-empty">{{ graph.emptyReason || '暂无人物关系资料，可先生成 AI资料。' }}</p>
    <div v-else class="relationship-layout">
      <div class="relationship-map" aria-label="人物关系图">
        <svg class="relationship-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <path
            v-for="link in graph.links"
            :key="link.id"
            :class="['relationship-link', `tone-${link.tone}`, `strength-${link.strength}`]"
            :d="link.path"
          />
        </svg>
        <div
          v-for="node in graph.nodes"
          :key="node.id"
          :class="['relationship-node', { protagonist: node.isProtagonist }]"
          :style="{ left: `${node.x}%`, top: `${node.y}%` }"
        >
          <strong>{{ node.name }}</strong>
          <small>{{ node.description }}</small>
        </div>
      </div>
      <div class="relationship-rows">
        <div v-for="row in graph.rows" :key="row.id" class="relationship-row">
          <span :class="['relationship-dot', `tone-${row.tone}`]"></span>
          <div>
            <strong>{{ row.name }}｜{{ row.label }}</strong>
            <small>{{ row.summary }}</small>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { CSSProperties } from 'vue'
import type { SummaryRelationshipGraphView } from '../../utils/summaryRelationshipGraph'

defineProps<{
  graph: SummaryRelationshipGraphView
  status: 'idle' | 'loading' | 'ready' | 'error'
  bodyStyle: CSSProperties
}>()
</script>

<style scoped>
.summary-relationship-panel {
  margin-top: 10px;
  line-height: 1.65;
}

.relationship-layout {
  display: grid;
  gap: 14px;
}

.relationship-map {
  position: relative;
  min-height: 260px;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 18px;
  background:
    radial-gradient(circle at 50% 50%, color-mix(in srgb, var(--color-primary, #c97f3a) 16%, transparent), transparent 34%),
    color-mix(in srgb, currentColor 3%, transparent);
}

.relationship-lines {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

.relationship-link {
  fill: none;
  stroke: currentColor;
  stroke-width: 0.65;
  stroke-linecap: round;
  opacity: 0.44;
  vector-effect: non-scaling-stroke;
}

.relationship-link.strength-critical,
.relationship-link.strength-strong {
  stroke-width: 0.9;
  opacity: 0.62;
}

.relationship-node {
  position: absolute;
  transform: translate(-50%, -50%);
  display: grid;
  gap: 2px;
  min-width: 76px;
  max-width: 118px;
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  border-radius: 999px;
  color: inherit;
  background: color-mix(in srgb, currentColor 4%, transparent);
  box-shadow: 0 10px 26px color-mix(in srgb, #000 10%, transparent);
  text-align: center;
}

.relationship-node.protagonist {
  min-width: 104px;
  padding: 12px 14px;
  border-color: color-mix(in srgb, var(--color-primary, #c97f3a) 44%, transparent);
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 16%, transparent);
}

.relationship-node strong,
.relationship-row strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.9em;
}

.relationship-node small,
.relationship-row small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.62;
  font-size: 0.78em;
}

.relationship-rows {
  display: grid;
  gap: 8px;
}

.relationship-row {
  display: flex;
  gap: 9px;
  align-items: flex-start;
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, currentColor 9%, transparent);
  border-radius: 14px;
  background: color-mix(in srgb, currentColor 3%, transparent);
}

.relationship-row > div {
  min-width: 0;
  display: grid;
  gap: 2px;
}

.relationship-dot {
  width: 8px;
  height: 8px;
  margin-top: 0.6em;
  border-radius: 999px;
  background: currentColor;
  opacity: 0.74;
}

.tone-family { color: #5f7fc8; stroke: #5f7fc8; }
.tone-romance { color: #c86a8e; stroke: #c86a8e; }
.tone-ally { color: #5d9d72; stroke: #5d9d72; }
.tone-conflict { color: #c66a5d; stroke: #c66a5d; }
.tone-affiliation { color: #7f6bc8; stroke: #7f6bc8; }
.tone-neutral { color: currentColor; stroke: currentColor; }

.relationship-loading {
  display: grid;
  gap: 10px;
  padding: 16px;
  border: 1px solid color-mix(in srgb, currentColor 9%, transparent);
  border-radius: 18px;
}

.relationship-loading span {
  height: 14px;
  border-radius: 999px;
  background: linear-gradient(90deg, color-mix(in srgb, currentColor 5%, transparent), color-mix(in srgb, currentColor 12%, transparent), color-mix(in srgb, currentColor 5%, transparent));
}

.relationship-empty {
  margin: 0;
  color: inherit;
  opacity: 0.62;
}

@media (max-width: 720px) {
  .relationship-map { min-height: 230px; }
  .relationship-node { min-width: 68px; max-width: 96px; padding: 7px 8px; }
  .relationship-node.protagonist { min-width: 90px; }
}
</style>
```

- [ ] **Step 2: Run build**

Run:

```bash
cd /Users/maple/Documents/reader/frontend
npm run build
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
cd /Users/maple/Documents/reader
git add frontend/src/components/reader/ChapterSummaryRelationshipPanel.vue
git commit -m "feat(summary): add relationship panel"
```

---

### Task 3: Wire tab and data loading in ReaderView

**Files:**
- Modify: `/Users/maple/Documents/reader/frontend/src/views/ReaderView.vue`

- [ ] **Step 1: Add imports and state**

In `/Users/maple/Documents/reader/frontend/src/views/ReaderView.vue`, add imports near existing API/util imports:

```ts
import { getAiBookMemory } from '../api/aiBook'
import { buildSummaryRelationshipGraph } from '../utils/summaryRelationshipGraph'
import type { AiBookMemoryViewModel, Book, ChapterSummaryConfigResponse, ChapterSummaryRecord } from '../types'
```

Replace the existing type import:

```ts
import type { Book, ChapterSummaryConfigResponse, ChapterSummaryRecord } from '../types'
```

with the expanded import above.

Add component import near other async components:

```ts
import ChapterSummaryRelationshipPanel from '../components/reader/ChapterSummaryRelationshipPanel.vue'
```

Add state near existing chapter summary refs:

```ts
const aiBookMemoryForSummary = ref<AiBookMemoryViewModel | null>(null)
const aiBookRelationshipStatus = ref<'idle' | 'loading' | 'ready' | 'error'>('idle')
let aiBookRelationshipRequestId = 0
```

Extend tab type:

```ts
type ChapterSummaryTab = 'content' | 'relationships' | 'settings'
```

Add computed near `chapterSummaryBodyStyle`:

```ts
const summaryRelationshipGraph = computed(() => buildSummaryRelationshipGraph({
  memory: aiBookMemoryForSummary.value,
  currentChapterIndex: store.currentIndex,
}))
```

- [ ] **Step 2: Add loader**

Add near `loadChapterSummaryForCurrentChapter()`:

```ts
async function loadAiBookRelationshipForCurrentBook() {
  const bookUrl = store.book?.bookUrl
  if (!bookUrl) {
    aiBookMemoryForSummary.value = null
    aiBookRelationshipStatus.value = 'idle'
    return
  }

  const requestId = ++aiBookRelationshipRequestId
  aiBookRelationshipStatus.value = 'loading'
  try {
    const res = await getAiBookMemory(bookUrl)
    if (requestId !== aiBookRelationshipRequestId || store.book?.bookUrl !== bookUrl) return
    aiBookMemoryForSummary.value = res.memory
    aiBookRelationshipStatus.value = 'ready'
  } catch {
    if (requestId !== aiBookRelationshipRequestId) return
    aiBookMemoryForSummary.value = null
    aiBookRelationshipStatus.value = 'error'
  }
}
```

In the existing watcher that calls `loadChapterSummaryForCurrentChapter()`, call the relationship loader:

```ts
watch(
  () => [store.book?.bookUrl, store.currentChapter?.url, store.currentIndex, store.displayContent] as const,
  () => {
    resetChapterSummaryState()
    void loadChapterSummaryForCurrentChapter()
    void loadAiBookRelationshipForCurrentBook()
  },
  { immediate: true },
)
```

- [ ] **Step 3: Add tab button to each summary-tabs block**

In all three `.summary-tabs` blocks, insert this button between `正文` and `设置`:

```vue
<button
  class="summary-tab"
  :class="{ active: chapterSummaryActiveTab === 'relationships' }"
  :aria-selected="chapterSummaryActiveTab === 'relationships'"
  role="tab"
  type="button"
  @click="chapterSummaryActiveTab = 'relationships'"
>人物关系</button>
```

- [ ] **Step 4: Add panel to each summary body block**

After each existing content section and before each settings section, add:

```vue
<ChapterSummaryRelationshipPanel
  v-else-if="chapterSummaryActiveTab === 'relationships'"
  :graph="summaryRelationshipGraph"
  :status="aiBookRelationshipStatus"
  :body-style="chapterSummaryBodyStyle"
/>
```

For side mode, add the same component directly between the side content section and side settings section.

- [ ] **Step 5: Run build**

Run:

```bash
cd /Users/maple/Documents/reader/frontend
npm run build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cd /Users/maple/Documents/reader
git add frontend/src/views/ReaderView.vue
git commit -m "feat(summary): wire relationship tab"
```

---

### Task 4: Taste and visual verification

**Files:**
- Modify only if verification shows UI slop.

- [ ] **Step 1: Start single-port Reader app if needed**

Check port first:

```bash
cd /Users/maple/Documents/reader
lsof -i :18080
```

If no Reader server is running, start the normal single-port app:

```bash
cd /Users/maple/Documents/reader
cd frontend && npm run build
cd ..
SERVER_PORT=18080 cargo run
```

- [ ] **Step 2: Browser verify the summary tab**

Open `http://localhost:18080` and verify in an actual book reader view:

- AI summary window has `正文 / 人物关系 / 设置`.
- Default selected tab is still `正文`.
- `人物关系` tab appears in inline, continuous, and side summary layouts.
- Empty state is readable when no AI资料 exists.
- With AI资料, graph looks like a reading aid, not a debug SVG.
- Light and dark theme are both legible.

- [ ] **Step 3: Taste audit fixes if needed**

If the graph looks like a raw engineering diagram, adjust only the component CSS:

- Increase map whitespace, not density.
- Make center node visibly primary.
- Keep relationship colors muted.
- Keep long text in list, not in graph.
- Do not add animations unless they make state changes clearer.

- [ ] **Step 4: Final checks**

Run:

```bash
cd /Users/maple/Documents/reader
git diff --check
cd frontend && npm test -- --run summaryRelationshipGraph && npm run build
```

Expected: all pass.

- [ ] **Step 5: Commit any taste fixes**

If Step 3 changed files:

```bash
cd /Users/maple/Documents/reader
git add frontend/src/components/reader/ChapterSummaryRelationshipPanel.vue frontend/src/views/ReaderView.vue
git commit -m "style(summary): polish relationship graph"
```

---

## Self-Review

- Spec coverage: AI summary internal tab, protagonist-centered graph, direct relationships, aggregation, no backend change, and visual quality are each covered by tasks.
- Placeholder scan: no `TBD`, `TODO`, or vague implementation-only steps remain.
- Type consistency: selector exports `SummaryRelationshipGraphView`; component accepts that exact type; ReaderView computed passes that exact view.
