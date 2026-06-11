<template>
  <div class="ai-book-view">
    <div v-if="loading" class="ai-loading">
      <div class="loading-spinner"></div>
      <span>加载中...</span>
    </div>

    <div v-else-if="book && memory" class="ai-shell">
      <header class="ai-header">
        <div class="title-stack">
          <div class="title-row">
            <button class="back-btn" @click="goBack">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="m15 18-6-6 6-6" />
              </svg>
              返回
            </button>
            <h1>{{ book.name }}</h1>
          </div>
          <p>{{ book.author || '未知作者' }} · {{ progressText }}</p>
        </div>

        <div class="header-actions">
          <label class="enable-switch">
            <input type="checkbox" :checked="memory.enabled" @change="toggleEnabled" />
            <span></span>
            自动更新
          </label>
          <button class="primary-btn" :disabled="aiStore.isBusy" @click="updateToCurrent">
            {{ aiStore.phase === 'text' ? '更新中...' : '更新到当前进度' }}
          </button>
        </div>
      </header>

      <div v-if="statusNotice" class="status-strip" :class="{ error: statusNotice.isError }">
        <div class="status-main">
          <strong>{{ statusNotice.isError ? '生成失败' : '状态' }}</strong>
          <p>{{ statusNotice.summary }}</p>
        </div>
        <details v-if="statusNotice.detail" class="status-detail">
          <summary>查看详情</summary>
          <pre>{{ statusNotice.detail }}</pre>
        </details>
      </div>

      <nav class="tabs">
        <button v-for="tab in tabs" :key="tab.key" :class="{ active: activeTab === tab.key }" @click="activeTab = tab.key">
          {{ tab.label }}
        </button>
      </nav>

      <main class="ai-content">
        <section v-if="activeTab === 'overview'" class="overview-grid">
          <div class="overview-main">
            <h2>世界观</h2>
            <p class="summary">{{ overviewSummary }}</p>
            <div v-if="recentChanges.length" class="summary-points">
              <span>最近变化</span>
              <ul>
                <li v-for="item in recentChanges" :key="item">{{ item }}</li>
              </ul>
            </div>
            <div v-if="openQuestions.length" class="summary-points">
              <span>未解问题</span>
              <ul>
                <li v-for="item in openQuestions" :key="item">{{ item }}</li>
              </ul>
            </div>
            <div class="worldview-groups">
              <section v-for="group in worldviewGroups" :key="group.category" class="worldview-group">
                <div class="group-head">
                  <button class="group-toggle" @click="toggleWorldviewGroup(group.category)">
                    <span>{{ group.collapsed ? '+' : '-' }}</span>
                    <h3>{{ group.category }}</h3>
                  </button>
                  <span>{{ group.items.length }}</span>
                </div>
                <div v-if="!group.collapsed" class="group-items">
                  <article v-for="note in group.items" :key="`${group.category}-${note.title}`" class="note-item">
                    <div class="item-title">
                      <h4>{{ note.title }}</h4>
                      <span v-if="note.confidence">{{ note.confidence }}</span>
                    </div>
                    <p>{{ note.content }}</p>
                  </article>
                </div>
              </section>
            </div>
            <EmptyState v-if="!worldviewGroups.length" text="暂无世界观资料" />
          </div>

          <aside class="overview-side">
            <div class="metric">
              <span>角色</span>
              <strong>{{ importantCharacters.length }}</strong>
            </div>
            <div class="metric">
              <span>关系</span>
              <strong>{{ displayRelationships.length }}</strong>
            </div>
            <div class="metric">
              <span>地点</span>
              <strong>{{ displayLocations.length }}</strong>
            </div>
            <div class="metric">
              <span>最近章节</span>
              <strong>{{ memory.processedChapterIndex != null ? memory.processedChapterIndex + 1 : '-' }}</strong>
            </div>
          </aside>
        </section>

        <section v-else-if="activeTab === 'characters'" class="list-panel">
          <div class="panel-toolbar">
            <label class="search-field">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="11" cy="11" r="7" />
                <path d="m20 20-3.5-3.5" />
              </svg>
              <input v-model="characterSearch" placeholder="搜索角色、别名、势力、位置" />
            </label>
            <span class="result-count">{{ filteredCharacters.length }} / {{ importantCharacters.length }}</span>
          </div>
          <article v-for="character in filteredCharacters" :key="character.name" class="list-item">
            <div class="item-title">
              <h3>{{ character.name }}</h3>
              <span v-if="character.faction">{{ character.faction }}</span>
            </div>
            <p>{{ character.status || character.description || '暂无状态' }}</p>
            <div class="meta-line">
              <span v-if="character.location">位置：{{ character.location }}</span>
              <span v-if="character.lastSeenChapter">最近：{{ character.lastSeenChapter }}</span>
              <span v-if="character.aliases?.length">别名：{{ character.aliases.join('、') }}</span>
            </div>
          </article>
          <EmptyState v-if="!filteredCharacters.length" :text="importantCharacters.length ? '没有匹配的角色' : '暂无重要角色资料'" />
        </section>

        <section v-else-if="activeTab === 'relationships'" class="relation-grid">
          <article v-for="relationship in displayRelationships" :key="`${relationship.source}-${relationship.target}-${relationship.relation}`" class="relation-item">
            <div class="relation-head">
              <strong>{{ relationship.source }}</strong>
              <span>{{ relationship.relation }}</span>
              <strong>{{ relationship.target }}</strong>
            </div>
            <p>{{ relationship.description || relationship.status || '暂无说明' }}</p>
          </article>
          <EmptyState v-if="!displayRelationships.length" text="暂无重要人物关系" />
        </section>

        <section v-else-if="activeTab === 'map'" class="map-panel">
          <div class="map-toolbar">
            <div class="map-title">
              <h2>世界地图</h2>
              <p>{{ displayBaseMemory?.map?.updatedAt ? formatTime(displayBaseMemory.map.updatedAt) : '未生成' }}</p>
            </div>
            <button class="secondary-btn" :disabled="aiStore.isBusy" @click="redrawMap">
              {{ aiStore.phase === 'map' ? '绘制中...' : '重绘地图' }}
            </button>
          </div>

          <div class="map-frame">
            <img v-if="displayBaseMemory?.map?.imageUrl" :src="displayBaseMemory.map.imageUrl" alt="世界地图" />
            <div v-else-if="relationshipGraph.nodes.length" class="graph-fallback">
              <div class="graph-canvas">
                <div class="graph-legend">
                  <span class="legend-location">地点</span>
                  <span class="legend-character">角色</span>
                </div>
                <svg :viewBox="`0 0 ${graphLayout.width} ${graphLayout.height}`" role="img" aria-label="人物关系图">
                  <defs>
                    <marker id="graph-arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
                      <path d="M 0 0 L 10 5 L 0 10 z" />
                    </marker>
                  </defs>
                  <g class="graph-links">
                    <g
                      v-for="link in graphLayout.links"
                      :key="`${link.source}-${link.target}-${link.label}`"
                      class="graph-link"
                      :class="{ highlighted: link.highlighted, dimmed: link.dimmed, located: link.label === '位于' }"
                    >
                      <path :d="link.path" />
                      <g v-if="link.showLabel" class="graph-link-label">
                        <rect
                          :x="link.labelX - graphLabelWidth(link.label) / 2"
                          :y="link.labelY - 13"
                          :width="graphLabelWidth(link.label)"
                          height="22"
                          rx="11"
                        />
                        <text :x="link.labelX" :y="link.labelY + 3">{{ link.label }}</text>
                      </g>
                    </g>
                  </g>
                  <g
                    v-for="node in graphLayout.nodes"
                    :key="node.id"
                    class="graph-node"
                    :class="{
                      active: selectedGraphNode?.id === node.id,
                      location: node.kind === 'location',
                      dimmed: node.dimmed,
                      connected: node.connectedToSelected,
                    }"
                    @click="selectGraphNode(node.id)"
                  >
                    <foreignObject :x="node.x" :y="node.y" :width="node.width" :height="node.height">
                      <div xmlns="http://www.w3.org/1999/xhtml" class="graph-node-card">
                        <span class="node-dot"></span>
                        <strong>{{ node.label }}</strong>
                      </div>
                    </foreignObject>
                  </g>
                </svg>
              </div>
              <aside v-if="selectedGraphNode" class="graph-detail">
                <div class="graph-detail-head">
                  <span>{{ selectedGraphNode.kind === 'location' ? '地点' : '角色' }}</span>
                  <strong>{{ selectedGraphNode.label }}</strong>
                </div>
                <p>{{ selectedGraphNode.detail || '暂无说明' }}</p>
                <div v-if="selectedGraphConnections.length" class="graph-connection-list">
                  <span>直接关联</span>
                  <button
                    v-for="connection in selectedGraphConnections"
                    :key="`${connection.id}-${connection.relation}`"
                    @click="selectGraphNode(connection.id)"
                  >
                    <strong>{{ connection.label }}</strong>
                    <small>{{ connection.relation }}</small>
                  </button>
                </div>
                <small>{{ displayBaseMemory?.map?.fallbackReason || '图片地图未生成，显示关系图' }}</small>
              </aside>
            </div>
            <div v-else class="map-empty">暂无地图</div>
          </div>

          <div class="location-tree">
            <article
              v-for="row in visibleLocationRows"
              :key="row.location.name"
              class="location-item tree-location"
              :style="{ '--depth-offset': `${row.depth * 22}px` }"
            >
              <div class="item-title">
                <div class="location-title-wrap">
                  <button
                    v-if="row.hasChildren"
                    class="location-toggle"
                    :aria-label="isLocationCollapsed(row.location.name) ? '展开地点' : '收起地点'"
                    @click="toggleLocation(row.location.name)"
                  >
                    {{ isLocationCollapsed(row.location.name) ? '+' : '-' }}
                  </button>
                  <span v-else class="location-toggle ghost"></span>
                  <h3>{{ row.location.name }}</h3>
                </div>
                <span v-if="row.location.kind">{{ row.location.kind }}</span>
              </div>
              <p>{{ row.location.description }}</p>
              <div class="meta-line">
                <span v-if="row.location.status">状态：{{ row.location.status }}</span>
                <span v-if="row.location.parentName">上级：{{ row.location.parentName }}</span>
                <span v-if="row.location.relatedCharacters?.length">相关：{{ row.location.relatedCharacters.join('、') }}</span>
              </div>
            </article>
            <EmptyState v-if="!visibleLocationRows.length" text="暂无地点资料" />
          </div>
        </section>

        <section v-else class="settings-panel">
          <article class="settings-card source-card">
            <div class="settings-card-head">
              <h2>模型来源</h2>
              <span class="server-status" :class="{ active: canUseServerModel }">
                {{ canUseServerModel ? '可用后端配置' : '未授权后端配置' }}
              </span>
            </div>
            <div class="source-options">
              <button class="source-option" :class="{ active: configDraft.modelSource === 'browser' }" @click="configDraft.modelSource = 'browser'">
                自己配置模型
              </button>
              <button
                class="source-option"
                :class="{ active: configDraft.modelSource === 'server' }"
                :disabled="!canUseServerModel"
                @click="configDraft.modelSource = 'server'"
              >
                使用后端配置
              </button>
            </div>
            <p class="settings-hint">
              后端配置由管理员保存到服务器；只有开启 AI 模型权限的账号才能使用。自己配置仍只保存在当前浏览器。
            </p>
          </article>

          <div v-if="configDraft.modelSource === 'server'" class="settings-cards">
            <article class="settings-card">
              <div class="settings-card-head">
                <h2>后端文本模型</h2>
                <span class="server-status" :class="{ active: serverTextReady }">{{ serverTextReady ? '已启用' : '未配置' }}</span>
              </div>
              <p class="settings-hint">{{ serverConfig?.text.model || '管理员尚未配置文本模型' }}</p>
            </article>
            <article class="settings-card">
              <div class="settings-card-head">
                <h2>后端图片模型</h2>
                <span class="server-status" :class="{ active: serverImageReady }">{{ serverImageReady ? '已启用' : '未配置' }}</span>
              </div>
              <p class="settings-hint">{{ serverConfig?.image.model || '管理员尚未配置图片模型' }} · {{ serverConfig?.image.imageSize || '1024x1024' }}</p>
            </article>
            <article class="settings-card">
              <div class="settings-card-head">
                <h2>后端语音模型</h2>
                <span class="server-status" :class="{ active: serverSpeechReady }">{{ serverSpeechReady ? '已启用' : '未配置' }}</span>
              </div>
              <p class="settings-hint">{{ serverConfig?.speech.model || '管理员尚未配置 OpenAI Speech' }} · {{ serverConfig?.speech.voice || 'alloy' }}</p>
            </article>
          </div>

          <div v-else class="settings-cards">
            <article class="settings-card">
              <div class="settings-card-head">
                <h2>文本模型</h2>
                <label class="switch-line compact">
                  <input v-model="configDraft.textUseFullUrl" type="checkbox" />
                  <span class="switch-ui"></span>
                  <span>完整链接</span>
                </label>
              </div>
              <div class="settings-grid">
                <label class="field span-2">
                  <span>Base URL</span>
                  <input v-model="configDraft.textBaseUrl" placeholder="http://localhost:8825" />
                </label>
                <label class="field">
                  <span>模型</span>
                  <input v-model="configDraft.textModel" />
                </label>
                <label class="field">
                  <span>API Key</span>
                  <input v-model="configDraft.textApiKey" type="password" autocomplete="off" />
                </label>
              </div>
            </article>

            <article class="settings-card">
              <div class="settings-card-head">
                <h2>图片模型</h2>
                <label class="switch-line compact">
                  <input v-model="configDraft.imageUseFullUrl" type="checkbox" />
                  <span class="switch-ui"></span>
                  <span>完整链接</span>
                </label>
              </div>
              <div class="settings-grid">
                <label class="field span-2">
                  <span>Base URL</span>
                  <input v-model="configDraft.imageBaseUrl" placeholder="http://localhost:8826" />
                </label>
                <label class="field">
                  <span>模型</span>
                  <input v-model="configDraft.imageModel" />
                </label>
                <label class="field">
                  <span>尺寸</span>
                  <select v-model="configDraft.imageSize">
                    <option value="1024x1024">1024x1024</option>
                    <option value="1792x1024">1792x1024</option>
                    <option value="1024x1792">1024x1792</option>
                  </select>
                </label>
                <label class="field span-2">
                  <span>API Key</span>
                  <input v-model="configDraft.imageApiKey" type="password" autocomplete="off" />
                </label>
              </div>
            </article>
          </div>

          <div v-if="configDraft.modelSource === 'browser'" class="settings-footer">
            <label class="switch-line proxy-option">
              <input v-model="configDraft.useBackendProxy" type="checkbox" />
              <span class="switch-ui"></span>
              <span>使用后端代理调用模型</span>
            </label>
          </div>
          <div class="settings-actions">
            <button class="primary-btn" @click="saveConfig">保存配置</button>
            <button class="danger-btn" @click="resetMemory">重置 AI资料</button>
          </div>

          <section v-if="isServerModelAdmin" class="admin-model-panel">
            <div class="admin-model-head">
              <h2>后端模型配置</h2>
              <button class="primary-btn" @click="saveServerConfig">保存后端配置</button>
            </div>
            <div class="settings-cards">
              <article class="settings-card">
                <div class="settings-card-head">
                  <h2>文本模型</h2>
                  <label class="switch-line compact">
                    <input v-model="serverConfigDraft.text.enabled" type="checkbox" />
                    <span class="switch-ui"></span>
                    <span>启用</span>
                  </label>
                </div>
                <div class="settings-grid">
                  <label class="field span-2">
                    <span>Base URL</span>
                    <input v-model="serverConfigDraft.text.baseUrl" placeholder="https://api.openai.com" />
                  </label>
                  <label class="field">
                    <span>模型</span>
                    <input v-model="serverConfigDraft.text.model" placeholder="gpt-4o-mini" />
                  </label>
                  <label class="field">
                    <span>API Key</span>
                    <input v-model="serverConfigDraft.text.apiKey" type="password" autocomplete="off" />
                  </label>
                  <label class="switch-line compact">
                    <input v-model="serverConfigDraft.text.useFullUrl" type="checkbox" />
                    <span class="switch-ui"></span>
                    <span>完整链接</span>
                  </label>
                </div>
              </article>

              <article class="settings-card">
                <div class="settings-card-head">
                  <h2>图片模型</h2>
                  <label class="switch-line compact">
                    <input v-model="serverConfigDraft.image.enabled" type="checkbox" />
                    <span class="switch-ui"></span>
                    <span>启用</span>
                  </label>
                </div>
                <div class="settings-grid">
                  <label class="field span-2">
                    <span>Base URL</span>
                    <input v-model="serverConfigDraft.image.baseUrl" placeholder="https://api.openai.com" />
                  </label>
                  <label class="field">
                    <span>模型</span>
                    <input v-model="serverConfigDraft.image.model" placeholder="gpt-image-1" />
                  </label>
                  <label class="field">
                    <span>尺寸</span>
                    <select v-model="serverConfigDraft.image.imageSize">
                      <option value="1024x1024">1024x1024</option>
                      <option value="1792x1024">1792x1024</option>
                      <option value="1024x1792">1024x1792</option>
                    </select>
                  </label>
                  <label class="field span-2">
                    <span>API Key</span>
                    <input v-model="serverConfigDraft.image.apiKey" type="password" autocomplete="off" />
                  </label>
                  <label class="switch-line compact">
                    <input v-model="serverConfigDraft.image.useFullUrl" type="checkbox" />
                    <span class="switch-ui"></span>
                    <span>完整链接</span>
                  </label>
                </div>
              </article>

              <article class="settings-card">
                <div class="settings-card-head">
                  <h2>OpenAI Speech</h2>
                  <label class="switch-line compact">
                    <input v-model="serverConfigDraft.speech.enabled" type="checkbox" />
                    <span class="switch-ui"></span>
                    <span>启用</span>
                  </label>
                </div>
                <div class="settings-grid">
                  <label class="field span-2">
                    <span>Base URL</span>
                    <input v-model="serverConfigDraft.speech.baseUrl" placeholder="https://api.openai.com" />
                  </label>
                  <label class="field">
                    <span>模型</span>
                    <input v-model="serverConfigDraft.speech.model" placeholder="gpt-4o-mini-tts" />
                  </label>
                  <label class="field">
                    <span>音色</span>
                    <input v-model="serverConfigDraft.speech.voice" placeholder="alloy" />
                  </label>
                  <label class="field">
                    <span>格式</span>
                    <select v-model="serverConfigDraft.speech.responseFormat">
                      <option value="mp3">mp3</option>
                      <option value="wav">wav</option>
                      <option value="opus">opus</option>
                      <option value="flac">flac</option>
                      <option value="pcm">pcm</option>
                    </select>
                  </label>
                  <label class="field">
                    <span>API Key</span>
                    <input v-model="serverConfigDraft.speech.apiKey" type="password" autocomplete="off" />
                  </label>
                  <label class="switch-line compact">
                    <input v-model="serverConfigDraft.speech.useFullUrl" type="checkbox" />
                    <span class="switch-ui"></span>
                    <span>完整链接</span>
                  </label>
                </div>
              </article>
            </div>
          </section>
        </section>
      </main>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, defineComponent, h, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { saveAiModelConfig } from '../api/aiModel'
