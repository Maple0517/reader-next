# V4 Console Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign V4 frontend from modern dashboard to restrained warm-tone memory database console, matching the 7 rail page designs in `recovered/v4-frontend-focus/visual-companion/index.html`.

**Architecture:** V4 independent design system (`--v4-*` CSS tokens) scoped to V4 console. Shell becomes pure layout container, panels get visual redesign against the HTML mockup. No new API, no new dependency, no V3 impact.

**Tech Stack:** Vue 3 Composition API + `<script setup lang="ts">`, pure CSS animations, existing V4 API layer unchanged.

**Spec:** `docs/superpowers/specs/2026-07-08-v4-console-redesign-design.md`
**HTML mockup reference:** `recovered/v4-frontend-focus/visual-companion/index.html` (每个 task 中标注具体行号)

## Global Constraints

- All V4 CSS uses `--v4-*` tokens only, never `--color-*` global tokens.
- Zero new npm dependencies.
- All existing tests must pass after each task.
- Each task ends with `cd frontend && npm run build` (or focused `npm test -- V4` for test tasks).
- Vue 3 Composition API + `<script setup lang="ts">` only.
- `shallowRef` for primitives, `ref` for replaceable objects, `computed` for derived display data.

---

## File Map

| File | Action | Task |
| --- | --- | --- |
| `frontend/src/styles/v4-console.css` | Create | 1 |
| `frontend/src/main.ts` | Modify (import CSS) | 1 |
| `frontend/src/components/reader/v4/V4ConsoleShell.vue` | Modify | 2 |
| `frontend/src/components/reader/v4/V4ConsoleShell.test.ts` | Modify | 2 |
| `frontend/src/components/reader/v4/V4DomainRail.vue` | Modify | 3 |
| `frontend/src/components/reader/v4/V4PanelShell.vue` | Modify | 4 |
| `frontend/src/components/reader/v4/V4PanelShell.test.ts` | Modify | 4 |
| `frontend/src/views/AiBookV4View.vue` | Modify | 5 |
| `frontend/src/views/AiBookV4View.test.ts` | Modify | 5 |
| `frontend/src/components/reader/V4BookOverviewPanel.vue` | Modify | 6 |
| `frontend/src/components/reader/V4BookOverviewPanel.test.ts` | Modify | 6 |
| `frontend/src/components/reader/V4TaskProgressPanel.vue` | Modify | 7 |
| `frontend/src/components/reader/V4TaskProgressPanel.test.ts` | Modify | 7 |
| `frontend/src/components/reader/V4CharacterPanel.vue` | Modify | 8 |
| `frontend/src/components/reader/V4CharacterPanel.test.ts` | Modify | 8 |
| `frontend/src/components/reader/V4RelationshipPanel.vue` | Modify | 9 |
| `frontend/src/components/reader/V4RelationshipPanel.test.ts` | Modify | 9 |
| `frontend/src/components/reader/V4KnowledgePanel.vue` | Modify | 10 |
| `frontend/src/components/reader/V4KnowledgePanel.test.ts` | Modify | 10 |
| `frontend/src/components/reader/V4MapPanel.vue` | Modify | 11 |
| `frontend/src/components/reader/V4MapPanel.test.ts` | Modify | 11 |
| `frontend/src/components/reader/V4QualityPanel.vue` | Modify | 12 |
| `frontend/src/components/reader/V4QualityPanel.test.ts` | Modify | 12 |

---

### Task 1: V4 Design Token System

**Files:**
- Create: `frontend/src/styles/v4-console.css`
- Modify: `frontend/src/main.ts` (add import)

**Interfaces:**
- Produces: All `--v4-*` CSS custom properties consumed by every subsequent task

- [ ] **Step 1: Create `frontend/src/styles/v4-console.css`**

```css
/* V4 Console — Restrained Warm Tone Design System */

/* === Base Palette === */
:root {
  --v4-bg: #f5f1ea;
  --v4-surface: #fffdf9;
  --v4-line: #d8d0c3;
  --v4-ink: #181612;
  --v4-muted: #655f55;
  --v4-soft: #ebe4d8;
  --v4-inspector-bg: #fffaf2;
  --v4-rail-bg: #eee6d8;
  --v4-shell-top-bg: #f0ebe3;

  /* Functional */
  --v4-accent: #3657d6;
  --v4-success: #26704f;
  --v4-warn: #9a6416;
  --v4-danger: #b83a3a;
  --v4-purple: #6550b9;

  /* Spacing */
  --v4-radius: 0px;
  --v4-gap: 6px;
  --v4-pad: 8px;
  --v4-pad-lg: 10px;
  --v4-line-height: 1.45;
}

/* === Utility Classes === */
.v4-pill {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 9px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  color: var(--v4-muted);
  font-size: 12px;
  font-weight: 750;
  white-space: nowrap;
}

.v4-pill.blue {
  color: var(--v4-accent);
  border-color: color-mix(in srgb, var(--v4-accent) 34%, var(--v4-line));
}

.v4-pill.green {
  color: var(--v4-success);
  border-color: color-mix(in srgb, var(--v4-success) 34%, var(--v4-line));
}

.v4-pill.amber {
  color: var(--v4-warn);
  border-color: color-mix(in srgb, var(--v4-warn) 34%, var(--v4-line));
}

.v4-pill.red {
  color: var(--v4-danger);
  border-color: color-mix(in srgb, var(--v4-danger) 34%, var(--v4-line));
}

.v4-pill.violet {
  color: var(--v4-purple);
  border-color: color-mix(in srgb, var(--v4-purple) 34%, var(--v4-line));
}

.v4-icon-btn {
  width: 28px;
  height: 24px;
  display: grid;
  place-items: center;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  color: var(--v4-muted);
  font-size: 12px;
  font-weight: 850;
  cursor: pointer;
}

.v4-action-btn {
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  min-height: 28px;
  padding: 0 9px;
  font-size: 11px;
  font-weight: 850;
  color: var(--v4-ink);
  cursor: pointer;
}

.v4-action-btn.primary {
  border-color: color-mix(in srgb, var(--v4-accent) 48%, var(--v4-line));
  color: var(--v4-accent);
  background: color-mix(in srgb, var(--v4-accent) 7%, var(--v4-surface));
}

.v4-action-btn:disabled {
  cursor: not-allowed;
  opacity: 0.52;
}

.v4-evidence {
  border-left: 3px solid var(--v4-success);
  background: color-mix(in srgb, var(--v4-success) 8%, var(--v4-surface));
  padding: 8px 9px;
  font-size: 11px;
  color: var(--v4-muted);
}

.v4-field {
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  padding: 8px;
  min-height: 54px;
}

.v4-field label {
  display: block;
  color: var(--v4-muted);
  font-size: 10px;
  margin-bottom: 3px;
}

.v4-field strong {
  font-size: 12px;
}

/* === Progress Animations === */
@keyframes v4-shimmer {
  to { transform: translateX(100%); }
}

@keyframes v4-scan {
  0%, 18% { transform: translateX(-100%); }
  72%, 100% { transform: translateX(100%); }
}

@keyframes v4-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.62; transform: scale(0.92); }
}
```

