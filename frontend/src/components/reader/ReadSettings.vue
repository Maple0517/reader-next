<template>
  <div class="read-settings" :style="{ background: theme.popup, color: theme.fontColor }">
    <div class="settings-header">
      <h3 class="settings-title">设置</h3>
      <button class="reset-btn" @click="store.resetConfig()">重置为默认配置</button>
    </div>
    <div class="settings-sep"></div>

    <div class="settings-body">
      <!-- 阅读主题 -->
      <div class="setting-row">
        <label>阅读主题</label>
        <div class="theme-swatches">
          <button
            v-for="(t, i) in themePresets"
            :key="i"
            class="swatch"
            :class="{ active: store.themeIndex === i && !store.isNight }"
            :style="{ background: t.body }"
            @click="store.setThemeIndex(i)"
          >
            <svg v-if="store.themeIndex === i && !store.isNight" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><path d="M20 6 9 17l-5-5" /></svg>
          </button>
        </div>
      </div>

      <!-- 正文字体 -->
      <div class="setting-row">
        <label>正文字体</label>
        <div class="btn-group">
          <button
            v-for="f in fontPresets"
            :key="f.value"
            class="opt-btn"
            :class="{ active: config.fontFamily === f.value }"
            @click="store.updateConfig('fontFamily', f.value)"
          >{{ f.label }}</button>
        </div>
      </div>

      <!-- 简繁转换 -->
      <div class="setting-row">
        <label>简繁转换</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.chineseMode === 'simplified' }" @click="store.updateConfig('chineseMode', 'simplified')">简体</button>
          <button class="opt-btn" :class="{ active: config.chineseMode === 'traditional' }" @click="store.updateConfig('chineseMode', 'traditional')">繁体</button>
        </div>
      </div>

      <div class="settings-sep"></div>
      <!-- 预加载 -->
      <div class="setting-row">
        <label>&#x9884;&#x52A0;&#x8F7D;</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.enablePreload }" @click="store.updateConfig('enablePreload', true)">&#x5F00;&#x542F;</button>
          <button class="opt-btn" :class="{ active: !config.enablePreload }" @click="store.updateConfig('enablePreload', false)">&#x5173;&#x95ED;</button>
        </div>
      </div>

      <div class="setting-row setting-row-top chapter-summary-settings">
        <label>AI 摘要</label>
        <div class="chapter-summary-panel">
          <div class="btn-group">
            <button class="opt-btn" :class="{ active: config.enableChapterSummaryAuto }" @click="store.updateConfig('enableChapterSummaryAuto', true)">自动</button>
            <button class="opt-btn" :class="{ active: !config.enableChapterSummaryAuto }" @click="store.updateConfig('enableChapterSummaryAuto', false)">手动</button>
          </div>
          <p class="chapter-summary-hint">进入章节后延迟生成；已有缓存不会重复消耗模型。</p>

          <details v-if="chapterSummaryConfig?.isAdmin" class="chapter-summary-advanced">
            <summary>高级配置</summary>
            <div class="chapter-summary-grid">
              <label>
                功能启用
                <select v-model="chapterSummaryDraft.enabledText">
                  <option value="true">启用</option>
                  <option value="false">关闭</option>
                </select>
              </label>
              <label>
                默认自动
                <select v-model="chapterSummaryDraft.autoEnabledDefaultText">
                  <option value="true">自动</option>
                  <option value="false">手动</option>
                </select>
              </label>
              <label>
                详细程度
                <select v-model="chapterSummaryDraft.detailLevel">
                  <option value="short">简短</option>
                  <option value="normal">正常</option>
                  <option value="detailed">详细</option>
                </select>
              </label>
              <label>
                最大字数
                <input v-model.number="chapterSummaryDraft.maxWords" type="number" min="80" max="600">
              </label>
              <label>
                Temperature
                <input v-model.number="chapterSummaryDraft.temperature" type="number" min="0" max="1.5" step="0.1">
              </label>
              <label>
                最短正文
                <input v-model.number="chapterSummaryDraft.minContentChars" type="number" min="0" max="5000">
              </label>
            </div>
            <label class="chapter-summary-prompt">
              Prompt
              <textarea v-model="chapterSummaryDraft.prompt" rows="4"></textarea>
            </label>
            <button class="opt-btn wide" :disabled="savingChapterSummaryConfig" @click="saveChapterSummarySettings">
              {{ savingChapterSummaryConfig ? '保存中...' : '保存摘要配置' }}
            </button>
          </details>
        </div>
      </div>

      <!-- 字体大小 -->
      <div class="setting-row">
        <label>字体大小</label>
        <div class="stepper">
          <button class="step-btn" @click="step('fontSize', -1, 12, 32)">A-</button>
          <span class="step-val">{{ config.fontSize }}</span>
          <button class="step-btn" @click="step('fontSize', 1, 12, 32)">A+</button>
        </div>
      </div>

      <!-- 字体粗细 -->
      <div class="setting-row">
        <label>字体粗细</label>
        <div class="stepper">
          <button class="step-btn" @click="step('fontWeight', -100, 100, 900)">—</button>
          <span class="step-val">{{ config.fontWeight }}</span>
          <button class="step-btn" @click="step('fontWeight', 100, 100, 900)">+</button>
        </div>
      </div>

      <!-- 段落行高 -->
      <div class="setting-row">
        <label>段落行高</label>
        <div class="stepper">
          <button class="step-btn" @click="stepFloat('lineHeight', -0.1, 1.0, 3.0)">—</button>
          <span class="step-val">{{ config.lineHeight.toFixed(1) }}</span>
          <button class="step-btn" @click="stepFloat('lineHeight', 0.1, 1.0, 3.0)">+</button>
        </div>
      </div>

      <!-- 段落间距 -->
      <div class="setting-row">
        <label>段落间距</label>
        <div class="stepper">
          <button class="step-btn" @click="stepFloat('paragraphSpacing', -0.1, 0, 2.0)">—</button>
          <span class="step-val">{{ config.paragraphSpacing.toFixed(1) }}</span>
          <button class="step-btn" @click="stepFloat('paragraphSpacing', 0.1, 0, 2.0)">+</button>
        </div>
      </div>

      <div class="setting-row">
        <label>首行缩进</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.firstLineIndent }" @click="store.updateConfig('firstLineIndent', true)">开启</button>
          <button class="opt-btn" :class="{ active: !config.firstLineIndent }" @click="store.updateConfig('firstLineIndent', false)">关闭</button>
        </div>
      </div>

      <!-- 页面模式 -->
      <div class="setting-row">
        <label>页面模式</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.pageMode === 'auto' }" @click="store.updateConfig('pageMode', 'auto')">自适应</button>
          <button class="opt-btn" :class="{ active: config.pageMode === 'mobile' }" @click="store.updateConfig('pageMode', 'mobile')">手机模式</button>
        </div>
      </div>

      <!-- 页面宽度 -->
      <div class="setting-row">
        <label>页面宽度</label>
        <div class="stepper">
          <button class="step-btn" @click="step('pageWidth', -50, 400, 1200)">目-</button>
          <span class="step-val">{{ config.pageWidth }}</span>
          <button class="step-btn" @click="step('pageWidth', 50, 400, 1200)">目+</button>
        </div>
      </div>

      <div class="settings-sep"></div>

      <!-- 翻页方式 -->
      <div class="setting-row">
        <label>翻页方式</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.readMethod === '上下滑动' }" @click="store.updateConfig('readMethod', '上下滑动')">上下滑动</button>
          <button class="opt-btn" :class="{ active: config.readMethod === '左右翻页' }" @click="store.updateConfig('readMethod', '左右翻页')">左右翻页</button>
          <button class="opt-btn" :class="{ active: config.readMethod === '上下滚动' }" @click="store.updateConfig('readMethod', '上下滚动')">上下滚动</button>
          <button class="opt-btn" :class="{ active: config.readMethod === '上下滚动2' }" @click="store.updateConfig('readMethod', '上下滚动2')">上下滚动2</button>
        </div>
      </div>

      <!-- 动画时长 -->
      <div class="setting-row">
        <label>动画时长</label>
        <div class="stepper">
          <button class="step-btn" @click="step('animateDuration', -50, 0, 1000)">—</button>
          <span class="step-val">{{ config.animateDuration }}</span>
          <button class="step-btn" @click="step('animateDuration', 50, 0, 1000)">+</button>
        </div>
      </div>

      <!-- 自动阅读 -->
      <div class="setting-row">
        <label>自动阅读</label>
        <button 
          class="opt-btn wide" 
          :class="{ active: store.isAutoScrolling }"
          @click="store.isAutoScrolling = !store.isAutoScrolling"
        >
          {{ store.isAutoScrolling ? '停止滚动' : '开启自动平滑滚动' }}
        </button>
      </div>

      <!-- 自动翻页模式 -->
      <div class="setting-row">
        <label>滚动方式</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.autoPageMode === 'pixel' }" @click="store.updateConfig('autoPageMode', 'pixel')">像素滚动</button>
          <button class="opt-btn" :class="{ active: config.autoPageMode === 'paragraph' }" @click="store.updateConfig('autoPageMode', 'paragraph')">段落滚动</button>
        </div>
      </div>

      <!-- 滚动像素 -->
      <div class="setting-row">
        <label>滚动像素</label>
        <div class="stepper">
          <button class="step-btn" @click="step('scrollPixel', -1, 1, 10)">—</button>
          <span class="step-val">{{ config.scrollPixel }}</span>
          <button class="step-btn" @click="step('scrollPixel', 1, 1, 10)">+</button>
        </div>
      </div>

      <!-- 滚动速度 -->
      <div class="setting-row">
        <label>垂直速度</label>
        <div class="stepper">
          <button class="step-btn" @click="step('pageSpeed', -100, 100, 5000)">—</button>
          <span class="step-val">{{ config.pageSpeed }}</span>
          <button class="step-btn" @click="step('pageSpeed', 100, 100, 5000)">+</button>
        </div>
      </div>

      <!-- 点击翻页 (全屏热区) -->
      <div class="setting-row">
        <label>点击翻页</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.clickAction === 'auto' }" @click="store.updateConfig('clickAction', 'auto')">自动翻页</button>
          <button class="opt-btn" :class="{ active: config.clickAction === 'next' }" @click="store.updateConfig('clickAction', 'next')">仅下滚</button>
          <button class="opt-btn" :class="{ active: config.clickAction === 'none' }" @click="store.updateConfig('clickAction', 'none')">禁用</button>
        </div>
      </div>

      <!-- 选择文字 -->
      <div class="setting-row">
        <label>选择文字</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: config.selectAction === 'popup' }" @click="store.updateConfig('selectAction', 'popup')">操作弹窗</button>
          <button class="opt-btn" :class="{ active: config.selectAction === 'ignore' }" @click="store.updateConfig('selectAction', 'ignore')">忽略</button>
        </div>
      </div>

      <div class="settings-sep"></div>

      <!-- 朗读引擎 -->
      <div class="setting-row">
        <label>朗读引擎</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: store.speechConfig.provider === 'system' }" @click="store.setSpeechProvider('system')">系统语音</button>
          <button class="opt-btn" :class="{ active: store.speechConfig.provider === 'openai' }" @click="store.setSpeechProvider('openai')">OpenAI Speech</button>
        </div>
      </div>

      <div v-if="store.speechConfig.provider === 'system'" class="setting-row setting-row-top">
        <label>朗读音源</label>
        <select class="voice-select" :value="store.speechConfig.voiceName" @change="handleVoiceChange">
          <option value="">系统默认</option>
          <option v-for="voice in store.voiceList" :key="voice.name" :value="voice.name">
            {{ voice.name }} ({{ voice.lang }})
          </option>
        </select>
      </div>

      <template v-else>
        <div class="setting-row">
          <label>模型来源</label>
          <div class="btn-group">
            <button
              class="opt-btn"
              :class="{ active: store.speechConfig.openaiSource === 'browser' }"
              @click="store.setOpenAISpeechSource('browser')"
            >
              自己配置
            </button>
            <button
              class="opt-btn"
              :class="{ active: store.speechConfig.openaiSource === 'server' }"
              :disabled="serverModelLoaded && !canUseServerModel"
              @click="selectOpenAISpeechSource('server')"
            >
              后端配置
            </button>
          </div>
        </div>

        <template v-if="store.speechConfig.openaiSource === 'browser'">
        <div class="setting-row setting-row-top">
          <label>服务地址</label>
          <input
            class="voice-select"
            type="url"
            :value="store.speechConfig.openaiBaseUrl"
            placeholder="http://localhost:8825"
            @input="store.setOpenAISpeechBaseUrl(($event.target as HTMLInputElement).value)"
          >
        </div>

        <div class="setting-row setting-row-top">
          <label>API Key</label>
          <input
            class="voice-select"
            type="password"
            :value="store.speechConfig.openaiApiKey"
            placeholder="sk-..."
            autocomplete="off"
            @input="store.setOpenAISpeechApiKey(($event.target as HTMLInputElement).value)"
          >
        </div>

        <div class="setting-row setting-row-top">
          <label>语音模型</label>
          <input
            class="voice-select"
            type="text"
            :value="store.speechConfig.openaiModel"
            placeholder="gpt-4o-mini-tts"
            @input="store.setOpenAISpeechModel(($event.target as HTMLInputElement).value)"
          >
        </div>

        <div class="setting-row setting-row-top">
          <label>语音音色</label>
          <input
            class="voice-select"
            type="text"
            :value="store.speechConfig.openaiVoice"
            placeholder="alloy"
            @input="store.setOpenAISpeechVoice(($event.target as HTMLInputElement).value)"
          >
        </div>

        <div class="setting-row setting-row-top">
          <label>音频格式</label>
          <select
            class="voice-select"
            :value="store.speechConfig.openaiFormat"
            @change="store.setOpenAISpeechFormat(($event.target as HTMLSelectElement).value as 'mp3' | 'wav' | 'opus' | 'flac' | 'pcm')"
          >
            <option value="mp3">mp3</option>
            <option value="wav">wav</option>
            <option value="opus">opus</option>
            <option value="flac">flac</option>
            <option value="pcm">pcm</option>
          </select>
        </div>
        </template>

        <div v-else class="server-speech-note">
          <template v-if="canUseServerModel">
            使用管理员配置的 OpenAI Speech 模型、音色和音频格式。请求通过后端代理转发，浏览器不会保存后端 API Key。
          </template>
          <template v-else>
            当前账号没有使用后端模型配置的权限，请让管理员在用户管理中开启“AI 模型”，或切回自己配置。
          </template>
        </div>

        <div class="setting-row setting-row-top">
          <label>请求模式</label>
          <div class="btn-group">
            <button
              class="opt-btn"
              :class="{ active: store.speechConfig.openaiRequestMode === 'chunked' }"
              @click="store.setOpenAISpeechRequestMode('chunked')"
            >
              少字多请求
            </button>
            <button
              class="opt-btn"
              :class="{ active: store.speechConfig.openaiRequestMode === 'merged' }"
              @click="store.setOpenAISpeechRequestMode('merged')"
            >
              多字少请求
            </button>
          </div>
        </div>

        <div class="setting-hint">
          少字多请求会按短句细分并预加载更多片段；多字少请求会合并较短段落，只预加载下一段。
          <template v-if="store.speechConfig.openaiSource === 'browser'">URL 和 Key 仅保存在当前浏览器。</template>
          <template v-else>后端配置由管理员维护，权限不足时朗读请求会失败。</template>
        </div>
      </template>

      <div class="setting-row setting-row-top">
        <label>朗读语速</label>
        <div class="stepper">
          <button class="step-btn" @click="adjustSpeechRate(-0.1)">—</button>
          <span class="step-val">{{ store.speechConfig.speechRate.toFixed(1) }}</span>
          <button class="step-btn" @click="adjustSpeechRate(0.1)">+</button>
        </div>
      </div>

      <div v-if="store.speechConfig.provider === 'system'" class="setting-row">
        <label>朗读音调</label>
        <div class="stepper">
          <button class="step-btn" @click="adjustSpeechPitch(-0.1)">—</button>
          <span class="step-val">{{ store.speechConfig.speechPitch.toFixed(1) }}</span>
          <button class="step-btn" @click="adjustSpeechPitch(0.1)">+</button>
        </div>
      </div>

      <div class="setting-row setting-row-top">
        <label>定时停止</label>
        <div class="btn-group">
          <button class="opt-btn" :class="{ active: store.speechConfig.stopAfterMinutes === 0 }" @click="store.setSpeechStopTimer(0)">关闭</button>
          <button class="opt-btn" :class="{ active: store.speechConfig.stopAfterMinutes === 15 }" @click="store.setSpeechStopTimer(15)">15分钟</button>
          <button class="opt-btn" :class="{ active: store.speechConfig.stopAfterMinutes === 30 }" @click="store.setSpeechStopTimer(30)">30分钟</button>
          <button class="opt-btn" :class="{ active: store.speechConfig.stopAfterMinutes === 60 }" @click="store.setSpeechStopTimer(60)">60分钟</button>
          <button class="opt-btn" :class="{ active: store.speechConfig.stopAfterMinutes === 120 }" @click="store.setSpeechStopTimer(120)">120分钟</button>
        </div>
      </div>

      <div class="settings-sep"></div>

      <!-- 更多操作 -->
      <div class="setting-row">
        <label>离线缓存</label>
        <button class="opt-btn wide" @click="store.openPanel('cache', 'settings')">批量缓存章节</button>
      </div>

      <div class="setting-row">
        <label>内容净化</label>
        <button class="opt-btn wide" @click="store.openPanel('rule', 'settings')">管理净化规则</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useReaderStore, themePresets, fontPresets } from '../../stores/reader'