import { getBookContent, getChapterList, getShelfBook } from '../api/bookshelf'
import { useAiBookStore } from '../stores/aiBook'
import { useAppStore } from '../stores/app'
import { useReaderStore } from '../stores/reader'
import type {
  AiBookCharacter,
  AiBookConfig,
  AiBookLocation,
  AiBookMemory,
  AiBookRelationship,
  AiServerModelConfig,
  Book,
  BookChapter,
} from '../types'
import { buildAiBookRelationshipGraph, layoutAiBookRelationshipGraph } from '../utils/aiBookGraph'
import { buildAiBookLocationRows, groupAiBookWorldview } from '../utils/aiBookPresentation'
import { isAiBookMemoryV2, toAiBookDisplayMemory } from '../utils/aiBookV2'
import { shouldAutoUseServerAiBookConfig } from '../utils/aiBookConfig'
import { collapseWhitespace, summarizeDisplayError } from '../utils/httpError'

type AiTab = 'overview' | 'characters' | 'relationships' | 'map' | 'settings'

const EmptyState = defineComponent({
  props: { text: { type: String, required: true } },
  setup(props) {
    return () => h('div', { class: 'empty-state' }, props.text)
  },
})

const route = useRoute()
const router = useRouter()
const aiStore = useAiBookStore()
const appStore = useAppStore()
const readerStore = useReaderStore()