- [ ] **Step 2: Import in `frontend/src/main.ts`**

Find the existing CSS imports (near the top) and add after them:

```ts
import './styles/v4-console.css'
```

- [ ] **Step 3: Verify build**

Run: `cd frontend && npm run build`
Expected: Build succeeds, no new warnings.

---

### Task 2: Restructure V4ConsoleShell

**Files:**
- Modify: `frontend/src/components/reader/v4/V4ConsoleShell.vue`
- Modify: `frontend/src/components/reader/v4/V4ConsoleShell.test.ts`

**Interfaces:**
- Consumes: `--v4-*` tokens from Task 1
- Produces: Pure layout shell (header slot + rail slot + default slot) used by AiBookV4View in Task 5

**Reference:** HTML mockup lines 148-165 (`.mock-top` top bar) and lines 192-229 (`.console-a` body)

- [ ] **Step 1: Read current `V4ConsoleShell.vue`**

Run: `cat frontend/src/components/reader/v4/V4ConsoleShell.vue`

- [ ] **Step 2: Rewrite template**

Replace the entire `<template>` section. The new shell is a pure layout container:

```vue
<template>
  <section class="v4-console" aria-label="AI Book Memory V4 console">
    <div class="v4-top-bar">
      <slot name="header" />
    </div>

    <div class="v4-console-body">
      <aside class="v4-console-rail">
        <slot name="rail-before" />
        <V4DomainRail :items="railItems" :active-key="activeKey" @select="emit('select', $event)" />
        <slot name="rail-after" />
      </aside>

      <div class="v4-console-main">
        <div v-if="loading" class="v4-console-state" role="status">正在加载 V4 资料...</div>
        <div v-else-if="error" class="v4-console-state is-error" role="alert">{{ error }}</div>

        <main class="v4-console-content">
          <slot />
        </main>
      </div>
    </div>
  </section>
</template>
```

- [ ] **Step 3: Rewrite styles**

Replace the entire `<style scoped>` section:

```vue
<style scoped>
.v4-console {
  display: grid;
  grid-template-rows: auto 1fr;
  min-height: 100%;
  background: var(--v4-bg);
  color: var(--v4-ink);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  line-height: var(--v4-line-height);
}

.v4-top-bar {
  display: grid;
  gap: 8px;
  padding: 10px;
  border-bottom: 1px solid var(--v4-line);
  background: var(--v4-shell-top-bg);
}

.v4-console-body {
  display: grid;
  grid-template-columns: 116px 1fr;
  min-height: 0;
}

.v4-console-rail {
  border-right: 1px solid var(--v4-line);
  background: var(--v4-rail-bg);
  padding: 8px;
  display: grid;
  align-content: start;
  gap: 6px;
  position: sticky;
  top: 0;
  min-height: 100%;
}

.v4-console-main {
  min-width: 0;
  padding: 10px;
  background: var(--v4-bg);
}

.v4-console-state {
  margin-bottom: 10px;
  padding: 10px;
  color: var(--v4-muted);
  font-weight: 750;
  font-size: 13px;
}

.v4-console-state.is-error {
  color: var(--v4-danger);
}

.v4-console-content {
  min-width: 0;
}

@media (max-width: 820px) {
  .v4-console-body {
    grid-template-columns: 1fr;
  }

  .v4-console-rail {
    position: static;
    overflow: hidden;
    border-right: 0;
    border-bottom: 1px solid var(--v4-line);
  }
}
</style>
```

- [ ] **Step 4: Update tests if needed**

Read `V4ConsoleShell.test.ts`. Update any test that checks for old class names (`.v4-console-shell-inner`, `.v4-console-header`, etc.) to match the new class names (`.v4-console`, `.v4-top-bar`, `.v4-console-body`, `.v4-console-rail`).

- [ ] **Step 5: Verify**

Run: `cd frontend && npm test -- V4ConsoleShell`
Run: `cd frontend && npm run build`

---

### Task 3: Restyle V4DomainRail

**Files:**
- Modify: `frontend/src/components/reader/v4/V4DomainRail.vue`

**Interfaces:**
- Consumes: `--v4-*` tokens from Task 1
- Produces: Same props interface (`items`, `activeKey`, `select` event) — no breaking change

**Reference:** HTML mockup lines 198-229 (`.domain-rail` and `.nav-row`)

- [ ] **Step 1: Rewrite styles in `V4DomainRail.vue`**

Replace the entire `<style scoped>` section:

```vue
<style scoped>
.v4-domain-rail {
  display: grid;
  align-content: start;
  gap: 6px;
}

.v4-domain-rail-item {
  display: grid;
  gap: 1px;
  width: 100%;
  min-height: 38px;
  padding: 6px 7px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  text-align: left;
  cursor: pointer;
}

.v4-domain-rail-item.active {
  border-color: var(--v4-accent);
  color: var(--v4-accent);
  font-weight: 850;
}

.v4-domain-rail-item:hover:not(.active) {
  background: var(--v4-soft);
}

.v4-domain-rail-label {
  font-size: 12px;
  font-weight: 800;
  line-height: 1.2;
}

.v4-domain-rail-meta {
  overflow: hidden;
  color: var(--v4-muted);
  font-size: 10px;
  font-weight: 500;
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.v4-domain-rail-item.active .v4-domain-rail-meta {
  color: var(--v4-accent);
  opacity: 0.8;
}

/* tone modifiers for meta color */
.v4-domain-rail-item.tone-warning .v4-domain-rail-meta { color: var(--v4-warn); }
.v4-domain-rail-item.tone-danger .v4-domain-rail-meta { color: var(--v4-danger); }
.v4-domain-rail-item.tone-success .v4-domain-rail-meta { color: var(--v4-success); }
.v4-domain-rail-item.tone-active .v4-domain-rail-meta { color: var(--v4-accent); }

@media (max-width: 820px) {
  .v4-domain-rail {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .v4-domain-rail-item {
    min-width: 80px;
  }
}
</style>
```