import { useAiBookStore } from '../../stores/aiBook'
import { useAppStore } from '../../stores/app'
import { getChapterSummaryConfig, saveChapterSummaryConfig } from '../../api/chapterSummary'
import type { ChapterSummaryConfigResponse } from '../../types'

const store = useReaderStore()
const aiBookStore = useAiBookStore()
const appStore = useAppStore()
const config = computed(() => store.config)
const theme = computed(() => store.currentTheme)
const serverModelLoaded = ref(false)
const canUseServerModel = computed(() => Boolean(aiBookStore.serverModelConfig?.canUseServerModel))

const chapterSummaryConfig = ref<ChapterSummaryConfigResponse | null>(null)
const savingChapterSummaryConfig = ref(false)
const chapterSummaryDraft = reactive({
  enabledText: 'true',
  autoEnabledDefaultText: 'true',
  detailLevel: 'normal' as 'short' | 'normal' | 'detailed',
  maxWords: 300,
  temperature: 0.3,
  minContentChars: 300,
  prompt: '',
})

function applyChapterSummaryDraft(response: ChapterSummaryConfigResponse) {
  chapterSummaryConfig.value = response
  chapterSummaryDraft.enabledText = response.config.enabled ? 'true' : 'false'
  chapterSummaryDraft.autoEnabledDefaultText = response.config.autoEnabledDefault ? 'true' : 'false'
  chapterSummaryDraft.detailLevel = response.config.detailLevel
  chapterSummaryDraft.maxWords = response.config.maxWords
  chapterSummaryDraft.temperature = response.config.temperature
  chapterSummaryDraft.minContentChars = response.config.minContentChars
  chapterSummaryDraft.prompt = response.config.prompt
}