const loading = ref(true)
const activeTab = ref<AiTab>('overview')
const book = ref<Book | null>(null)
const chapters = ref<BookChapter[]>([])
const configDraft = reactive<AiBookConfig>({ ...aiStore.config })
const serverConfigDraft = reactive<AiServerModelConfig>(createEmptyServerModelConfig())
const selectedGraphNodeId = ref('')
const characterSearch = ref('')
const collapsedLocationIds = ref(new Set<string>())
const collapsedWorldviewCategories = ref(new Set<string>())

const tabs: Array<{ key: AiTab; label: string }> = [
  { key: 'overview', label: '总览' },
  { key: 'characters', label: '角色' },
  { key: 'relationships', label: '关系' },
  { key: 'map', label: '地图' },
  { key: 'settings', label: '设置' },
]

const memory = computed(() => aiStore.memory)
const displayBaseMemory = computed(() => memory.value ? toAiBookDisplayMemory(memory.value) : null)
const v2Summary = computed(() => isAiBookMemoryV2(memory.value) ? memory.value.summary : null)
const overviewSummary = computed(() => v2Summary.value?.current || displayBaseMemory.value?.summary || '暂无资料')
const recentChanges = computed(() => v2Summary.value?.recentChanges || [])
const openQuestions = computed(() => v2Summary.value?.openQuestions || [])
const canUseServerModel = computed(() => aiStore.canUseServerModel || aiStore.isServerModelAdmin)
const isServerModelAdmin = computed(() => aiStore.isServerModelAdmin)
const serverConfig = computed(() => aiStore.serverModelConfig?.config || null)
const serverTextReady = computed(() => Boolean(serverConfig.value?.text.enabled && serverConfig.value.text.baseUrl && serverConfig.value.text.model))
const serverImageReady = computed(() => Boolean(serverConfig.value?.image.enabled && serverConfig.value.image.baseUrl && serverConfig.value.image.model))
const serverSpeechReady = computed(() => Boolean(serverConfig.value?.speech.enabled && serverConfig.value.speech.baseUrl && serverConfig.value.speech.model))
const worldviewGroups = computed(() => groupAiBookWorldview(displayBaseMemory.value?.worldview || [], collapsedWorldviewCategories.value))
const importantCharacters = computed(() => normalizeDisplayCharacters(displayBaseMemory.value?.characters || []))
const filteredCharacters = computed(() => filterCharacters(importantCharacters.value, characterSearch.value))
const displayRelationships = computed(() => normalizeDisplayRelationships(displayBaseMemory.value?.relationships || []))
const displayLocations = computed(() => normalizeDisplayLocations(displayBaseMemory.value?.locations || []))
const visibleLocationRows = computed(() => buildAiBookLocationRows(displayLocations.value, collapsedLocationIds.value))
const displayMemory = computed<AiBookMemory | null>(() => displayBaseMemory.value
  ? {
      ...displayBaseMemory.value,
      characters: importantCharacters.value,
      relationships: displayRelationships.value,
      locations: displayLocations.value,
    }
  : null)