- [ ] **Step 2: Verify**

Run: `cd frontend && npm run build`

---

### Task 4: Strip V4PanelShell Border/Radius

**Files:**
- Modify: `frontend/src/components/reader/v4/V4PanelShell.vue`
- Modify: `frontend/src/components/reader/v4/V4PanelShell.test.ts`

**Interfaces:**
- Consumes: `--v4-*` tokens from Task 1
- Produces: Same props interface — no breaking change

**Reference:** HTML mockup lines 1220-1251 (`.screen-header` — panel header with title left + toolbar right)

- [ ] **Step 1: Read current `V4PanelShell.vue`**

- [ ] **Step 2: Rewrite styles**

Replace the entire `<style scoped>` section. Remove `.v4-panel-shell` border/radius/background:

```vue
<style scoped>
.v4-panel-shell {
  min-height: 100%;
  background: var(--v4-bg);
  color: var(--v4-ink);
}

.v4-panel-shell-inner {
  min-height: 100%;
  padding: 14px;
}

.v4-panel-shell-header {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 14px;
  align-items: center;
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--v4-line);
}

.v4-panel-shell-header h2,
.v4-panel-shell-header p {
  margin: 0;
}

.v4-panel-shell-header h2 {
  color: var(--v4-ink);
  font-size: 18px;
  font-weight: 850;
  letter-spacing: 0;
}

.v4-panel-shell-header p {
  margin-top: 5px;
  color: var(--v4-muted);
  line-height: 1.6;
  font-size: 13px;
}

.v4-panel-shell-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

@media (max-width: 768px) {
  .v4-panel-shell-header {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
```

- [ ] **Step 3: Update tests**

Read `V4PanelShell.test.ts`. Update any assertion checking for `.v4-panel-shell` having border/radius to check for absence.

- [ ] **Step 4: Verify**

Run: `cd frontend && npm test -- V4PanelShell`
Run: `cd frontend && npm run build`

---

### Task 5: Restructure AiBookV4View (Masthead + Rail Meta)

**Files:**
- Modify: `frontend/src/views/AiBookV4View.vue`
- Modify: `frontend/src/views/AiBookV4View.test.ts`

**Interfaces:**
- Consumes: New shell layout from Task 2, rail from Task 3, `--v4-*` tokens from Task 1
- Produces: `railItems` with computed meta; masthead merged into shell top-bar

**Reference:** HTML mockup lines 148-191 (shell top-bar + rail with meta labels)

- [ ] **Step 1: Read current `AiBookV4View.vue`**

- [ ] **Step 2: Update `railItems` to include computed meta**

Replace the static `railItems` array with a `computed` that includes meta labels:

```ts
const railItems = computed(() => [
  { key: 'overview' as const, label: '总览', meta: 'health' },
  {
    key: 'task' as const,
    label: '任务',
    meta: catchupStatus.value?.status === 'running'
      ? `running · ${catchupProgress.value}%`
      : 'idle',
  },
  {
    key: 'characters' as const,
    label: '角色',
    meta: `${memoryStatus.value?.characterCount ?? '?'} canonical`,
  },
  {
    key: 'relationships' as const,
    label: '关系',
    meta: `${memoryStatus.value?.relationshipCount ?? '?'} edges`,
  },
  {
    key: 'knowledge' as const,
    label: '知识',
    meta: `${memoryStatus.value?.knowledgeCount ?? '?'} facts`,
  },
  {
    key: 'map' as const,
    label: '地点',
    meta: `${memoryStatus.value?.placeCount ?? '?'} places`,
  },
  {
    key: 'quality' as const,
    label: '质量',
    meta: 'queues',
  },
])
```

Add `catchupProgress` computed:

```ts
const catchupProgress = computed(() => {
  const c = catchupStatus.value
  if (!c || !c.targetChapter || c.targetChapter <= 0) return 0
  return Math.round(((c.currentChapter ?? 0) / c.targetChapter) * 100)
})
```

Note: `memoryStatus.value` fields like `characterCount`, `relationshipCount`, `knowledgeCount`, `placeCount` — verify these exist on `V4MemoryStatusResponse`. If not, use a simpler label (e.g. just the domain name without count).

- [ ] **Step 3: Restructure masthead in template**

Replace the `#header` slot content inside `V4ConsoleShell`. The new masthead is a flat grid (not a card):

```vue
<template #header>
  <div class="v4-masthead" data-test="v4-console-masthead">
    <div class="v4-masthead-title">
      <button class="v4-masthead-back" type="button" @click="goBack">←</button>
      <div>
        <strong class="v4-masthead-name">{{ bookTitle }} · V4 Memory Console</strong>
        <span class="v4-masthead-sub">{{ bookAuthor }} · {{ processingSummary }}</span>
      </div>
    </div>
    <div class="v4-masthead-actions">
      <span class="v4-pill">已读 {{ formatChapterNumber(memoryStatus?.maxReadChapter) }}</span>
      <span class="v4-pill">已处理 {{ formatChapterNumber(memoryStatus?.maxProcessedChapter) }}</span>
      <span class="v4-pill blue">当前 {{ currentChapterLabel }}</span>
      <button class="v4-action-btn" type="button" :disabled="isAnyActionBusy" @click="refreshStatus">刷新状态</button>
      <button class="v4-action-btn" type="button" :disabled="isAnyActionBusy || !hasCurrentChapter" @click="generateCurrentChapter">生成当前章节</button>
      <button class="v4-action-btn primary" type="button" :disabled="isAnyActionBusy || !hasCurrentChapter || isV4Processing" @click="catchupToCurrentReading">
        {{ isV4Processing ? '补齐运行中' : '补齐到当前阅读' }}
      </button>
      <button class="v4-action-btn" type="button" :disabled="isAnyActionBusy" @click="resetMemory" style="color: var(--v4-danger);">重置</button>
    </div>
    <p v-if="actionMessage" class="v4-masthead-msg" role="status">{{ actionMessage }}</p>
    <p v-else-if="actionError" class="v4-masthead-msg is-error" role="alert">{{ actionError }}</p>
  </div>
</template>
```

