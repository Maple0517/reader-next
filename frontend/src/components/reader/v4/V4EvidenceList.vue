<template>
  <div v-if="items.length" class="v4-evidence-list">
    <button type="button" class="v4-evidence-toggle" @click="expanded = !expanded">
      {{ expanded ? '收起证据' : `查看证据 (${items.length})` }}
    </button>
    <ul v-if="expanded">
      <li v-for="item in items" :key="evidenceKey(item)">
        <strong>{{ chapterLabel(item) }}</strong>
        <p>{{ item.textExcerpt || item.note || '暂无摘录' }}</p>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

export type V4EvidenceDisplayItem = {
  id?: string
  chapterIndex?: number | null
  chapterTitle?: string | null
  textExcerpt?: string | null
  note?: string | null
}

defineProps<{
  items: V4EvidenceDisplayItem[]
}>()

const expanded = ref(false)

function evidenceKey(item: V4EvidenceDisplayItem) {
  return item.id || `${item.chapterIndex ?? 'unknown'}-${item.textExcerpt || item.note || 'empty'}`
}

function chapterLabel(item: V4EvidenceDisplayItem) {
  if (item.chapterTitle) return item.chapterTitle
  return typeof item.chapterIndex === 'number' ? `第 ${item.chapterIndex + 1} 章` : '未知章节'
}
</script>

<style scoped>
.v4-evidence-list {
  display: grid;
  gap: 10px;
  margin-top: 12px;
}

.v4-evidence-toggle {
  justify-self: start;
  min-height: 32px;
  padding: 0 12px;
  border: 0;
  border-radius: 999px;
  background: color-mix(in srgb, var(--color-primary) 12%, var(--color-bg));
  color: var(--color-primary);
  font-weight: 800;
  cursor: pointer;
  transition: transform 220ms cubic-bezier(0.32, 0.72, 0, 1);
}

.v4-evidence-toggle:active {
  transform: translateY(1px) scale(0.98);
}

.v4-evidence-list ul {
  display: grid;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.v4-evidence-list li {
  padding: 12px;
  border-radius: 14px;
  background: color-mix(in srgb, var(--color-bg-sunken) 80%, transparent);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-border) 64%, transparent);
}

.v4-evidence-list strong {
  display: block;
  color: var(--color-text);
  font-size: 12px;
}

.v4-evidence-list p {
  margin: 5px 0 0;
  color: var(--color-text-secondary);
  line-height: 1.6;
}
</style>