const relationshipGraph = computed(() => displayMemory.value
  ? buildAiBookRelationshipGraph(displayMemory.value)
  : { nodes: [], links: [] })
const activeGraphNodeId = computed(() => {
  const selected = selectedGraphNodeId.value
  if (selected && relationshipGraph.value.nodes.some((node) => node.id === selected)) {
    return selected
  }
  return relationshipGraph.value.nodes[0]?.id || ''
})
const graphLayout = computed(() => layoutAiBookRelationshipGraph(relationshipGraph.value, activeGraphNodeId.value))
const selectedGraphNode = computed(() => {
  return graphLayout.value.nodes.find((node) => node.id === activeGraphNodeId.value)
    || graphLayout.value.nodes[0]
    || null
})
const selectedGraphConnections = computed(() => {
  const current = selectedGraphNode.value
  if (!current) return []
  return graphLayout.value.links
    .filter((link) => link.source === current.id || link.target === current.id)
    .map((link) => {
      const otherId = link.source === current.id ? link.target : link.source
      const other = graphLayout.value.nodes.find((node) => node.id === otherId)
      return other ? { id: other.id, label: other.label, relation: link.label } : null
    })
    .filter((item): item is { id: string; label: string; relation: string } => Boolean(item))
})
const progressText = computed(() => {
  const index = memory.value?.processedChapterIndex
  if (index == null) return '尚未生成'
  return `已更新至第 ${index + 1} 章`
})
const statusNotice = computed(() => {
  const source = aiStore.statusText || memory.value?.lastError || ''
  if (!source.trim()) return null
  const isLastError = !aiStore.statusText && Boolean(memory.value?.lastError)
  const summary = summarizeDisplayError(source)
  const normalizedSource = collapseWhitespace(source)
  const hasDetail = normalizedSource !== summary && source.trim().length > summary.length + 20
  return {
    summary,
    detail: hasDetail ? source.trim() : '',
    isError: aiStore.phase === 'error' || isLastError,
  }
})

watch(
  () => aiStore.config,
  (next) => Object.assign(configDraft, next),
  { deep: true },
)

watch(
  () => aiStore.serverModelConfig?.config,
  (next) => {
    if (next) Object.assign(serverConfigDraft, cloneServerModelConfig(next))
  },
  { deep: true },
)

onMounted(async () => {
  await appStore.fetchUserInfo()
  aiStore.refreshConfig()
  await aiStore.loadServerModelConfig({ force: true })
  if (shouldAutoUseServerAiBookConfig(aiStore.config, canUseServerModel.value)) {
    aiStore.persistConfig({ ...aiStore.config, modelSource: 'server' })
  }
  Object.assign(configDraft, aiStore.config)
  if (aiStore.serverModelConfig?.config) {
    Object.assign(serverConfigDraft, cloneServerModelConfig(aiStore.serverModelConfig.config))
  }
  const bookUrl = String(route.query.bookUrl || '')
  if (!bookUrl) {
    router.replace('/')
    return
  }
  try {
    book.value = await getShelfBook(bookUrl)
    await aiStore.load(book.value)
    chapters.value = await getChapterList({
      bookUrl: book.value.bookUrl,
      bookSourceUrl: book.value.origin,
    }).catch(() => [])
  } catch (error) {
    appStore.showToast((error as Error).message || 'AI资料加载失败', 'error')
    router.replace('/')
  } finally {
    loading.value = false
  }
})