- [ ] **Step 4: Replace styles**

Remove all old `.v4-*` scoped styles and replace with new masthead styles:

```vue
<style scoped>
.ai-v4-view {
  height: 100%;
  min-height: 100%;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: var(--v4-bg);
  color: var(--v4-ink);
}

.v4-shell {
  min-height: 100%;
}

.v4-masthead {
  display: grid;
  gap: 8px;
}

.v4-masthead-title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.v4-masthead-name {
  font-size: 14px;
  font-weight: 850;
  letter-spacing: 0.02em;
}

.v4-masthead-sub {
  display: block;
  margin-top: 2px;
  color: var(--v4-muted);
  font-size: 12px;
}

.v4-masthead-back {
  width: 28px;
  height: 24px;
  display: grid;
  place-items: center;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  color: var(--v4-muted);
  font-size: 14px;
  cursor: pointer;
}

.v4-masthead-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.v4-masthead-msg {
  margin: 0;
  color: var(--v4-muted);
  font-size: 12px;
}

.v4-masthead-msg.is-error {
  color: var(--v4-danger);
}

.v4-empty-shell {
  max-width: 560px;
  margin: 72px auto;
  display: grid;
  gap: 16px;
  padding: 22px;
}

.v4-empty-shell h1 {
  margin: 0;
  font-size: 24px;
  line-height: 1.08;
}

.v4-empty-shell p {
  margin: 0;
  color: var(--v4-muted);
  line-height: 1.7;
}
</style>
```

- [ ] **Step 5: Update tests**

Read `AiBookV4View.test.ts`. Update any test that checks for `.v4-console-masthead` inner structure, old `.v4-status-chip`, `.v4-action-button` class names to match new class names (`.v4-pill`, `.v4-action-btn`, `.v4-masthead-title`, `.v4-masthead-actions`).

- [ ] **Step 6: Verify**

Run: `cd frontend && npm test -- AiBookV4View`
Run: `cd frontend && npm run build`

---

### Task 6: Redesign V4BookOverviewPanel

**Files:**
- Modify: `frontend/src/components/reader/V4BookOverviewPanel.vue`
- Modify: `frontend/src/components/reader/V4BookOverviewPanel.test.ts`

**Interfaces:**
- Consumes: `bookUrl` prop, `--v4-*` tokens, APIs unchanged
- Produces: Same data shape (no prop/emit changes)

**Reference:** HTML mockup lines 2260-2352 (Overview screen with live-core, domain-health-grid, attention-list)

- [ ] **Step 1: Read current `V4BookOverviewPanel.vue`**

- [ ] **Step 2: Rewrite template**

Replace the inner `<div v-if="overview" class="v4-overview">` section with the new 3-part layout:

```vue
<div v-if="overview" class="v4-overview">
  <!-- 1. Live Core: progress bar (only when task running or recent) -->
  <section v-if="hasLiveData" class="v4-live-core" aria-label="实时任务" data-test="overview-live-task">
    <div class="v4-core-row">
      <div>
        <strong>{{ livePercent }}%</strong>
        <span>{{ liveTaskDetail }}</span>
      </div>
      <span class="v4-pill blue">{{ liveStageLabel }}</span>
    </div>
    <div class="v4-progress-track">
      <div class="v4-progress-fill" :style="{ width: `${livePercent}%` }"></div>
    </div>
    <div class="v4-core-row">
      <span>{{ liveRecentLabel }}</span>
      <span v-if="liveFailLabel">{{ liveFailLabel }}</span>
    </div>
  </section>

  <!-- 2. overview-grid: Domain Health + Attention -->
  <div class="v4-overview-grid">
    <!-- Domain Health -->
    <section class="v4-wide-panel" data-test="overview-domain-health">
      <div class="v4-panel-title">
        <h3>Domain Health</h3>
        <span class="v4-pill green">canonical ready</span>
      </div>
      <div class="v4-domain-health-grid">
        <div class="v4-domain-card">
          <span>角色</span>
          <strong>{{ countLabel(overview.memory?.characterCount) }}</strong>
          <span>{{ domainHint('character') }}</span>
          <div class="v4-spark"></div>
        </div>
        <div class="v4-domain-card">
          <span>关系</span>
          <strong>{{ countLabel(overview.memory?.relationshipCount) }}</strong>
          <span>{{ domainHint('relationship') }}</span>
          <div class="v4-spark"></div>
        </div>
        <div class="v4-domain-card">
          <span>知识</span>
          <strong>{{ countLabel(overview.memory?.knowledgeCount) }}</strong>
          <span>{{ domainHint('knowledge') }}</span>
          <div class="v4-spark"></div>
        </div>
        <div class="v4-domain-card">
          <span>地点</span>
          <strong>{{ overview.mapError ? '—' : countLabel(overview.mapOverview?.placeCount) }}</strong>
          <span>{{ overview.mapError ? '打开地图页查看' : domainHint('place') }}</span>
          <div class="v4-spark"></div>
        </div>
      </div>
    </section>

    <!-- Attention List -->
    <aside class="v4-wide-panel" data-test="overview-attention-list">
      <div class="v4-panel-title">
        <h3>需要注意</h3>
        <span class="v4-pill amber">{{ attentionItems.length }} items</span>
      </div>
      <div class="v4-attention-list">
        <div v-for="item in attentionItems" :key="item.key" class="v4-attention-row">
          <span :class="['v4-status-dot', item.tone]"></span>
          <strong>{{ item.label }}</strong>
          <span class="v4-chip">{{ item.tag }}</span>
        </div>
        <p v-if="!attentionItems.length" class="v4-no-attention">暂无高优先级提醒</p>
      </div>
    </aside>
  </div>
</div>
```

- [ ] **Step 3: Rewrite computed properties**

Add new computed properties for the template:

```ts
const hasLiveData = computed(() => {
  const c = overview.value?.catchup
  return c?.status === 'running' || c?.status === 'failed' || Boolean(overview.value?.status?.processing)
})

const livePercent = computed(() => {
  const c = overview.value?.catchup
  if (!c || !c.targetChapter || c.targetChapter <= 0) return 0
  return Math.round(((c.currentChapter ?? 0) / c.targetChapter) * 100)
})

const liveStageLabel = computed(() => {
  const c = overview.value?.catchup
  if (c?.status === 'running') return 'running'
  if (c?.status === 'failed') return 'failed'
  return 'idle'
})

const liveRecentLabel = computed(() => {
  const c = overview.value?.catchup
  if (c?.status === 'running') return `当前 ${formatChapter(c.currentChapter)} · 目标 ${formatChapter(c.targetChapter)}`
  if (c?.status === 'completed') return `已处理到 ${formatChapter(c.maxProcessedChapter)}`
  return `已处理到 ${formatChapter(overview.value?.status?.maxProcessedChapter ?? overview.value?.memory?.maxProcessedChapter)}`
})

const liveFailLabel = computed(() => {
  const err = overview.value?.status?.lastError
  return err ? '1 个历史失败可重试' : ''
})

const attentionItems = computed(() => {
  const items: Array<{ key: string; tone: string; label: string; tag: string }> = []
  const c = overview.value?.catchup
  if (c?.status === 'running') items.push({ key: 'task', tone: 'blue', label: `第 ${c.currentChapter ?? '?'} 章正在处理`, tag: '任务' })
  const err = overview.value?.status?.lastError
  if (err) items.push({ key: 'error', tone: 'red', label: '最近处理失败', tag: '重试' })
  if (overview.value?.mapOverview && overview.value.mapOverview.conflictCount > 0) items.push({ key: 'map', tone: 'amber', label: `地图冲突 ${overview.value.mapOverview.conflictCount}`, tag: '地点' })
  const qf = overview.value?.quality?.findings?.length ?? 0
  if (qf > 0) items.push({ key: 'quality', tone: 'amber', label: `质量发现 ${qf}`, tag: '质量' })
  return items
})

function domainHint(domain: string): string {
  // Placeholder hints — can be refined with real data later
  return ''
}
```

- [ ] **Step 4: Replace styles**

Remove all old `<style scoped>` content and replace:

```vue
<style scoped>
.v4-overview {
  display: grid;
  gap: 16px;
}

/* === Live Core === */
.v4-live-core {
  border: 1px solid var(--v4-line);
  background: var(--v4-inspector-bg);
  padding: 12px;
  display: grid;
  gap: 10px;
  position: relative;
  overflow: hidden;
}

.v4-live-core::before {
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--v4-accent) 12%, transparent), transparent);
  transform: translateX(-100%);
  animation: v4-scan 2.8s ease-in-out infinite;
  pointer-events: none;
}

.v4-core-row {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  align-items: baseline;
  position: relative;
  z-index: 1;
}

.v4-core-row strong {
  font-size: 24px;
  line-height: 1;
  letter-spacing: -0.03em;
}

.v4-core-row span {
  color: var(--v4-muted);
  font-size: 11px;
  font-weight: 750;
}

.v4-progress-track {
  height: 9px;
  border: 1px solid var(--v4-line);
  background: var(--v4-soft);
  position: relative;
  overflow: hidden;
  z-index: 1;
}

.v4-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--v4-accent), var(--v4-purple));
  transition: width 300ms ease;
}

.v4-progress-fill::after {
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg, transparent, rgba(255,255,255,0.55), transparent);
  transform: translateX(-100%);
  animation: v4-shimmer 1.8s linear infinite;
}

/* === Overview Grid === */
.v4-overview-grid {
  display: grid;
  grid-template-columns: 1.3fr 0.9fr;
  gap: 12px;
  align-items: start;
}

.v4-wide-panel {
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  padding: 12px;
  display: grid;
  gap: 10px;
  min-width: 0;
}

.v4-panel-title {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: center;
  margin-bottom: 8px;
}

.v4-panel-title h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 850;
}

/* === Domain Health === */
.v4-domain-health-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

.v4-domain-card {
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  padding: 8px;
  min-height: 80px;
  display: grid;
  align-content: space-between;
  gap: 4px;
}

.v4-domain-card span {
  color: var(--v4-muted);
  font-size: 11px;
}

.v4-domain-card strong {
  display: block;
  font-size: 20px;
  line-height: 1;
}

.v4-spark {
  height: 20px;
  border: 1px solid var(--v4-line);
  background:
    linear-gradient(135deg, transparent 0 17%, color-mix(in srgb, var(--v4-accent) 36%, transparent) 18% 19%, transparent 20% 42%, color-mix(in srgb, var(--v4-success) 44%, transparent) 43% 45%, transparent 46% 68%, color-mix(in srgb, var(--v4-warn) 38%, transparent) 69% 70%, transparent 71%);
}

/* === Attention List === */
.v4-attention-list {
  display: grid;
  gap: 8px;
}

.v4-attention-row {
  border: 1px solid var(--v4-line);
  background: var(--v4-inspector-bg);
  padding: 9px;
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: 9px;
  align-items: center;
  font-size: 12px;
}

.v4-status-dot {
  width: 11px;
  height: 11px;
  border: 2px solid var(--v4-line);
  background: var(--v4-soft);
}

.v4-status-dot.blue { background: var(--v4-accent); border-color: var(--v4-accent); animation: v4-pulse 1.2s ease-in-out infinite; }
.v4-status-dot.green { background: var(--v4-success); border-color: var(--v4-success); }
.v4-status-dot.amber { background: var(--v4-warn); border-color: var(--v4-warn); }
.v4-status-dot.red { background: var(--v4-danger); border-color: var(--v4-danger); }

.v4-chip {
  display: inline-flex;
  align-items: center;
  min-height: 19px;
  padding: 0 6px;
  border: 1px solid var(--v4-line);
  background: var(--v4-surface);
  font-size: 10px;
  color: var(--v4-muted);
  font-weight: 750;
}

.v4-no-attention {
  margin: 0;
  color: var(--v4-muted);
  font-size: 12px;
}

@media (max-width: 900px) {
  .v4-overview-grid {
    grid-template-columns: 1fr;
  }

  .v4-domain-health-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
```