async function loadChapterSummarySettings() {
  try {
    applyChapterSummaryDraft(await getChapterSummaryConfig())
  } catch {
    chapterSummaryConfig.value = null
  }
}

async function saveChapterSummarySettings() {
  if (!chapterSummaryConfig.value?.isAdmin) return
  savingChapterSummaryConfig.value = true
  try {
    const saved = await saveChapterSummaryConfig({
      enabled: chapterSummaryDraft.enabledText === 'true',
      autoEnabledDefault: chapterSummaryDraft.autoEnabledDefaultText === 'true',
      detailLevel: chapterSummaryDraft.detailLevel,
      maxWords: Number(chapterSummaryDraft.maxWords) || 300,
      temperature: Number(chapterSummaryDraft.temperature) || 0.3,
      minContentChars: Number(chapterSummaryDraft.minContentChars) || 300,
      prompt: chapterSummaryDraft.prompt,
    })
    applyChapterSummaryDraft(saved)
    appStore.showToast('摘要配置已保存', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '摘要配置保存失败', 'error')
  } finally {
    savingChapterSummaryConfig.value = false
  }
}

function step(key: 'fontSize' | 'fontWeight' | 'pageWidth' | 'animateDuration' | 'scrollPixel' | 'pageSpeed', delta: number, min: number, max: number) {
  const val = Math.max(min, Math.min(max, (config.value[key] as number) + delta))
  store.updateConfig(key, val)
}