function goBack() {
  router.back()
}

async function toggleEnabled(event: Event) {
  if (!book.value) return
  const enabled = (event.target as HTMLInputElement).checked
  try {
    await aiStore.setEnabled(book.value, enabled)
    appStore.showToast(enabled ? '已开启自动更新' : '已关闭自动更新', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '设置失败', 'error')
  }
}

async function updateToCurrent() {
  if (!book.value || !memory.value) return
  const targetIndex = resolveCurrentIndex()
  if (!chapters.value.length) {
    appStore.showToast('目录未加载，无法更新', 'warning')
    return
  }
  const startIndex = Math.max(0, (memory.value.processedChapterIndex ?? -1) + 1)
  if (startIndex > targetIndex) {
    appStore.showToast('当前进度已更新', 'success')
    return
  }

  try {
    let currentMemory = memory.value
    for (let index = startIndex; index <= targetIndex; index += 1) {
      const chapter = chapters.value[index]
      if (!chapter) continue
      const chapterContent = await resolveChapterContent(index, chapter)
      currentMemory = await aiStore.runChapterUpdate({
        book: book.value,
        chapter,
        chapterContent,
        current: currentMemory,
        chapters: chapters.value,
      })
    }
    appStore.showToast('AI资料已更新', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || 'AI资料更新失败', 'error')
  }
}

async function redrawMap() {
  if (!book.value) return
  const next = await aiStore.redrawMap(book.value)
  const displayNext = next ? toAiBookDisplayMemory(next) : null
  if (displayNext?.map?.imageUrl) {
    appStore.showToast('地图已更新', 'success')
  } else {
    appStore.showToast('图片地图不可用，已显示关系图', 'warning')
  }
}

function selectGraphNode(id: string) {
  selectedGraphNodeId.value = id
}

function toggleLocation(name: string) {
  const key = normalizeKey(name)
  const next = new Set(collapsedLocationIds.value)
  if (next.has(key)) {
    next.delete(key)
  } else {
    next.add(key)
  }
  collapsedLocationIds.value = next
}

function isLocationCollapsed(name: string) {
  return collapsedLocationIds.value.has(normalizeKey(name))
}

function toggleWorldviewGroup(category: string) {
  const key = normalizeKey(category)
  const next = new Set(collapsedWorldviewCategories.value)
  if (next.has(key)) {
    next.delete(key)
  } else {
    next.add(key)
  }
  collapsedWorldviewCategories.value = next
}

function graphLabelWidth(label: string) {
  return Math.max(44, Math.min(108, label.length * 14 + 22))
}

function saveConfig() {
  if (configDraft.modelSource === 'server' && !canUseServerModel.value) {
    appStore.showToast('当前账号没有使用后端模型配置的权限', 'warning')
    configDraft.modelSource = 'browser'
  }
  aiStore.persistConfig({ ...configDraft })
  appStore.showToast('AI配置已保存', 'success')
}

async function saveServerConfig() {
  if (!isServerModelAdmin.value) return
  try {
    const saved = await saveAiModelConfig(cloneServerModelConfig(serverConfigDraft))
    aiStore.serverModelConfig = saved
    Object.assign(serverConfigDraft, cloneServerModelConfig(saved.config))
    appStore.showToast('后端模型配置已保存', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '后端模型配置保存失败', 'error')
  }
}

async function resetMemory() {
  if (!book.value) return
  if (!confirm('确定重置当前书的 AI资料？')) return
  await aiStore.reset(book.value)
  appStore.showToast('AI资料已重置', 'success')
}

function resolveCurrentIndex() {
  if (readerStore.book?.bookUrl === book.value?.bookUrl) {
    return Math.max(0, readerStore.currentIndex)
  }
  return Math.max(0, Math.min(chapters.value.length - 1, book.value?.durChapterIndex || 0))
}

async function resolveChapterContent(index: number, chapter: BookChapter) {
  if (readerStore.book?.bookUrl === book.value?.bookUrl) {
    const content = await readerStore.fetchChapterContent(index)
    if (content) return content
  }
  return getBookContent({
    chapterUrl: chapter.url,
    bookSourceUrl: book.value?.origin,
  })
}

function formatTime(value: number) {
  return new Date(value).toLocaleString()
}

function normalizeDisplayCharacters(characters: AiBookCharacter[]) {
  const byName = new Map<string, AiBookCharacter>()
  for (const character of characters) {
    if (!character.name || isLowImportance(character.importance)) continue
    const key = normalizeKey(character.name)
    const existing = byName.get(key)
    byName.set(key, existing ? mergeDisplayCharacter(existing, character) : character)
  }
  return [...byName.values()]
}

function filterCharacters(characters: AiBookCharacter[], query: string) {
  const normalizedQuery = normalizeSearch(query)
  if (!normalizedQuery) return characters
  return characters.filter((character) => normalizeSearch([
    character.name,
    character.aliases?.join(' '),
    character.status,
    character.faction,
    character.location,
    character.description,
  ].filter(Boolean).join(' ')).includes(normalizedQuery))
}

function normalizeDisplayRelationships(relationships: AiBookRelationship[]) {
  const byPair = new Map<string, AiBookRelationship>()
  for (const relationship of relationships) {
    if (
      !relationship.source
      || !relationship.target
      || !relationship.relation
      || normalizeKey(relationship.source) === normalizeKey(relationship.target)
      || isLowImportance(relationship.importance)
      || isLowValueRelationship(relationship)
    ) {
      continue
    }
    const key = relationshipKey(relationship.source, relationship.target, relationship.relation)
    const existing = byPair.get(key)
    byPair.set(key, existing ? mergeDisplayRelationship(existing, relationship) : relationship)
  }
  return [...byPair.values()]
}

function normalizeDisplayLocations(locations: AiBookLocation[]) {
  const byName = new Map<string, AiBookLocation>()
  for (const location of locations) {
    if (!location.name || isLowImportance(location.importance)) continue
    const parentName = location.parentName && normalizeKey(location.parentName) !== normalizeKey(location.name)
      ? location.parentName
      : undefined
    const normalized = { ...location, parentName }
    const key = normalizeKey(location.name)
    const existing = byName.get(key)
    byName.set(key, existing ? mergeDisplayLocation(existing, normalized) : normalized)
  }
  return [...byName.values()]
}