- [ ] **Step 5: Update tests**

Read `V4BookOverviewPanel.test.ts`. Update selectors for old class names (`.v4-live-task-card`, `.v4-domain-grid`, `.v4-attention-panel`) to match new names (`.v4-live-core`, `.v4-domain-health-grid`, `.v4-attention-list`).

- [ ] **Step 6: Verify**

Run: `cd frontend && npm test -- V4BookOverviewPanel`
Run: `cd frontend && npm run build`

---

### Task 7: Redesign V4TaskProgressPanel

**Files:**
- Modify: `frontend/src/components/reader/V4TaskProgressPanel.vue`
- Modify: `frontend/src/components/reader/V4TaskProgressPanel.test.ts`

**Interfaces:**
- Consumes: `bookUrl` prop, `--v4-*` tokens, APIs unchanged
- Produces: Same data shape

**Reference:** HTML mockup lines 2354-2440 (Task Progress screen) + lines 2120-2252 (recommended hybrid with pipeline + error inspector + heatmap)

- [ ] **Step 1: Read current `V4TaskProgressPanel.vue`**

- [ ] **Step 2: Rewrite template with 3-part layout**

Replace the inner `<section class="v4-task-progress">` with:

```vue
<section class="v4-task-progress" data-test="v4-task-panel">
  <!-- 1. Hero: dual-column -->
  <div class="v4-task-hero">
    <div class="v4-task-hero-main">
      <h2>{{ taskTitle }}</h2>
      <p>{{ taskSummary }}</p>
      <div class="v4-action-row">
        <button class="v4-action-btn primary" type="button" @click="reload">刷新状态</button>
      </div>
    </div>
    <aside class="v4-task-hero-meter">
      <span>本次任务</span>
      <strong>{{ progressPercent }}%</strong>
      <div class="v4-progress-track"><div class="v4-progress-fill" :style="{ width: `${progressPercent}%` }"></div></div>
      <span>{{ progressTimeLabel }}</span>
    </aside>
  </div>

  <!-- 2. Pipeline + Inspector -->
  <div class="v4-task-main-grid">
    <section class="v4-pipeline-large">
      <h3>Pipeline Timeline</h3>
      <article v-for="stage in stages" :key="stage.key" :class="['v4-stage-large', stage.status]" :data-test="`task-stage-${stage.key}`">
        <span class="v4-stage-dot"></span>
        <div>
          <strong>{{ stage.label }}</strong>
          <span>{{ stage.description }}</span>
        </div>
        <small>{{ stage.statusLabel }}</small>
      </article>
    </section>

    <aside class="v4-error-inspector">
      <div class="v4-error-title">
        <h3>Stage Inspector</h3>
        <span class="v4-pill blue">{{ currentStageLabel }}</span>
      </div>
      <div v-for="field in inspectorFields" :key="field.label" class="v4-detail-line">
        <span>{{ field.label }}</span>
        <strong>{{ field.value }}</strong>
      </div>
      <div v-if="lastError" class="v4-error-code">{{ lastError }}</div>
      <div class="v4-action-row">
        <button class="v4-action-btn primary" type="button" @click="reload">查看状态</button>
      </div>
    </aside>
  </div>

  <!-- 3. Mini Heatmap -->
  <section class="v4-heatmap-wide">
    <h3>Compressed Chapter Navigator</h3>
    <div class="v4-heatmap-grid" aria-label="章节处理进度">
      <span v-for="cell in heatmapCells" :key="cell.index" :class="['v4-heatmap-cell', cell.status]"></span>
    </div>
    <div class="v4-heatmap-legend">
      <span>绿色成功 · 蓝色当前 · 红色失败 · 灰色等待</span>
    </div>
  </section>
</section>
```

- [ ] **Step 3: Add computed properties for heatmap and stages**

```ts
const heatmapCells = computed(() => {
  const maxProcessed = data.value?.memory.maxProcessedChapter ?? 0
  const current = data.value?.memory.maxReadChapter ?? 0
  const target = data.value?.catchup.targetChapter ?? current
  const total = Math.max(target, 32)
  const cells: Array<{ index: number; status: 'done' | 'active' | 'failed' | 'pending' }> = []
  const hasError = Boolean(data.value?.memory.lastError)
  const errorChapter = hasError ? maxProcessed - 1 : -1
  for (let i = 0; i < Math.min(total, 64); i++) {
    if (i < maxProcessed && i !== errorChapter) cells.push({ index: i, status: 'done' })
    else if (i === errorChapter) cells.push({ index: i, status: 'failed' })
    else if (i === maxProcessed && data.value?.catchup.status === 'running') cells.push({ index: i, status: 'active' })
    else cells.push({ index: i, status: 'pending' })
  }
  return cells
})

const inspectorFields = computed(() => {
  const d = data.value
  if (!d) return []
  return [
    { label: '状态', value: d.catchup.status },
    { label: '当前章节', value: formatChapterNumber(d.catchup.currentChapter) },
    { label: '目标章节', value: formatChapterNumber(d.catchup.targetChapter) },
    { label: '已处理', value: formatChapterNumber(d.memory.maxProcessedChapter) },
  ]
})
```

- [ ] **Step 4: Replace all styles**

Remove old `<style scoped>` and replace with styles from the design spec (task-hero, pipeline-large, stage-large, error-inspector, heatmap-wide, progress-track/fill, etc.).

- [ ] **Step 5: Update tests**

Update selectors from old class names (`.v4-task-progress-hero`, `.v4-task-metrics`, `.v4-task-stage-panel`) to new names (`.v4-task-hero`, `.v4-task-main-grid`, `.v4-pipeline-large`, `.v4-heatmap-wide`).

- [ ] **Step 6: Verify**

Run: `cd frontend && npm test -- V4TaskProgressPanel`
Run: `cd frontend && npm run build`

---

### Task 8: Redesign V4CharacterPanel (+ Search + Identity Thread)

**Files:**
- Modify: `frontend/src/components/reader/V4CharacterPanel.vue`
- Modify: `frontend/src/components/reader/V4CharacterPanel.test.ts`

**Interfaces:**
- Consumes: `bookUrl` prop, `--v4-*` tokens, APIs unchanged
- Produces: Same data shape