function stepFloat(key: 'lineHeight' | 'paragraphSpacing', delta: number, min: number, max: number) {
  const val = Math.max(min, Math.min(max, parseFloat(((config.value[key] as number) + delta).toFixed(1))))
  store.updateConfig(key, val)
}

function adjustSpeechRate(delta: number) {
  const val = Math.max(0.5, Math.min(3, parseFloat((store.speechConfig.speechRate + delta).toFixed(1))))
  store.setSpeechRate(val)
}

function adjustSpeechPitch(delta: number) {
  const val = Math.max(0.5, Math.min(2, parseFloat((store.speechConfig.speechPitch + delta).toFixed(1))))
  store.setSpeechPitch(val)
}

function handleVoiceChange(event: Event) {
  const target = event.target as HTMLSelectElement | null
  store.setVoiceName(target?.value || '')
}

async function selectOpenAISpeechSource(source: 'browser' | 'server') {
  if (source === 'browser') {
    store.setOpenAISpeechSource('browser')
    return
  }
  if (!serverModelLoaded.value) {
    await aiBookStore.loadServerModelConfig({ force: true })
    serverModelLoaded.value = true
  }
  if (!canUseServerModel.value) {
    store.setOpenAISpeechSource('browser')
    appStore.showToast('当前账号没有使用后端模型配置的权限', 'warning')
    return
  }
  store.setOpenAISpeechSource('server')
}