function mergeDisplayCharacter(current: AiBookCharacter, next: AiBookCharacter): AiBookCharacter {
  return {
    ...current,
    aliases: uniqueStrings([...(current.aliases || []), ...(next.aliases || [])]),
    status: richerString(current.status, next.status),
    faction: current.faction || next.faction,
    location: current.location || next.location,
    description: richerString(current.description, next.description),
    lastSeenChapter: current.lastSeenChapter || next.lastSeenChapter,
    importance: preferImportance(current.importance, next.importance),
  }
}

function mergeDisplayRelationship(current: AiBookRelationship, next: AiBookRelationship): AiBookRelationship {
  return {
    ...current,
    status: richerString(current.status, next.status),
    description: richerString(current.description, next.description),
    importance: preferImportance(current.importance, next.importance),
  }
}

function mergeDisplayLocation(current: AiBookLocation, next: AiBookLocation): AiBookLocation {
  return {
    ...current,
    kind: current.kind || next.kind,
    parentName: current.parentName || next.parentName,
    description: richerString(current.description, next.description),
    status: richerString(current.status, next.status),
    relatedCharacters: uniqueStrings([...(current.relatedCharacters || []), ...(next.relatedCharacters || [])]),
    firstSeenChapter: current.firstSeenChapter || next.firstSeenChapter,
    importance: preferImportance(current.importance, next.importance),
  }
}

function relationshipKey(source: string, target: string, relation: string) {
  return `${[normalizeKey(source), normalizeKey(target)].sort().join('::')}::${normalizeKey(relation)}`
}

function isLowValueRelationship(relationship: AiBookRelationship) {
  if (importanceRank(relationship.importance) >= 2) return false
  const relation = normalizeKey(relationship.relation)
  if (!['认识', '见过', '路过', '同村', '同校', '位于', '相关'].includes(relation)) return false
  return normalizeKey(relationship.description || relationship.status || '').length < 18
}

function normalizeSearch(value: string) {
  return value.trim().toLowerCase().replace(/\s+/g, '')
}

function normalizeKey(value: string | undefined) {
  return (value || '')
    .trim()
    .toLowerCase()
    .replace(/[·•・]/g, '.')
    .replace(/\s+/g, '')
}

function isLowImportance(value: string | undefined) {
  const key = normalizeKey(value)
  if (!key) return false
  return ['low', '低', '低重要性', '不重要', '路人', '背景', 'minor', 'background', 'oneoff', '一次性']
    .some((term) => key.includes(term))
}

function importanceRank(value: string | undefined) {
  const key = normalizeKey(value)
  if (key.includes('high') || key.includes('高')) return 3
  if (key.includes('medium') || key.includes('中')) return 2
  if (isLowImportance(value)) return 1
  return 0
}

function richerString(current: string | undefined, next: string | undefined) {
  if (!current) return next || ''
  if (!next) return current
  return next.length > current.length ? next : current
}

function preferImportance(current: string | undefined, next: string | undefined) {
  return importanceRank(next) > importanceRank(current) ? next : current || next
}

function uniqueStrings(values: string[]) {
  const seen = new Set<string>()
  const result: string[] = []
  for (const value of values) {
    const key = normalizeKey(value)
    if (!key || seen.has(key)) continue
    seen.add(key)
    result.push(value)
  }
  return result
}

function createEmptyServerModelConfig(): AiServerModelConfig {
  return {
    text: {
      enabled: false,
      baseUrl: '',
      apiKey: '',
      model: 'gpt-4o-mini',
      useFullUrl: false,
    },
    image: {
      enabled: false,
      baseUrl: '',
      apiKey: '',
      model: 'gpt-image-1',
      useFullUrl: false,
      imageSize: '1024x1024',
    },
    speech: {
      enabled: false,
      baseUrl: '',
      apiKey: '',
      model: 'gpt-4o-mini-tts',
      useFullUrl: false,
      voice: 'alloy',
      responseFormat: 'mp3',
    },
  }
}

function cloneServerModelConfig(config: AiServerModelConfig): AiServerModelConfig {
  return JSON.parse(JSON.stringify(config)) as AiServerModelConfig
}
</script>

<style scoped>
.ai-book-view {
  height: 100%;
  overflow: hidden;
  background: var(--color-bg);
  color: var(--color-text);
}

.ai-loading {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--color-text-secondary);
}

.ai-shell {
  height: 100%;
  display: flex;
  flex-direction: column;
  max-width: 1240px;
  margin: 0 auto;
  padding: 16px 28px 22px;
  box-sizing: border-box;
  min-height: 0;
}

.ai-header {
  display: flex;
  justify-content: space-between;
  gap: 18px;
  align-items: center;
  border-bottom: 1px solid var(--color-border-light);
  padding-bottom: 12px;
}

.title-stack {
  min-width: 0;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.ai-header h1 {
  margin: 0;
  min-width: 0;
  font-size: 24px;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-header p,
.map-toolbar p {
  margin: 4px 0 0;
  color: var(--color-text-tertiary);
  font-size: 13px;
}

.back-btn,
.secondary-btn,
.primary-btn,
.danger-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 34px;
  padding: 0 13px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
  background: var(--color-bg-elevated);
  font-weight: 600;
  cursor: pointer;
}

.back-btn svg {
  width: 16px;
  height: 16px;
}

.back-btn {
  flex: 0 0 auto;
  background: transparent;
}

.primary-btn {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

.danger-btn {
  color: var(--color-danger, #d14b4b);
}

.primary-btn:disabled,
.secondary-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.header-actions,
.settings-actions,
.map-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}

.enable-switch {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--color-text-secondary);
  cursor: pointer;
}

.enable-switch input {
  display: none;
}

.enable-switch span {
  width: 36px;
  height: 20px;
  border-radius: 999px;
  background: var(--color-border);
  position: relative;
  transition: background var(--duration-fast);
}

.enable-switch span::after {
  content: "";
  position: absolute;
  width: 16px;
  height: 16px;
  left: 2px;
  top: 2px;
  border-radius: 50%;
  background: #fff;
  transition: transform var(--duration-fast);
}

.enable-switch input:checked + span {
  background: var(--color-primary);
}

.enable-switch input:checked + span::after {
  transform: translateX(16px);
}

.status-strip {
  margin-top: 14px;
  padding: 10px 12px;
  border-radius: 8px;
  background: rgba(201, 127, 58, 0.12);
  color: var(--color-text-secondary);
  font-size: 13px;
  flex: 0 0 auto;
  max-height: 240px;
  overflow: hidden;
}

.status-strip.error {
  background: rgba(209, 75, 75, 0.12);
}

.status-main {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  min-width: 0;
}

.status-main strong {
  flex: 0 0 auto;
  color: var(--color-text);
  font-weight: 700;
}

.status-main p {
  min-width: 0;
  margin: 0;
  line-height: 1.55;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.status-detail {
  margin-top: 8px;
}

.status-detail summary {
  width: fit-content;
  cursor: pointer;
  color: var(--color-primary);
  font-weight: 700;
}

.status-detail pre {
  max-height: 150px;
  margin: 8px 0 0;
  padding: 10px;
  overflow: auto;
  border-radius: 6px;
  border: 1px solid rgba(209, 75, 75, 0.18);
  background: rgba(255, 255, 255, 0.46);
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
}

.tabs {
  display: flex;
  gap: 4px;
  margin-top: 10px;
  border-bottom: 1px solid var(--color-border-light);
}

.tabs button {
  padding: 10px 15px;
  color: var(--color-text-tertiary);
  font-weight: 600;
  border-bottom: 2px solid transparent;
}

.tabs button.active {
  color: var(--color-primary);
  border-color: var(--color-primary);
}

.ai-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 14px 0 28px;
  scrollbar-width: none;
}

.overview-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 260px;
  gap: 24px;
}