**Reference:** HTML mockup lines 2442-2524 (Characters screen with directory + inspector + identity-thread)

- [ ] **Step 1: Read current `V4CharacterPanel.vue`**

- [ ] **Step 2: Add search filter**

Add a `searchQuery` ref and a `filteredCharacters` computed:

```ts
const searchQuery = shallowRef('')

const filteredCharacters = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return characters.value
  return characters.value.filter(c =>
    c.name.toLowerCase().includes(q) ||
    c.aliases?.some(a => a.toLowerCase().includes(q))
  )
})
```

Replace `visibleCharacters` to use `filteredCharacters`:

```ts
const visibleCharacters = computed(() => {
  const list = filteredCharacters.value
  if (showAllCharacters.value || list.length <= DEFAULT_VISIBLE_CHARACTER_COUNT) return list
  return list.slice(0, DEFAULT_VISIBLE_CHARACTER_COUNT)
})
```

- [ ] **Step 3: Add search bar to template**

Insert above the character grid:

```vue
<div class="v4-character-search">
  <input
    v-model="searchQuery"
    type="text"
    placeholder="搜索角色 / 别名"
    class="v4-search-input"
  />
</div>
```

- [ ] **Step 4: Rewrite character list as compact rows**

Replace the `.v4-character-grid` with row-style items (no avatar circle):

```vue
<div class="v4-character-rows">
  <button
    v-for="character in visibleCharacters"
    :key="character.id"
    type="button"
    :class="['v4-character-row', { active: selectedCharacterId === character.id }]"
    :data-character-id="character.id"
    @click="selectCharacter(character.id)"
  >
    <div class="v4-row-main">
      <span class="v4-row-name">{{ character.name }}</span>
      <span :class="['v4-chip', importanceChipClass(character.importance)]">{{ importanceLabel(character.importance) }}</span>
    </div>
    <div class="v4-row-meta">
      别名 {{ character.aliases?.length ?? 0 }} · 首见 {{ formatChapter(character.firstSeenChapter) }} · 最近 {{ formatChapter(character.lastSeenChapter) }}
    </div>
  </button>
  <button v-if="hiddenCharacterCount > 0" type="button" class="v4-character-toggle" @click="showAllCharacters = !showAllCharacters">
    {{ showAllCharacters ? '收起' : `显示其余 ${hiddenCharacterCount} 位角色` }}
  </button>
</div>
```

- [ ] **Step 5: Add identity-thread section to detail**

After the state matrix, add:

```vue
<section v-if="detail" class="v4-identity-thread">
  <h3>身份线索</h3>
  <div class="v4-detail-line">
    <span>稳定链接</span>
    <strong>{{ identityStableLabel }}</strong>
  </div>
  <div class="v4-detail-line">
    <span>待确认</span>
    <strong>{{ identityPendingLabel }}</strong>
  </div>
  <div class="v4-detail-line">
    <span>处理入口</span>
    <strong>打开证据 / 送入质量治理</strong>
  </div>
</section>
```

Add computed stubs:

```ts
const identityStableLabel = computed(() => '—')
const identityPendingLabel = computed(() => '—')
```

- [ ] **Step 6: Rewrite styles**

Replace all `<style scoped>` with design-spec styles: compact rows, warm inspector bg, identity-thread (purple border), evidence (green left border), search input, field-grid.

- [ ] **Step 7: Update tests**

Update selectors for old class names and add tests for search filtering and identity-thread rendering.

- [ ] **Step 8: Verify**

Run: `cd frontend && npm test -- V4CharacterPanel`
Run: `cd frontend && npm run build`

---

### Task 9: Redesign V4RelationshipPanel (+ Visual Graph)

**Files:**
- Modify: `frontend/src/components/reader/V4RelationshipPanel.vue`
- Modify: `frontend/src/components/reader/V4RelationshipPanel.test.ts`

**Interfaces:**
- Consumes: `bookUrl` prop, `--v4-*` tokens, APIs unchanged
- Produces: Same data shape

**Reference:** HTML mockup lines 2526-2596 (Relationships screen with graph-panel + detail-panel)

- [ ] **Step 1: Read current `V4RelationshipPanel.vue`**

- [ ] **Step 2: Add a simple deterministic graph layout composable**

Add a `computed` that converts `nodes` and `edges` into positioned elements. Use a simple circular layout for the first pass:

```ts
const graphNodes = computed(() => {
  const nodes = listState.data.value?.nodes ?? []
  if (!nodes.length) return []
  const cx = 240, cy = 200, r = 140
  return nodes.map((node, i) => {
    const angle = (2 * Math.PI * i) / nodes.length - Math.PI / 2
    return {
      ...node,
      x: cx + r * Math.cos(angle),
      y: cy + r * Math.sin(angle),
    }
  })
})

const graphEdges = computed(() => {
  const edges = listState.data.value?.edges ?? []
  const nodeMap = new Map(graphNodes.value.map(n => [n.id, n]))
  return edges.map(edge => {
    const src = nodeMap.get(edge.sourceId)
    const tgt = nodeMap.get(edge.targetId)
    if (!src || !tgt) return null
    const dx = tgt.x - src.x, dy = tgt.y - src.y
    const len = Math.sqrt(dx * dx + dy * dy)
    const angle = Math.atan2(dy, dx) * (180 / Math.PI)
    return { ...edge, x: src.x, y: src.y, len, angle }
  }).filter(Boolean)
})
```

- [ ] **Step 3: Replace center column with graph visualization**

Replace the node-chip cloud with a positioned graph:

```vue
<div class="v4-graph-panel">
  <div
    v-for="node in graphNodes"
    :key="node.id"
    class="v4-graph-node"
    :style="{ left: `${node.x}px`, top: `${node.y}px` }"
    @click="selectEdgeByNode(node.id)"
  >
    {{ node.name }}
  </div>
  <span
    v-for="edge in graphEdges"
    :key="edge.id"
    class="v4-edge-line"
    :style="{ left: `${edge.x}px`, top: `${edge.y}px`, width: `${edge.len}px`, transform: `rotate(${edge.angle}deg)` }"
  ></span>
  <span
    v-for="edge in graphEdges"
    :key="`label-${edge.id}`"
    class="v4-edge-label"
    :style="{ left: `${edge.x + edge.len / 2 - 30}px`, top: `${edge.y - 8}px` }"
  >
    {{ edge.label }} · {{ (edge.confidence * 100).toFixed(0) }}%
  </span>
</div>
```