onMounted(async () => {
  store.fetchVoices()
  await Promise.all([
    aiBookStore.loadServerModelConfig({ force: true }),
    loadChapterSummarySettings(),
  ])
  serverModelLoaded.value = true
  if (store.speechConfig.openaiSource === 'server' && !canUseServerModel.value) {
    store.setOpenAISpeechSource('browser')
  }
})
</script>

<style scoped>
.read-settings {
  width: 100%;
  height: 100%;
  overflow-y: auto;
  padding: 24px;
  transition: background 0.3s, color 0.3s;
  -webkit-overflow-scrolling: touch;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.settings-title {
  font-size: 20px;
  font-weight: 700;
}

.settings-sep {
  height: 1px;
  background: currentColor;
  opacity: 0.08;
  margin: 16px 0;
  width: 60px;
}

.settings-sep:first-of-type {
  background: var(--color-primary, #c97f3a);
  opacity: 1;
  height: 3px;
  border-radius: 2px;
}

.reset-btn {
  padding: 6px 16px;
  border-radius: 20px;
  border: 1px solid var(--color-primary, #c97f3a);
  color: var(--color-primary, #c97f3a);
  font-size: 13px;
  font-weight: 500;
  transition: all 0.2s;
  background: transparent;
}

.reset-btn:hover {
  background: var(--color-primary, #c97f3a);
  color: white;
}

.settings-body {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 20px;
}

.setting-row-top {
  align-items: flex-start;
}

.setting-row > label {
  min-width: 70px;
  font-size: 14px;
  font-weight: 500;
  opacity: 0.7;
  flex-shrink: 0;
}

.voice-select {
  flex: 1;
  min-width: 0;
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid rgba(0, 0, 0, 0.08);
  background: rgba(255, 255, 255, 0.6);
  color: inherit;
}

.setting-hint {
  margin-top: -8px;
  padding-left: 90px;
  font-size: 12px;
  line-height: 1.5;
  opacity: 0.65;
}

.server-speech-note {
  margin-left: 90px;
  padding: 12px 14px;
  border-radius: 12px;
  border: 1px solid rgba(201, 127, 58, 0.18);
  background: rgba(201, 127, 58, 0.08);
  font-size: 12px;
  line-height: 1.6;
  color: inherit;
}


.chapter-summary-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: stretch;
}

.chapter-summary-panel > .btn-group {
  justify-content: flex-end;
}

.chapter-summary-hint {
  margin: 0;
  font-size: 12px;
  opacity: 0.55;
  text-align: right;
  line-height: 1.5;
}

.chapter-summary-advanced {
  width: 100%;
  border: 1px solid rgba(128, 128, 128, 0.18);
  border-radius: 12px;
  padding: 12px;
  box-sizing: border-box;
}

.chapter-summary-advanced summary {
  cursor: pointer;
  color: var(--color-primary, #c97f3a);
  font-size: 13px;
  font-weight: 600;
}

.chapter-summary-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 10px;
}

.chapter-summary-grid label,
.chapter-summary-prompt {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  font-size: 12px;
  opacity: 0.85;
}

.chapter-summary-grid input,
.chapter-summary-grid select,
.chapter-summary-prompt textarea {
  width: 100%;
  border: 1px solid rgba(128, 128, 128, 0.22);
  border-radius: 8px;
  padding: 7px 9px;
  background: transparent;
  color: inherit;
}

.chapter-summary-prompt {
  margin-top: 12px;
}

.chapter-summary-prompt textarea {
  min-height: 120px;
  resize: vertical;
}

.chapter-summary-advanced .opt-btn.wide {
  width: 100%;
  margin-top: 12px;
  padding: 10px 12px;
  border-radius: 12px;
}

/* Theme swatches */
.theme-swatches {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.swatch {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 3px rgba(0,0,0,0.12);
}

.swatch:hover {
  transform: scale(1.1);
}

.swatch.active {
  border-color: var(--color-primary, #c97f3a);
}

.swatch svg {
  width: 16px;
  height: 16px;
  color: var(--color-primary, #c97f3a);
}

/* Button groups */
.btn-group {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.opt-btn {
  padding: 6px 16px;
  border-radius: 20px;
  font-size: 13px;
  font-weight: 500;
  border: 1px solid rgba(0,0,0,0.12);
  background: transparent;
  color: inherit;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}

.opt-btn:hover {
  border-color: var(--color-primary, #c97f3a);
}

.opt-btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.opt-btn.active {
  background: var(--color-primary, #c97f3a);
  color: white;
  border-color: var(--color-primary, #c97f3a);
}

/* Steppers */
.stepper {
  display: flex;
  align-items: center;
  border-radius: 20px;
  border: 1px solid rgba(0,0,0,0.12);
  overflow: hidden;
}

.step-btn {
  padding: 6px 14px;
  font-size: 13px;
  font-weight: 600;
  color: inherit;
  background: transparent;
  cursor: pointer;
  transition: background 0.15s;
  border: none;
  min-width: 40px;
}

.step-btn:hover {
  background: rgba(0,0,0,0.06);
}

.step-btn:active {
  background: rgba(0,0,0,0.1);
}

.step-val {
  min-width: 50px;
  text-align: center;
  font-size: 14px;
  font-variant-numeric: tabular-nums;
  border-left: 1px solid rgba(0,0,0,0.08);
  border-right: 1px solid rgba(0,0,0,0.08);
  padding: 6px 0;
}

@media (max-width: 420px) {
  .read-settings {
    padding: 16px;
  }

  .settings-header {
    align-items: center;
    gap: 8px;
  }

  .settings-title {
    font-size: 18px;
  }

  .reset-btn {
    padding: 6px 12px;
    font-size: 12px;
  }

  .setting-row {
    align-items: center;
    gap: 12px;
  }

  .setting-row label {
    min-width: 60px;
    font-size: 13px;
  }

  .btn-group {
    gap: 6px;
    flex: 0 1 auto;
  }

  .stepper,
  .voice-select,
  .theme-swatches {
    flex: 0 1 auto;
    min-width: 0;
    max-width: 100%;
  }

  .stepper {
    width: auto;
    display: inline-flex;
  }

  .opt-btn {
    padding: 6px 12px;
    font-size: 12px;
  }

  .step-btn {
    min-width: 36px;
    padding: 6px 10px;
  }

  .step-val {
    min-width: 42px;
    font-size: 13px;
  }

  .voice-select {
    width: auto;
  }

  .setting-hint {
    padding-left: 0;
    margin-top: -12px;
  }

  .server-speech-note {
    margin-left: 0;
  }
}
</style>