.overview-main h2 {
  margin: 0 0 14px;
  font-size: 18px;
}

.summary {
  margin: 0 0 18px;
  line-height: 1.8;
  color: var(--color-text-secondary);
}

.summary-points {
  display: grid;
  gap: 8px;
  margin: 0 0 16px;
}

.summary-points span {
  font-size: 12px;
  font-weight: 700;
  color: var(--color-text-muted);
}

.summary-points ul {
  display: grid;
  gap: 6px;
  margin: 0;
  padding-left: 18px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.worldview-groups,
.worldview-group,
.list-panel,
.relation-grid,
.location-tree {
  display: grid;
  gap: 12px;
}

.worldview-group {
  gap: 10px;
}

.group-items {
  display: grid;
  gap: 10px;
}

.group-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 2px;
}

.group-toggle {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text);
  font-weight: 700;
  text-align: left;
}

.group-toggle span {
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1;
}

.group-head h3 {
  margin: 0;
  font-size: 14px;
}

.group-head span,
.result-count {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 600;
}

.note-item,
.list-item,
.relation-item,
.location-item,
.metric {
  border: 1px solid var(--color-border-light);
  border-radius: 8px;
  padding: 14px;
  background: var(--color-bg-elevated);
}

.item-title,
.relation-head {
  display: flex;
  align-items: center;
  gap: 10px;
  justify-content: space-between;
}

.item-title h3 {
  margin: 0;
  font-size: 15px;
}

.item-title h4 {
  margin: 0;
  font-size: 14px;
}

.item-title span,
.relation-head span {
  font-size: 12px;
  color: var(--color-primary);
}

.note-item p,
.list-item p,
.relation-item p,
.location-item p {
  margin: 8px 0 0;
  line-height: 1.7;
  color: var(--color-text-secondary);
}

.overview-side {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  align-content: start;
  gap: 12px;
}

.metric {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.metric span,
.meta-line {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.metric strong {
  font-size: 28px;
}

.meta-line {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 10px;
}

.panel-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  position: sticky;
  top: -14px;
  z-index: 2;
  padding: 2px 0 10px;
  background: var(--color-bg);
}

.search-field {
  min-width: 260px;
  max-width: 420px;
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 38px;
  padding: 0 12px;
  border: 1px solid var(--color-border-light);
  border-radius: 8px;
  background: var(--color-bg-elevated);
  color: var(--color-text-tertiary);
}

.search-field svg {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
}

.search-field input {
  min-width: 0;
  flex: 1;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--color-text);
  font-size: 14px;
}

.relation-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.map-panel {
  display: grid;
  gap: 12px;
}

.map-toolbar {
  justify-content: space-between;
  min-height: 42px;
  padding: 0 2px;
}

.map-title {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}

.map-title h2 {
  margin: 0;
  font-size: 18px;
  white-space: nowrap;
}

.map-title p {
  margin: 0;
  white-space: nowrap;
}

.map-frame {
  min-height: min(58vh, 640px);
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--color-border-light);
  background: #1f2522;
  display: flex;
  align-items: center;
  justify-content: center;
}

.map-frame img {
  width: 100%;
  height: 100%;
  max-height: 620px;
  object-fit: contain;
  display: block;
}

.location-tree {
  gap: 10px;
}

.tree-location {
  margin-left: var(--depth-offset);
  position: relative;
}

.tree-location::before {
  content: "";
  position: absolute;
  left: -12px;
  top: -10px;
  bottom: -10px;
  width: 1px;
  background: var(--color-border-light);
  opacity: 0.55;
}

.tree-location[style*="--depth-offset: 0px"]::before {
  display: none;
}

.location-title-wrap {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.location-toggle {
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg);
  color: var(--color-text-secondary);
  font-weight: 700;
  line-height: 1;
}

.location-toggle.ghost {
  border-color: transparent;
  background: transparent;
}

.graph-fallback {
  width: 100%;
  min-height: 500px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 280px;
  background: var(--color-bg-elevated);
}

.graph-canvas {
  min-height: 500px;
  display: flex;
  align-items: stretch;
  position: relative;
  background:
    linear-gradient(rgba(70, 134, 121, 0.045) 1px, transparent 1px),
    linear-gradient(90deg, rgba(70, 134, 121, 0.045) 1px, transparent 1px),
    radial-gradient(circle at 50% 50%, rgba(212, 129, 42, 0.07), transparent 38%),
    var(--color-bg-elevated);
  background-size: 28px 28px, 28px 28px, 100% 100%, auto;
}

.graph-legend {
  position: absolute;
  top: 14px;
  left: 18px;
  z-index: 1;
  display: inline-flex;
  gap: 8px;
  padding: 6px;
  border: 1px solid var(--color-border-light);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.76);
  backdrop-filter: blur(10px);
}

.graph-legend span {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 24px;
  padding: 0 10px;
  border-radius: 999px;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 700;
}

.graph-legend span::before {
  content: "";
  width: 8px;
  height: 8px;
  border-radius: 999px;
}

.legend-location::before {
  background: #468679;
}

.legend-character::before {
  background: var(--color-primary);
}

.graph-canvas svg {
  width: 100%;
  height: auto;
  min-height: 500px;
}

.graph-link path {
  fill: none;
  stroke: rgba(52, 61, 56, 0.18);
  stroke-width: 2;
  marker-end: url(#graph-arrow);
  transition: opacity var(--duration-fast), stroke var(--duration-fast), stroke-width var(--duration-fast);
}

.graph-link.located path {
  stroke-dasharray: 5 8;
}

.graph-link.highlighted path {
  stroke: rgba(212, 129, 42, 0.72);
  stroke-width: 3;
}

.graph-link.dimmed {
  opacity: 0.18;
}

.graph-link marker path,
marker#graph-arrow path {
  fill: rgba(52, 61, 56, 0.35);
}