- [ ] **Step 4: Rewrite styles**

Replace `<style scoped>` with design-spec graph-panel styles (grid background, positioned nodes, edge-line, edge-label) + detail-panel warm bg + compact edge list.

- [ ] **Step 5: Update tests and verify**

Run: `cd frontend && npm test -- V4RelationshipPanel`
Run: `cd frontend && npm run build`

---

### Task 10: Redesign V4KnowledgePanel (+ Category Overview)

**Files:**
- Modify: `frontend/src/components/reader/V4KnowledgePanel.vue`
- Modify: `frontend/src/components/reader/V4KnowledgePanel.test.ts`

**Interfaces:**
- Consumes: `bookUrl` prop, `--v4-*` tokens, APIs unchanged
- Produces: Same data shape

**Reference:** HTML mockup lines 2598-2686 (Knowledge screen with Knowledge Atlas + Topic Inspector)

- [ ] **Step 1: Read current `V4KnowledgePanel.vue`**

- [ ] **Step 2: Add category overview grid above topic cards**

Add a 4-column grid of category summary cards above the category nav:

```vue
<div v-if="categories.length" class="v4-category-overview">
  <div v-for="cat in topCategories" :key="cat.category" class="v4-domain-card">
    <span>{{ categoryLabel(cat.category) }}</span>
    <strong>{{ cat.count }}</strong>
    <span>{{ categoryDescription(cat.category) }}</span>
    <div class="v4-spark"></div>
  </div>
</div>
```

Add computed + helper:

```ts
const topCategories = computed(() => categories.value.slice(0, 4))

function categoryDescription(cat: string): string {
  const map: Record<string, string> = {
    power_system: '境界、功法、能力边界',
    faction: '宗门、家族、暗线组织',
    world_rule: '禁制、契约、空间规则',
    secret: '未揭示或半揭示线索',
    history: '历史事件与演变',
    geography: '地理、空间、路线',
    politics: '权力结构与博弈',
    prophecy: '预言、暗示、伏笔',
  }
  return map[cat] ?? ''
}
```

- [ ] **Step 3: Restyle to match design**

Replace `<style scoped>` with design-spec styles: knowledge-layout (2-column), category overview grid, warm inspector bg, fact-stack, spark decoration.

- [ ] **Step 4: Update tests and verify**

Run: `cd frontend && npm test -- V4KnowledgePanel`
Run: `cd frontend && npm run build`

---

### Task 11: Restyle V4MapPanel

**Files:**
- Modify: `frontend/src/components/reader/V4MapPanel.vue`
- Modify: `frontend/src/components/reader/V4MapPanel.test.ts`

**Interfaces:**
- Consumes: `bookUrl` prop, `--v4-*` tokens, APIs unchanged
- Produces: Same data shape

**Reference:** HTML mockup lines 2688-2756 (Places screen with map-panel + place inspector)

- [ ] **Step 1: Read current `V4MapPanel.vue`**

- [ ] **Step 2: Restyle all CSS**

Replace `<style scoped>` with design-spec tokens. This is primarily a CSS-only change: `--color-*` → `--v4-*`, radius reduction, border color changes, bg color changes. Keep the existing template and logic intact.

Key changes:
- `.map-sidebar` → `background: var(--v4-rail-bg)`, `border-right: 1px solid var(--v4-line)`
- `.map-topology-canvas` → keep SVG, update fill/stroke to `--v4-*` tokens
- `.map-detail-panel` → `background: var(--v4-inspector-bg)`, remove rounded corners
- `.map-place-*` → update colors to `--v4-*`

- [ ] **Step 3: Update tests and verify**

Run: `cd frontend && npm test -- V4MapPanel`
Run: `cd frontend && npm run build`

---

### Task 12: Restyle V4QualityPanel

**Files:**
- Modify: `frontend/src/components/reader/V4QualityPanel.vue`
- Modify: `frontend/src/components/reader/V4QualityPanel.test.ts`

**Interfaces:**
- Consumes: `bookUrl` prop, `--v4-*` tokens, APIs unchanged
- Produces: Same data shape

**Reference:** HTML mockup lines 2758-2876 (Quality screen with queues + triage + review inspector)

- [ ] **Step 1: Read current `V4QualityPanel.vue`**

- [ ] **Step 2: Restyle all CSS**

Replace `<style scoped>` with design-spec tokens. Structure is already aligned — this is primarily a CSS token swap + radius/spacing adjustment:
- `--color-*` → `--v4-*` everywhere
- Remove border-radius on all elements (set to 0)
- Queue panel bg → `var(--v4-inspector-bg)`
- Triage severity badges → use `--v4-danger`/`--v4-warn` colors
- Inspector → `border: 1px solid color-mix(in srgb, var(--v4-danger) 38%, var(--v4-line))`
- Error code block → monospace, `var(--v4-danger)` text

- [ ] **Step 3: Update tests and verify**

Run: `cd frontend && npm test -- V4QualityPanel`
Run: `cd frontend && npm run build`

---

### Task 13: Final Integration Test

**Files:** None (verification only)

- [ ] **Step 1: Run all V4 tests**

Run: `cd frontend && npm test -- V4`
Expected: All tests pass.

- [ ] **Step 2: Run full build**

Run: `cd frontend && npm run build`
Expected: Build succeeds with no errors.

- [ ] **Step 3: Check for global token leaks**

Run: `grep -r "var(--color-" frontend/src/components/reader/v4/ frontend/src/components/reader/V4*.vue frontend/src/views/AiBookV4View.vue`
Expected: No matches (all V4 components use `--v4-*` tokens only).

- [ ] **Step 4: Check for leftover border-radius**

Run: `grep -r "border-radius" frontend/src/components/reader/v4/ frontend/src/components/reader/V4*.vue frontend/src/views/AiBookV4View.vue`
Expected: No matches or only `0px` values.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/styles/v4-console.css frontend/src/main.ts frontend/src/components/reader/v4/ frontend/src/components/reader/V4*.vue frontend/src/views/AiBookV4View.vue
git commit -m "feat(v4): redesign console to warm-tone memory database style"
```