.graph-link text {
  fill: var(--color-text-secondary);
  font-size: 11px;
  font-weight: 700;
  text-anchor: middle;
}

.graph-link-label rect {
  fill: rgba(255, 255, 255, 0.88);
  stroke: var(--color-border-light);
}

.graph-node {
  cursor: pointer;
  transition: opacity var(--duration-fast);
}

.graph-node.dimmed {
  opacity: 0.28;
}

.graph-node-card {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 13px;
  border: 2px solid rgba(212, 129, 42, 0.62);
  border-radius: 999px;
  background: rgba(255, 247, 238, 0.94);
  color: var(--color-text);
  box-shadow: 0 10px 24px rgba(140, 120, 90, 0.1);
  overflow: hidden;
  transition:
    border-color var(--duration-fast),
    background var(--duration-fast),
    color var(--duration-fast),
    box-shadow var(--duration-fast),
    transform var(--duration-fast);
}

.graph-node-card strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  line-height: 1.2;
}

.node-dot {
  flex: 0 0 auto;
  width: 9px;
  height: 9px;
  border-radius: 999px;
  background: var(--color-primary);
}

.graph-node.location .graph-node-card {
  border-color: rgba(70, 134, 121, 0.72);
  background: rgba(235, 247, 244, 0.94);
}

.graph-node.location .node-dot {
  background: #468679;
}

.graph-node.connected .graph-node-card {
  box-shadow:
    0 10px 24px rgba(140, 120, 90, 0.12),
    0 0 0 4px rgba(212, 129, 42, 0.08);
}

.graph-node.active .graph-node-card {
  transform: translateY(-1px);
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: #fff;
  box-shadow:
    0 14px 30px rgba(212, 129, 42, 0.24),
    0 0 0 5px rgba(212, 129, 42, 0.14);
}

.graph-node.active .node-dot {
  background: rgba(255, 255, 255, 0.86);
}

.graph-detail {
  border-left: 1px solid var(--color-border-light);
  padding: 18px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 14px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.7), rgba(255, 255, 255, 0)),
    var(--color-bg);
}

.graph-detail-head {
  display: grid;
  gap: 6px;
}

.graph-detail span,
.graph-detail small {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.graph-detail strong {
  font-size: 18px;
  line-height: 1.35;
}

.graph-detail p {
  margin: 0;
  line-height: 1.7;
  color: var(--color-text-secondary);
}

.graph-connection-list {
  display: grid;
  gap: 8px;
}

.graph-connection-list > span {
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.graph-connection-list button {
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 9px 10px;
  border: 1px solid var(--color-border-light);
  border-radius: 8px;
  background: var(--color-bg-elevated);
  text-align: left;
  transition: border-color var(--duration-fast), background var(--duration-fast);
}

.graph-connection-list button:hover {
  border-color: var(--color-primary-border);
  background: var(--color-primary-bg);
}

.graph-connection-list button strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.graph-connection-list button small {
  flex: 0 0 auto;
}

.map-empty,
.empty-state {
  color: var(--color-text-tertiary);
  padding: 48px;
  text-align: center;
}

.settings-panel {
  display: grid;
  gap: 14px;
}

.source-card {
  padding: 16px;
}

.source-options {
  display: inline-flex;
  gap: 8px;
  padding: 4px;
  border-radius: 8px;
  background: var(--color-bg-sunken);
}

.source-option {
  min-height: 32px;
  padding: 0 12px;
  border-radius: 7px;
  color: var(--color-text-secondary);
  font-weight: 700;
}

.source-option.active {
  background: var(--color-bg-elevated);
  color: var(--color-primary);
  box-shadow: var(--shadow-xs);
}

.source-option:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.server-status {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-weight: 700;
}

.server-status.active {
  color: var(--color-primary);
}

.settings-hint {
  margin: 10px 0 0;
  color: var(--color-text-tertiary);
  font-size: 12px;
  line-height: 1.6;
}

.admin-model-panel {
  display: grid;
  gap: 14px;
  margin-top: 4px;
  padding-top: 14px;
  border-top: 1px solid var(--color-border-light);
}

.admin-model-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.admin-model-head h2 {
  margin: 0;
  font-size: 16px;
}

.settings-cards {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.settings-card,
.settings-footer {
  border: 1px solid var(--color-border-light);
  border-radius: 8px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.72), rgba(255, 255, 255, 0)),
    var(--color-bg-elevated);
  box-shadow: var(--shadow-xs);
}

.settings-card {
  padding: 16px;
}

.settings-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

.settings-card h2 {
  margin: 0;
  font-size: 15px;
  letter-spacing: 0;
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.field {
  display: grid;
  gap: 7px;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.field.span-2 {
  grid-column: 1 / -1;
}

.settings-grid input,
.settings-grid select {
  min-height: 40px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-elevated);
  color: var(--color-text);
  padding: 0 11px;
  outline: none;
}

.settings-grid input:focus,
.settings-grid select:focus {
  border-color: var(--color-primary-border);
  box-shadow: 0 0 0 3px rgba(212, 129, 42, 0.1);
}

.settings-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
}

.switch-line {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text-secondary);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.switch-line.compact {
  font-size: 12px;
}

.switch-line input {
  display: none;
}

.switch-ui {
  width: 36px;
  height: 20px;
  border-radius: 999px;
  background: var(--color-bg-sunken);
  border: 1px solid var(--color-border);
  position: relative;
  transition: background var(--duration-fast), border-color var(--duration-fast);
}

.switch-ui::after {
  content: "";
  position: absolute;
  width: 14px;
  height: 14px;
  left: 2px;
  top: 2px;
  border-radius: 50%;
  background: var(--color-bg-elevated);
  box-shadow: var(--shadow-xs);
  transition: transform var(--duration-fast);
}

.switch-line input:checked + .switch-ui {
  background: var(--color-primary);
  border-color: var(--color-primary);
}

.switch-line input:checked + .switch-ui::after {
  transform: translateX(16px);
}

.settings-actions {
  justify-content: flex-end;
}

@media (max-width: 768px) {
  .ai-shell {
    padding: 16px;
  }

  .ai-header,
  .header-actions,
  .map-toolbar,
  .panel-toolbar {
    align-items: stretch;
    flex-direction: column;
  }

  .search-field {
    min-width: 0;
    max-width: none;
  }

  .title-row {
    align-items: flex-start;
    flex-direction: column;
    gap: 8px;
  }

  .overview-grid,
  .relation-grid,
  .settings-cards,
  .graph-fallback {
    grid-template-columns: 1fr;
  }

  .settings-grid {
    grid-template-columns: 1fr;
  }

  .graph-detail {
    border-left: 0;
    border-top: 1px solid var(--color-border-light);
  }

  .tabs {
    overflow-x: auto;
  }

  .tabs button {
    flex: 0 0 auto;
  }
}
</style>
