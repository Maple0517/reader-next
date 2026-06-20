<template>
  <div
    class="reader-view"
    :class="{ 'disable-system-callout': disableSystemCallout }"
    :style="{
      background: theme.body,
      color: theme.fontColor,
      fontFamily: currentFontFamily,
      '--color-primary': '#c97f3a',
      '--reader-summary-sider-width': showSideChapterSummary ? `${chapterSummarySiderWidth}px` : '0px'
    }"
    @click="handleBackgroundClick"
    @contextmenu.prevent="handleContextMenu"
  >
    <!-- Left Drawer Panels -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="store.activePanel" class="reader-overlay" @click="store.closePanel()"></div>
      </Transition>
      <Transition name="slide-left">
        <div v-if="store.activePanel" class="reader-drawer" :style="{ background: chromeTheme.popup }">
          <ReaderCatalog
            v-if="store.activePanel === 'catalog' || store.activePanel === 'bookmark'"
            :initial-tab="store.activePanel === 'bookmark' ? 'bookmarks' : 'chapters'"
            @jump-chapter="jumpFromCatalog"
          />
          <ReadSettings v-else-if="store.activePanel === 'settings'" />
          <ReaderBookshelf v-else-if="store.activePanel === 'bookshelf'" />
          <ReaderSource v-else-if="store.activePanel === 'source'" />
          <ReplaceRuleManager v-else-if="store.activePanel === 'rule'" />
          <CacheManager v-else-if="store.activePanel === 'cache'" />
        </div>
      </Transition>
    </Teleport>

    <!-- PC Desktop Toolbars (Always shown) -->
    <ReaderSidebar
      v-if="!isMobile"
      @goHome="goHome"
      @scrollTop="scrollToTop"
      @scrollBottom="scrollToBottom"
    />
    <ReaderToolbar
      v-if="!isMobile"
      :is-speaking="store.isSpeaking"
      :is-paused="store.isPaused"
      :show-chapter-summary="showChapterSummary"
      @bookmark="toggleBookmark"
      @search="toggleSearch"
      @info="openInfo"
      @ai="openAiBook"
      @toggleChapterSummary="toggleChapterSummary"
      @tts="handleTTS"
      @prev="prevChapter"
      @next="nextChapter"
      @progress="openCachePanel"
    />

    <!-- Mobile Controls (Click to toggle) -->
    <ReaderMobileControls
      v-if="isMobile"
      :show="showControls || !!store.activePanel"
      @goHome="goHome"
      @scrollTop="scrollToTop"
      @scrollBottom="scrollToBottom"
      @prev="prevChapter"
      @next="nextChapter"
      @bookmark="toggleBookmark"
      @search="openSearch"
      @info="openInfo"
      @ai="openAiBook"
      @tts="handleTTS"
      @progress="openCachePanel"
    />

    <ReaderTtsPanel
      :show="showTTSPanel"
      :theme="chromeTheme"
      :chapter-title="store.currentChapter?.title"
      :provider="store.speechConfig.provider"
      :provider-label="store.speechProviderLabel"
      :is-speaking="store.isSpeaking"
      :is-loading="store.isSpeechLoading"
      :is-paused="store.isPaused"
      :voices="store.voiceList"
      :voice-name="store.speechConfig.voiceName"
      :rate="store.speechConfig.speechRate"
      :pitch="store.speechConfig.speechPitch"
      :supports-pitch="store.speechConfig.provider === 'system'"
      :openai-model="store.speechConfig.openaiModel"
      :openai-voice="store.speechConfig.openaiVoice"
      :openai-source="store.speechConfig.openaiSource"
      :stop-after-minutes="store.speechConfig.stopAfterMinutes"
      :timer-text="speechTimerText"
      @close="closeTTSPanel"
      @prev="speechPrev"
      @toggle-play="toggleSpeechFromPanel"
      @stop="handleStopTTS"
      @next="speechNext"
      @voice-change="changeVoice"
      @openai-voice-change="changeOpenAIVoice"
      @rate-change="adjustSpeechRate"
      @pitch-change="adjustSpeechPitch"
      @timer-change="setSpeechTimer"
    />

    <!-- Main Content Area -->
    <div
      class="reader-scroll-container"
      :class="{ 'horizontal-page-mode': isHorizontalPageMode }"
      ref="scrollContainerRef"
      @scroll="handleScroll"
      @mousedown="stopAutoScroll"
      @touchstart="handleTouchStart"
      @touchmove="handleTouchMove"
      @touchend="handleTouchEnd"
      @click="handleGlobalClick"
    >
      <div v-if="store.loading" class="content-loading">
        <div class="loading-spinner"></div>
      </div>

      <div v-else-if="offlineBannerText" class="offline-banner">
        {{ offlineBannerText }}
      </div>

      <article
        v-if="!store.loading && !isContinuousMode"
        class="chapter-content"
        :class="{ 'horizontal-page-article': isHorizontalPageMode }"
        :style="{
          maxWidth: isHorizontalPageMode ? 'none' : (config.pageWidth + 'px'),
          fontSize: config.fontSize + 'px',
          fontWeight: config.fontWeight,
          lineHeight: config.lineHeight,
          '--reader-page-width': config.pageWidth + 'px',
          '--reader-side-padding': '24px',
          '--reader-page-step': horizontalPageStepStyle,
        }"
      >
        <div v-if="isHorizontalPageMode" class="horizontal-page-layout">
          <section class="horizontal-content-page">
            <div
              ref="chapterTextRef"
              class="horizontal-pages"
              :style="{
                transform: horizontalPageTransform,
                transitionDuration: horizontalPageTransitionDuration,
              }"
            >
              <section v-for="(page, idx) in horizontalPages" :key="`h-page-${idx}`" class="horizontal-page">
                <div
                  class="chapter-text horizontal-page-content"
                  :style="{
                    '--p-spacing': config.paragraphSpacing + 'em',
                  }"
                  v-html="page"
                ></div>
              </section>
            </div>
          </section>
        </div>

        <div v-else>
          <div class="chapter-title">{{ store.currentChapter?.title || '加载中...' }}</div>

          <button
            v-if="showCollapsedChapterSummary"
            class="chapter-summary-collapsed-pill"
            type="button"
            @click="expandCollapsedChapterSummary"
          >
            <span class="summary-kicker">摘要</span>
            <span class="summary-muted">{{ chapterSummary ? '展开管理' : '打开摘要设置' }}</span>
          </button>

          <section v-if="showInlineChapterSummary" class="chapter-summary-card">
            <div class="chapter-summary-header reader-ui-font">
              <div>
                <div class="summary-kicker">摘要</div>
                <div class="summary-muted">{{ store.currentChapter?.title || '当前章节' }}</div>
              </div>
              <div class="summary-tabs" role="tablist" aria-label="摘要面板">
                <button
                  class="summary-tab"
                  :class="{ active: chapterSummaryActiveTab === 'content' }"
                  :aria-selected="chapterSummaryActiveTab === 'content'"
                  role="tab"
                  type="button"
                  @click="chapterSummaryActiveTab = 'content'"
                >正文</button>
                <button
                  class="summary-tab"
                  :class="{ active: chapterSummaryActiveTab === 'settings' }"
                  :aria-selected="chapterSummaryActiveTab === 'settings'"
                  role="tab"
                  type="button"
                  @click="chapterSummaryActiveTab = 'settings'"
                >设置</button>
              </div>
            </div>

            <section v-if="chapterSummaryActiveTab === 'content'" class="chapter-summary-body" role="tabpanel" :style="chapterSummaryBodyStyle">
              <div v-if="chapterSummaryStatus === 'loading'" class="summary-skeleton" aria-label="摘要生成中">
                <span></span>
                <span></span>
                <span></span>
              </div>
              <p v-else-if="chapterSummary?.summary" class="summary-main">{{ chapterSummary.summary }}</p>
              <p v-else-if="chapterSummaryError" class="summary-error">{{ chapterSummaryError }}</p>
              <p v-else class="summary-main summary-muted">当前章节还没有摘要。</p>
              <div
                v-if="chapterSummary?.keyPoints.length"
                class="summary-list"
                :class="`style-${config.chapterSummaryKeyPointStyle}`"
              >
                <strong>要点</strong>
                <ul>
                  <li v-for="item in chapterSummary.keyPoints" :key="item">{{ item }}</li>
                </ul>
              </div>
              <div class="summary-actions reader-ui-font">
                <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryForCurrentChapter(Boolean(chapterSummary))">
                  {{ chapterSummary ? '重新生成' : '生成摘要' }}
                </button>
                <button v-if="chapterSummary" class="summary-action" @click.stop="copyChapterSummary">复制</button>
              </div>
            </section>
            <section v-else class="chapter-summary-settings-panel reader-ui-font" role="tabpanel">
              <div class="summary-setting-group">
                <div class="summary-setting-title">显示</div>
                <div class="summary-setting-row">
                  <span>摘要栏</span>
                  <div class="summary-switch">
                    <button class="active" type="button">显示</button>
                    <button type="button" @click="hideChapterSummary">隐藏</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>位置</span>
                  <div class="summary-switch">
                    <button :class="{ active: config.chapterSummaryLayout === 'auto' }" type="button" @click="store.updateConfig('chapterSummaryLayout', 'auto')">智能</button>
                    <button :class="{ active: config.chapterSummaryLayout === 'side' }" type="button" @click="store.updateConfig('chapterSummaryLayout', 'side')">右侧</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>栏宽</span>
                  <div class="summary-stepper">
                    <button type="button" @click="adjustChapterSummarySiderWidth(-20)">−</button>
                    <span>{{ chapterSummarySiderWidth }}</span>
                    <button type="button" @click="adjustChapterSummarySiderWidth(20)">+</button>
                  </div>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">阅读</div>
                <div class="summary-setting-row">
                  <span>摘要字号</span>
                  <div class="summary-stepper">
                    <button type="button" @click="adjustChapterSummaryFontSize(-1)">A-</button>
                    <span>{{ config.chapterSummaryFontSize }}</span>
                    <button type="button" @click="adjustChapterSummaryFontSize(1)">A+</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>要点样式</span>
                  <div class="summary-switch">
                    <button :class="{ active: config.chapterSummaryKeyPointStyle === 'card' }" type="button" @click="store.updateConfig('chapterSummaryKeyPointStyle', 'card')">整块</button>
                    <button :class="{ active: config.chapterSummaryKeyPointStyle === 'list' }" type="button" @click="store.updateConfig('chapterSummaryKeyPointStyle', 'list')">列表</button>
                  </div>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">生成</div>
                <div class="summary-setting-row">
                  <span>功能启用</span>
                  <div class="summary-switch">
                    <button :class="{ active: chapterSummaryConfigDraft.enabledText === 'true' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.enabledText = 'true'">开</button>
                    <button :class="{ active: chapterSummaryConfigDraft.enabledText === 'false' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.enabledText = 'false'">关</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>自动生成</span>
                  <div class="summary-switch">
                    <button :class="{ active: config.enableChapterSummaryAuto }" type="button" @click="store.updateConfig('enableChapterSummaryAuto', true)">开</button>
                    <button :class="{ active: !config.enableChapterSummaryAuto }" type="button" @click="store.updateConfig('enableChapterSummaryAuto', false)">关</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>详细程度</span>
                  <div class="summary-switch">
                    <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'short' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'short'">短</button>
                    <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'normal' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'normal'">正常</button>
                    <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'detailed' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'detailed'">详细</button>
                  </div>
                </div>
                <label class="summary-setting-field">
                  <span>最大字数</span>
                  <input v-model.number="chapterSummaryConfigDraft.maxWords" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="80" max="600">
                </label>
                <label class="summary-setting-field">
                  <span>最短正文</span>
                  <input v-model.number="chapterSummaryConfigDraft.minContentChars" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="0" max="5000">
                </label>
                <label class="summary-setting-field">
                  <span>Temperature</span>
                  <input v-model.number="chapterSummaryConfigDraft.temperature" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="0" max="1.5" step="0.1">
                </label>
                <div class="summary-actions compact">
                  <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryAfterSavingSettings(false)">生成摘要</button>
                  <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryAfterSavingSettings(true)">重新生成</button>
                  <button class="summary-action" :disabled="savingChapterSummaryConfig || !chapterSummaryConfig?.isAdmin" @click="saveChapterSummaryGenerationSettings">
                    {{ savingChapterSummaryConfig ? '保存中...' : '保存生成设置' }}
                  </button>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">Prompt</div>
                <textarea v-model="chapterSummaryConfigDraft.prompt" :disabled="!chapterSummaryConfig?.isAdmin" class="summary-prompt-input" rows="6"></textarea>
                <div class="summary-actions compact">
                  <button class="summary-action" :disabled="!chapterSummaryConfig?.isAdmin" @click="restoreDefaultChapterSummaryPrompt">恢复当前</button>
                  <button class="summary-action" :disabled="savingChapterSummaryConfig || !chapterSummaryConfig?.isAdmin" @click="saveChapterSummaryGenerationSettings">保存 Prompt</button>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">模型</div>
                <p class="summary-setting-note">摘要使用后端文本模型配置。API Key、Base URL 和模型名在 AI资料中统一管理。</p>
                <button class="summary-action" type="button" @click="openAiBackendSettings">打开 AI 后端设置</button>
              </div>
            </section>
          </section>

          <div
            ref="chapterTextRef"
            class="chapter-text"
            :style="{
              '--p-spacing': config.paragraphSpacing + 'em',
            }"
            v-html="formattedContent"
          ></div>

          <div class="chapter-footer">
            <button class="next-btn" :disabled="!store.hasNext" @click="nextChapter">
              {{ store.hasNext ? '下一章' : '没有更多了' }}
            </button>
          </div>
        </div>
      </article>

      <Transition name="fade">
        <div v-if="!store.loading && isHorizontalPageMode && isHorizontalAtEnd" class="horizontal-next-floating">
          <button class="next-btn" :disabled="!store.hasNext" @click="nextChapter">
            {{ store.hasNext ? '下一章' : '没有更多了' }}
          </button>
        </div>
      </Transition>

      <div
        v-if="!store.loading && isContinuousMode"
        class="continuous-reading"
        :style="{
          maxWidth: config.pageWidth + 'px',
          fontSize: config.fontSize + 'px',
          fontWeight: config.fontWeight,
          lineHeight: config.lineHeight,
        }"
      >
        <div v-if="continuousLoadingPrev" class="continuous-loading-inline">正在加载上一章...</div>

        <section
          v-for="chapter in continuousChapters"
          :key="chapter.index"
          class="chapter-content continuous-chapter"
          :data-chapter-index="chapter.index"
        >
          <div class="chapter-title">{{ chapter.title }}</div>

          <button
            v-if="showCollapsedChapterSummary && chapter.index === store.currentIndex"
            class="chapter-summary-collapsed-pill"
            type="button"
            @click="expandCollapsedChapterSummary"
          >
            <span class="summary-kicker">摘要</span>
            <span class="summary-muted">{{ chapterSummary ? '展开管理' : '打开摘要设置' }}</span>
          </button>

          <section
            v-if="showInlineChapterSummary && chapter.index === store.currentIndex"
            class="chapter-summary-card"
          >
            <div class="chapter-summary-header reader-ui-font">
              <div>
                <div class="summary-kicker">摘要</div>
                <div class="summary-muted">{{ chapter.title || '当前章节' }}</div>
              </div>
              <div class="summary-tabs" role="tablist" aria-label="摘要面板">
                <button
                  class="summary-tab"
                  :class="{ active: chapterSummaryActiveTab === 'content' }"
                  :aria-selected="chapterSummaryActiveTab === 'content'"
                  role="tab"
                  type="button"
                  @click="chapterSummaryActiveTab = 'content'"
                >正文</button>
                <button
                  class="summary-tab"
                  :class="{ active: chapterSummaryActiveTab === 'settings' }"
                  :aria-selected="chapterSummaryActiveTab === 'settings'"
                  role="tab"
                  type="button"
                  @click="chapterSummaryActiveTab = 'settings'"
                >设置</button>
              </div>
            </div>

            <section v-if="chapterSummaryActiveTab === 'content'" class="chapter-summary-body" role="tabpanel" :style="chapterSummaryBodyStyle">
              <div v-if="chapterSummaryStatus === 'loading'" class="summary-skeleton" aria-label="摘要生成中">
                <span></span>
                <span></span>
                <span></span>
              </div>
              <p v-else-if="chapterSummary?.summary" class="summary-main">{{ chapterSummary.summary }}</p>
              <p v-else-if="chapterSummaryError" class="summary-error">{{ chapterSummaryError }}</p>
              <p v-else class="summary-main summary-muted">当前章节还没有摘要。</p>
              <div
                v-if="chapterSummary?.keyPoints.length"
                class="summary-list"
                :class="`style-${config.chapterSummaryKeyPointStyle}`"
              >
                <strong>要点</strong>
                <ul>
                  <li v-for="item in chapterSummary.keyPoints" :key="item">{{ item }}</li>
                </ul>
              </div>
              <div class="summary-actions reader-ui-font">
                <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryForCurrentChapter(Boolean(chapterSummary))">
                  {{ chapterSummary ? '重新生成' : '生成摘要' }}
                </button>
                <button v-if="chapterSummary" class="summary-action" @click.stop="copyChapterSummary">复制</button>
              </div>
            </section>
            <section v-else class="chapter-summary-settings-panel reader-ui-font" role="tabpanel">
              <div class="summary-setting-group">
                <div class="summary-setting-title">显示</div>
                <div class="summary-setting-row">
                  <span>摘要栏</span>
                  <div class="summary-switch">
                    <button class="active" type="button">显示</button>
                    <button type="button" @click="hideChapterSummary">隐藏</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>位置</span>
                  <div class="summary-switch">
                    <button :class="{ active: config.chapterSummaryLayout === 'auto' }" type="button" @click="store.updateConfig('chapterSummaryLayout', 'auto')">智能</button>
                    <button :class="{ active: config.chapterSummaryLayout === 'side' }" type="button" @click="store.updateConfig('chapterSummaryLayout', 'side')">右侧</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>栏宽</span>
                  <div class="summary-stepper">
                    <button type="button" @click="adjustChapterSummarySiderWidth(-20)">−</button>
                    <span>{{ chapterSummarySiderWidth }}</span>
                    <button type="button" @click="adjustChapterSummarySiderWidth(20)">+</button>
                  </div>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">阅读</div>
                <div class="summary-setting-row">
                  <span>摘要字号</span>
                  <div class="summary-stepper">
                    <button type="button" @click="adjustChapterSummaryFontSize(-1)">A-</button>
                    <span>{{ config.chapterSummaryFontSize }}</span>
                    <button type="button" @click="adjustChapterSummaryFontSize(1)">A+</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>要点样式</span>
                  <div class="summary-switch">
                    <button :class="{ active: config.chapterSummaryKeyPointStyle === 'card' }" type="button" @click="store.updateConfig('chapterSummaryKeyPointStyle', 'card')">整块</button>
                    <button :class="{ active: config.chapterSummaryKeyPointStyle === 'list' }" type="button" @click="store.updateConfig('chapterSummaryKeyPointStyle', 'list')">列表</button>
                  </div>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">生成</div>
                <div class="summary-setting-row">
                  <span>功能启用</span>
                  <div class="summary-switch">
                    <button :class="{ active: chapterSummaryConfigDraft.enabledText === 'true' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.enabledText = 'true'">开</button>
                    <button :class="{ active: chapterSummaryConfigDraft.enabledText === 'false' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.enabledText = 'false'">关</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>自动生成</span>
                  <div class="summary-switch">
                    <button :class="{ active: config.enableChapterSummaryAuto }" type="button" @click="store.updateConfig('enableChapterSummaryAuto', true)">开</button>
                    <button :class="{ active: !config.enableChapterSummaryAuto }" type="button" @click="store.updateConfig('enableChapterSummaryAuto', false)">关</button>
                  </div>
                </div>
                <div class="summary-setting-row">
                  <span>详细程度</span>
                  <div class="summary-switch">
                    <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'short' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'short'">短</button>
                    <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'normal' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'normal'">正常</button>
                    <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'detailed' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'detailed'">详细</button>
                  </div>
                </div>
                <label class="summary-setting-field">
                  <span>最大字数</span>
                  <input v-model.number="chapterSummaryConfigDraft.maxWords" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="80" max="600">
                </label>
                <label class="summary-setting-field">
                  <span>最短正文</span>
                  <input v-model.number="chapterSummaryConfigDraft.minContentChars" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="0" max="5000">
                </label>
                <label class="summary-setting-field">
                  <span>Temperature</span>
                  <input v-model.number="chapterSummaryConfigDraft.temperature" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="0" max="1.5" step="0.1">
                </label>
                <div class="summary-actions compact">
                  <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryAfterSavingSettings(false)">生成摘要</button>
                  <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryAfterSavingSettings(true)">重新生成</button>
                  <button class="summary-action" :disabled="savingChapterSummaryConfig || !chapterSummaryConfig?.isAdmin" @click="saveChapterSummaryGenerationSettings">
                    {{ savingChapterSummaryConfig ? '保存中...' : '保存生成设置' }}
                  </button>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">Prompt</div>
                <textarea v-model="chapterSummaryConfigDraft.prompt" :disabled="!chapterSummaryConfig?.isAdmin" class="summary-prompt-input" rows="6"></textarea>
                <div class="summary-actions compact">
                  <button class="summary-action" :disabled="!chapterSummaryConfig?.isAdmin" @click="restoreDefaultChapterSummaryPrompt">恢复当前</button>
                  <button class="summary-action" :disabled="savingChapterSummaryConfig || !chapterSummaryConfig?.isAdmin" @click="saveChapterSummaryGenerationSettings">保存 Prompt</button>
                </div>
              </div>
              <div class="summary-setting-group">
                <div class="summary-setting-title">模型</div>
                <p class="summary-setting-note">摘要使用后端文本模型配置。API Key、Base URL 和模型名在 AI资料中统一管理。</p>
                <button class="summary-action" type="button" @click="openAiBackendSettings">打开 AI 后端设置</button>
              </div>
            </section>
          </section>

          <div
            class="chapter-text"
            data-role="continuous"
            :data-chapter-index="chapter.index"
            :style="{
              '--p-spacing': config.paragraphSpacing + 'em',
            }"
            v-html="chapter.html"
          ></div>

          <div v-if="chapter.index === continuousChapters[continuousChapters.length - 1]?.index" class="chapter-footer">
            <button class="next-btn" :disabled="!store.hasNext" @click="nextChapter">
              {{ store.hasNext ? '继续加载下一章' : '已经到底了' }}
            </button>
          </div>
        </section>

        <div v-if="continuousLoadingNext" class="continuous-loading-inline">正在加载下一章...</div>
      </div>
    </div>

    <aside
      v-if="showSideChapterSummary"
      class="chapter-summary-sider"
      :class="{ resizing: chapterSummarySiderResizing }"
      :style="chapterSummarySiderStyle"
      @click.stop
    >
      <div class="chapter-summary-resize-handle" @pointerdown="startChapterSummarySiderResize"></div>
      <div class="chapter-summary-sider-head reader-ui-font">
        <div>
          <div class="summary-kicker">摘要</div>
          <div class="summary-muted">{{ store.currentChapter?.title || '当前章节' }}</div>
        </div>
        <div class="summary-tabs" role="tablist" aria-label="摘要面板">
          <button
            class="summary-tab"
            :class="{ active: chapterSummaryActiveTab === 'content' }"
            :aria-selected="chapterSummaryActiveTab === 'content'"
            role="tab"
            type="button"
            @click="chapterSummaryActiveTab = 'content'"
          >正文</button>
          <button
            class="summary-tab"
            :class="{ active: chapterSummaryActiveTab === 'settings' }"
            :aria-selected="chapterSummaryActiveTab === 'settings'"
            role="tab"
            type="button"
            @click="chapterSummaryActiveTab = 'settings'"
          >设置</button>
        </div>
      </div>

      <section v-if="chapterSummaryActiveTab === 'content'" class="chapter-summary-card side" role="tabpanel">
        <div class="chapter-summary-body" :style="chapterSummaryBodyStyle">
          <div v-if="chapterSummaryStatus === 'loading'" class="summary-skeleton" aria-label="摘要生成中">
            <span></span>
            <span></span>
            <span></span>
          </div>
          <p v-else-if="chapterSummary?.summary" class="summary-main">{{ chapterSummary.summary }}</p>
          <p v-else-if="chapterSummaryError" class="summary-error">{{ chapterSummaryError }}</p>
          <p v-else class="summary-main summary-muted">当前章节还没有摘要。</p>
          <div
            v-if="chapterSummary?.keyPoints.length"
            class="summary-list"
            :class="`style-${config.chapterSummaryKeyPointStyle}`"
          >
            <strong>要点</strong>
            <ul>
              <li v-for="item in chapterSummary.keyPoints" :key="item">{{ item }}</li>
            </ul>
          </div>
          <div class="summary-actions reader-ui-font">
            <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryForCurrentChapter(Boolean(chapterSummary))">
              {{ chapterSummary ? '重新生成' : '生成摘要' }}
            </button>
            <button v-if="chapterSummary" class="summary-action" @click.stop="copyChapterSummary">复制</button>
            <button class="summary-action" @click="hideChapterSummary">隐藏</button>
          </div>
        </div>
      </section>
      <section v-else class="chapter-summary-settings-panel reader-ui-font" role="tabpanel">
        <div class="summary-setting-group">
          <div class="summary-setting-title">显示</div>
          <div class="summary-setting-row">
            <span>摘要栏</span>
            <div class="summary-switch">
              <button class="active" type="button">显示</button>
                <button type="button" @click="hideChapterSummary">隐藏</button>
            </div>
          </div>
          <div class="summary-setting-row">
            <span>位置</span>
            <div class="summary-switch">
              <button :class="{ active: config.chapterSummaryLayout === 'auto' }" type="button" @click="store.updateConfig('chapterSummaryLayout', 'auto')">智能</button>
              <button :class="{ active: config.chapterSummaryLayout === 'side' }" type="button" @click="store.updateConfig('chapterSummaryLayout', 'side')">右侧</button>
            </div>
          </div>
          <div class="summary-setting-row">
            <span>栏宽</span>
            <div class="summary-stepper">
              <button type="button" @click="adjustChapterSummarySiderWidth(-20)">−</button>
              <span>{{ chapterSummarySiderWidth }}</span>
              <button type="button" @click="adjustChapterSummarySiderWidth(20)">+</button>
            </div>
          </div>
        </div>
        <div class="summary-setting-group">
          <div class="summary-setting-title">阅读</div>
          <div class="summary-setting-row">
            <span>摘要字号</span>
            <div class="summary-stepper">
              <button type="button" @click="adjustChapterSummaryFontSize(-1)">A-</button>
              <span>{{ config.chapterSummaryFontSize }}</span>
              <button type="button" @click="adjustChapterSummaryFontSize(1)">A+</button>
            </div>
          </div>
          <div class="summary-setting-row">
            <span>要点样式</span>
            <div class="summary-switch">
              <button :class="{ active: config.chapterSummaryKeyPointStyle === 'card' }" type="button" @click="store.updateConfig('chapterSummaryKeyPointStyle', 'card')">整块</button>
              <button :class="{ active: config.chapterSummaryKeyPointStyle === 'list' }" type="button" @click="store.updateConfig('chapterSummaryKeyPointStyle', 'list')">列表</button>
            </div>
          </div>
        </div>
        <div class="summary-setting-group">
          <div class="summary-setting-title">生成</div>
          <div class="summary-setting-row">
            <span>功能启用</span>
            <div class="summary-switch">
              <button :class="{ active: chapterSummaryConfigDraft.enabledText === 'true' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.enabledText = 'true'">开</button>
              <button :class="{ active: chapterSummaryConfigDraft.enabledText === 'false' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.enabledText = 'false'">关</button>
            </div>
          </div>
          <div class="summary-setting-row">
            <span>自动生成</span>
            <div class="summary-switch">
              <button :class="{ active: config.enableChapterSummaryAuto }" type="button" @click="store.updateConfig('enableChapterSummaryAuto', true)">开</button>
              <button :class="{ active: !config.enableChapterSummaryAuto }" type="button" @click="store.updateConfig('enableChapterSummaryAuto', false)">关</button>
            </div>
          </div>
          <div class="summary-setting-row">
            <span>详细程度</span>
            <div class="summary-switch">
              <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'short' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'short'">短</button>
              <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'normal' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'normal'">正常</button>
              <button :class="{ active: chapterSummaryConfigDraft.detailLevel === 'detailed' }" :disabled="!chapterSummaryConfig?.isAdmin" type="button" @click="chapterSummaryConfigDraft.detailLevel = 'detailed'">详细</button>
            </div>
          </div>
          <label class="summary-setting-field">
            <span>最大字数</span>
            <input v-model.number="chapterSummaryConfigDraft.maxWords" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="80" max="600">
          </label>
          <label class="summary-setting-field">
            <span>最短正文</span>
            <input v-model.number="chapterSummaryConfigDraft.minContentChars" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="0" max="5000">
          </label>
          <label class="summary-setting-field">
            <span>Temperature</span>
            <input v-model.number="chapterSummaryConfigDraft.temperature" :disabled="!chapterSummaryConfig?.isAdmin" type="number" min="0" max="1.5" step="0.1">
          </label>
          <div class="summary-actions compact">
            <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryAfterSavingSettings(false)">生成摘要</button>
            <button class="summary-action" :disabled="chapterSummaryStatus === 'loading'" @click.stop="generateChapterSummaryAfterSavingSettings(true)">重新生成</button>
            <button class="summary-action" :disabled="savingChapterSummaryConfig || !chapterSummaryConfig?.isAdmin" @click="saveChapterSummaryGenerationSettings">
              {{ savingChapterSummaryConfig ? '保存中...' : '保存生成设置' }}
            </button>
          </div>
        </div>
        <div class="summary-setting-group">
          <div class="summary-setting-title">Prompt</div>
          <textarea v-model="chapterSummaryConfigDraft.prompt" :disabled="!chapterSummaryConfig?.isAdmin" class="summary-prompt-input" rows="6"></textarea>
          <div class="summary-actions compact">
            <button class="summary-action" :disabled="!chapterSummaryConfig?.isAdmin" @click="restoreDefaultChapterSummaryPrompt">恢复当前</button>
            <button class="summary-action" :disabled="savingChapterSummaryConfig || !chapterSummaryConfig?.isAdmin" @click="saveChapterSummaryGenerationSettings">保存 Prompt</button>
          </div>
        </div>
        <div class="summary-setting-group">
          <div class="summary-setting-title">模型</div>
          <p class="summary-setting-note">摘要使用后端文本模型配置。API Key、Base URL 和模型名在 AI资料中统一管理。</p>
          <button class="summary-action" type="button" @click="openAiBackendSettings">打开 AI 后端设置</button>
        </div>
      </section>
    </aside>


    <ReaderSearchPanel
      :show="showSearch"
      :theme="chromeTheme"
      :query="searchQuery"
      :results="searchResults"
      :active-index="searchIndex"
      :count="searchCount"
      :status="bookSearchStatus"
      @close="closeSearch"
      @search="runSearch"
      @next="nextSearchResult"
      @prev="prevSearchResult"
      @update:query="searchQuery = $event"
      @jump="jumpToSearchResult"
    />

    <Transition name="fade">
      <div
        v-if="selectionMenu.visible"
        class="selection-menu"
        @click.stop
        :style="{
          top: selectionMenu.top + 'px',
          left: selectionMenu.left + 'px',
          background: chromeTheme.popup,
          color: chromeTheme.fontColor,
        }"
      >
        <div class="selection-menu-text">{{ selectionMenu.text }}</div>
        <div class="selection-menu-actions">
          <button @click="addSelectionBookmark">加入书签</button>
          <button @click="addSelectionReplaceRule('book')">按本书替换</button>
          <button @click="addSelectionReplaceRule('source')">按书源替换</button>
        </div>
      </div>
    </Transition>

    <BookDetailModal
      v-model="showBookInfo"
      :book="bookInfoBook"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted, onUnmounted, nextTick, defineAsyncComponent } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { useReaderStore, fontPresets } from '../stores/reader'
import { useAppStore } from '../stores/app'
import { getBookInfo } from '../api/bookshelf'
import {
  getChapterSummary,
  generateChapterSummary,
  getChapterSummaryConfig,
  saveChapterSummaryConfig,
} from '../api/chapterSummary'
import { applySystemTheme } from '../utils/systemUi'
import { countBrowserBookCache } from '../utils/browserCache'
import { APP_VIEWPORT_CHANGE_EVENT, syncViewportSize } from '../utils/viewport'
import { isReaderInteractiveClickTarget } from '../utils/readerClick'
import { createReaderProgressAutoSaveScheduler, createReaderProgressExitSaver } from '../utils/readerProgressAutoSave'
import { buildChapterSummaryIdentity, isCurrentChapterSummaryIdentity } from '../utils/chapterSummaryState'
import { chooseChapterSummaryPlacement, clampChapterSummarySiderWidth, getChapterSummaryFontSize } from '../utils/chapterSummaryLayout'
import type { Book, ChapterSummaryConfigResponse, ChapterSummaryRecord } from '../types'

import ReaderSidebar from '../components/reader/ReaderSidebar.vue'
import ReaderToolbar from '../components/reader/ReaderToolbar.vue'
import ReaderMobileControls from '../components/reader/ReaderMobileControls.vue'
import { useReaderSearch } from '../composables/useReaderSearch'
import { useReaderSelection } from '../composables/useReaderSelection'
import { useHorizontalPaging } from '../composables/useHorizontalPaging'
import { useContinuousReading } from '../composables/useContinuousReading'
import { useReaderAutoPlayback } from '../composables/useReaderAutoPlayback'

const ReaderCatalog = defineAsyncComponent(() => import('../components/reader/ReaderCatalog.vue'))
const ReadSettings = defineAsyncComponent(() => import('../components/reader/ReadSettings.vue'))
const ReaderBookshelf = defineAsyncComponent(() => import('../components/reader/ReaderBookshelf.vue'))
const ReaderSource = defineAsyncComponent(() => import('../components/reader/ReaderSource.vue'))
const ReplaceRuleManager = defineAsyncComponent(() => import('../components/reader/ReplaceRuleManager.vue'))
const CacheManager = defineAsyncComponent(() => import('../components/reader/CacheManager.vue'))
const BookDetailModal = defineAsyncComponent(() => import('../components/BookDetailModal.vue'))
const ReaderTtsPanel = defineAsyncComponent(() => import('../components/reader/ReaderTtsPanel.vue'))
const ReaderSearchPanel = defineAsyncComponent(() => import('../components/reader/ReaderSearchPanel.vue'))

const router = useRouter()
const store = useReaderStore()
const appStore = useAppStore()
const READER_POSITION_PREFIX = 'reader-position:'
const SERVER_PROGRESS_AUTOSAVE_MS = 10000

interface SavedReadingPosition {
  chapterIndex: number
  progress: number
  paragraphIndex?: number
  paragraphProgress?: number
  updatedAt: number
}

const CONTINUOUS_POSITION_ANCHOR_RATIO = 0.12

function debugPositionLog(message: string, payload?: unknown) {
  void message
  void payload
}

const config = computed(() => store.config)
const theme = computed(() => store.currentTheme)
const chromeTheme = computed(() => {
  if (store.isNight || appStore.theme === 'dark') {
    return {
      ...store.currentTheme,
      popup: 'var(--color-bg-elevated)',
      fontColor: 'var(--color-text)',
    }
  }
  return store.currentTheme
})

const scrollContainerRef = ref<HTMLElement>()
const chapterTextRef = ref<HTMLElement>()
const showControls = ref(false)
const isMobile = ref(false)
const viewportWidth = ref(typeof window === 'undefined' ? 0 : window.innerWidth)
let speechTimerTicker: number | null = null
let suppressNextTapUntil = 0
let restorePositionTimer: number | null = null
let persistPositionTimer: number | null = null
const pendingRestorePosition = ref<SavedReadingPosition | null>(null)
let pendingRestoreAttempts = 0
let suppressPositionSaveUntil = 0
let suppressContinuousScrollSyncUntil = 0
let suppressContinuousAutoLoadUntil = 0
const restoreStabilizeTimers: number[] = []
const serverProgressAutoSaveScheduler = createReaderProgressAutoSaveScheduler({
  intervalMs: SERVER_PROGRESS_AUTOSAVE_MS,
  flush: () => store.flushProgressToServer(),
})
const readerProgressExitSaver = createReaderProgressExitSaver({
  disposeAutoSave: () => serverProgressAutoSaveScheduler.dispose(),
  savePosition: () => saveReadingPosition({ force: true }),
  flushToServer: () => store.flushProgressToServer(true),
  flushToServerKeepalive: () => store.flushProgressToServerKeepalive(true),
})
const isContinuousMode = computed(() =>
  config.value.readMethod === '上下滚动' || config.value.readMethod === '上下滚动2',
)
const hideReadChaptersMode = computed(() => config.value.readMethod === '上下滚动2')
const isHorizontalPageMode = computed(() => config.value.readMethod === '左右翻页')
const isIosWebkit = computed(() => {
  const ua = typeof navigator !== 'undefined' ? navigator.userAgent : ''
  return /iPhone|iPad|iPod/i.test(ua) || (/Macintosh/i.test(ua) && typeof navigator !== 'undefined' && navigator.maxTouchPoints > 1)
})
const disableSystemCallout = computed(() => {
  return isIosWebkit.value && isMobile.value && config.value.selectAction === 'popup'
})
const touchState = ref({
  startX: 0,
  startY: 0,
  startAt: 0,
  moving: false,
  horizontalLocked: false,
})
const showBookInfo = ref(false)
const bookInfoBook = ref<Book | null>(null)
const showTTSPanel = ref(false)
const ttsPanelDismissed = ref(false)
const offlineCachedCount = ref(0)
const chapterSummary = ref<ChapterSummaryRecord | null>(null)
const chapterSummaryStatus = ref<'idle' | 'loading' | 'ready' | 'error'>('idle')
const chapterSummaryError = ref('')
const showChapterSummary = ref(config.value.showChapterSummary)
type ChapterSummaryTab = 'content' | 'settings'
const chapterSummaryActiveTab = ref<ChapterSummaryTab>('content')
const chapterSummaryConfig = ref<ChapterSummaryConfigResponse | null>(null)
const savingChapterSummaryConfig = ref(false)
const chapterSummaryConfigDraft = reactive({
  enabledText: 'true',
  autoEnabledDefaultText: 'true',
  detailLevel: 'normal' as 'short' | 'normal' | 'detailed',
  maxWords: 300,
  temperature: 0.3,
  minContentChars: 300,
  prompt: '',
})
const chapterSummarySiderWidth = ref(clampChapterSummarySiderWidth(config.value.chapterSummarySiderWidth))
const chapterSummarySiderResizing = ref(false)
let chapterSummaryResizeStartX = 0
let chapterSummaryResizeStartWidth = 0
let chapterSummaryTimer: number | null = null
let chapterSummaryRequestId = 0
const speechTimerNow = ref(Date.now())
const speechTimerText = computed(() => {
  if (!store.speechStopAt) return ''
  const remainMs = store.speechStopAt - speechTimerNow.value
  if (remainMs <= 0) return ''
  const totalMinutes = Math.ceil(remainMs / 60000)
  if (totalMinutes >= 60) {
    const hours = Math.floor(totalMinutes / 60)
    const minutes = totalMinutes % 60
    return minutes ? `${hours}小时${minutes}分钟后停止` : `${hours}小时后停止`
  }
  return `${totalMinutes}分钟后停止`
})
const {
  showSearch,
  searchQuery,
  searchResults,
  searchIndex,
  searchCount,
  bookSearchStatus,
  toggleSearch,
  openSearch,
  closeSearch,
  runSearch,
  nextSearchResult,
  prevSearchResult,
  jumpToSearchResult,
  handleContentUpdated,
  handlePresentationUpdated,
} = useReaderSearch(store)
const {
  selectionMenu,
  suppressSelectionCloseUntil,
  hideSelectionMenu,
  scheduleSelectionMenuUpdate,
  handleMouseUpSelection,
  handleTouchEndSelection,
  handleSelectionChange,
  addSelectionBookmark,
  addSelectionReplaceRule,
  clearSelectionState,
  disposeSelection,
} = useReaderSelection(
  store,
  appStore,
  computed(() => ({ selectAction: config.value.selectAction })),
  scrollContainerRef,
)

const offlineBannerText = computed(() => {
  if (appStore.isOnline) return ''
  if (offlineCachedCount.value > 0) {
    return `离线模式：当前书已缓存 ${offlineCachedCount.value} 章，可继续阅读已缓存章节`
  }
  return '离线模式：当前书尚未缓存到浏览器，未缓存章节将无法打开'
})

const currentChapterSummaryIdentity = computed(() => buildChapterSummaryIdentity(
  store.book?.bookUrl,
  store.currentChapter?.url,
  store.currentIndex,
))

const chapterSummaryPlacement = computed(() => chooseChapterSummaryPlacement({
  mode: config.value.chapterSummaryLayout,
  viewportWidth: viewportWidth.value,
  pageWidth: config.value.pageWidth,
  isMobile: isMobile.value,
  siderWidth: chapterSummarySiderWidth.value,
}))
const showSideChapterSummary = computed(() => showChapterSummary.value && chapterSummaryPlacement.value === 'side' && !isHorizontalPageMode.value)
const showCollapsedChapterSummary = computed(() => showChapterSummary.value && chapterSummaryPlacement.value === 'collapsed' && !isHorizontalPageMode.value)
const showInlineChapterSummary = computed(() => showChapterSummary.value && chapterSummaryPlacement.value === 'inline' && !isHorizontalPageMode.value)
const chapterSummarySiderStyle = computed(() => ({
  width: `${chapterSummarySiderWidth.value}px`,
  background: chromeTheme.value.popup,
  color: chromeTheme.value.fontColor,
}))
const chapterSummaryBodyStyle = computed(() => ({
  fontSize: `${getChapterSummaryFontSize(config.value.chapterSummaryFontSize)}px`,
  fontFamily: currentFontFamily.value || 'var(--font-body)',
}))

function clearChapterSummaryTimer() {
  if (!chapterSummaryTimer) return
  window.clearTimeout(chapterSummaryTimer)
  chapterSummaryTimer = null
}

function resetChapterSummaryState() {
  clearChapterSummaryTimer()
  chapterSummary.value = null
  chapterSummaryStatus.value = 'idle'
  chapterSummaryError.value = ''
}

function applyChapterSummaryConfigDraft(response: ChapterSummaryConfigResponse) {
  chapterSummaryConfig.value = response
  chapterSummaryConfigDraft.enabledText = response.config.enabled ? 'true' : 'false'
  chapterSummaryConfigDraft.autoEnabledDefaultText = response.config.autoEnabledDefault ? 'true' : 'false'
  chapterSummaryConfigDraft.detailLevel = response.config.detailLevel
  chapterSummaryConfigDraft.maxWords = response.config.maxWords
  chapterSummaryConfigDraft.temperature = response.config.temperature
  chapterSummaryConfigDraft.minContentChars = response.config.minContentChars
  chapterSummaryConfigDraft.prompt = response.config.prompt
}

async function loadChapterSummaryConfigForSider() {
  try {
    applyChapterSummaryConfigDraft(await getChapterSummaryConfig())
  } catch {
    chapterSummaryConfig.value = null
  }
}

function normalizeFiniteNumber(value: unknown, fallback: number) {
  if (value === '' || value === null || value === undefined) return fallback
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : fallback
}

async function saveChapterSummaryGenerationSettings(options: { silent?: boolean } | Event = {}) {
  const silent = !(options instanceof Event) && Boolean(options.silent)
  if (!chapterSummaryConfig.value?.isAdmin) {
    appStore.showToast('请输入管理密码后再保存生成设置', 'warning')
    return
  }
  savingChapterSummaryConfig.value = true
  try {
    const saved = await saveChapterSummaryConfig({
      enabled: chapterSummaryConfigDraft.enabledText === 'true',
      autoEnabledDefault: chapterSummaryConfigDraft.autoEnabledDefaultText === 'true',
      detailLevel: chapterSummaryConfigDraft.detailLevel,
      maxWords: normalizeFiniteNumber(chapterSummaryConfigDraft.maxWords, 300),
      temperature: normalizeFiniteNumber(chapterSummaryConfigDraft.temperature, 0.3),
      minContentChars: normalizeFiniteNumber(chapterSummaryConfigDraft.minContentChars, 300),
      prompt: chapterSummaryConfigDraft.prompt,
    })
    applyChapterSummaryConfigDraft(saved)
    if (!silent) appStore.showToast('摘要生成设置已保存', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '摘要生成设置保存失败', 'error')
  } finally {
    savingChapterSummaryConfig.value = false
  }
}

async function generateChapterSummaryAfterSavingSettings(force: boolean) {
  if (chapterSummaryConfig.value?.isAdmin) {
    await saveChapterSummaryGenerationSettings({ silent: true })
  }
  await generateChapterSummaryForCurrentChapter(force)
}

function restoreDefaultChapterSummaryPrompt() {
  // Task 4 UI 文案计划为“恢复当前”，这里恢复的是当前已保存到服务端的 prompt。
  const fallback = chapterSummaryConfig.value?.config.prompt || ''
  chapterSummaryConfigDraft.prompt = fallback
}

async function loadChapterSummaryForCurrentChapter() {
  const bookUrl = store.book?.bookUrl
  const chapterUrl = store.currentChapter?.url
  if (!bookUrl || !chapterUrl) {
    resetChapterSummaryState()
    return
  }

  const identity = currentChapterSummaryIdentity.value
  const requestId = ++chapterSummaryRequestId
  chapterSummaryError.value = ''
  try {
    const res = await getChapterSummary(bookUrl, chapterUrl)
    if (requestId !== chapterSummaryRequestId || !isCurrentChapterSummaryIdentity(currentChapterSummaryIdentity.value, identity)) return
    chapterSummary.value = res.summary
    chapterSummaryStatus.value = res.summary ? 'ready' : 'idle'
    if (!res.summary) scheduleAutoChapterSummary(identity)
  } catch (error) {
    if (requestId !== chapterSummaryRequestId) return
    chapterSummaryStatus.value = 'error'
    chapterSummaryError.value = (error as Error).message || '摘要加载失败'
  }
}

function scheduleAutoChapterSummary(identity: string) {
  clearChapterSummaryTimer()
  if (!showChapterSummary.value) return
  if (!config.value.enableChapterSummaryAuto) return
  if (isHorizontalPageMode.value) return
  if (!store.displayContent || store.displayContent.trim().length < 300) return
  chapterSummaryTimer = window.setTimeout(() => {
    if (!showChapterSummary.value) return
    if (!isCurrentChapterSummaryIdentity(currentChapterSummaryIdentity.value, identity)) return
    void generateChapterSummaryForCurrentChapter(false)
  }, 1500)
}

async function generateChapterSummaryForCurrentChapter(force: boolean) {
  const bookUrl = store.book?.bookUrl
  const chapter = store.currentChapter
  if (!bookUrl || !chapter?.url || !store.displayContent.trim()) return

  const identity = currentChapterSummaryIdentity.value
  const requestId = ++chapterSummaryRequestId
  clearChapterSummaryTimer()
  chapterSummaryStatus.value = 'loading'
  chapterSummaryError.value = ''
  try {
    const res = await generateChapterSummary({
      bookUrl,
      chapterUrl: chapter.url,
      chapterIndex: store.currentIndex,
      chapterTitle: chapter.title,
      content: store.displayContent,
      force,
      previousChapters: buildPreviousChapterSummaryContext(),
    })
    if (requestId !== chapterSummaryRequestId || !isCurrentChapterSummaryIdentity(currentChapterSummaryIdentity.value, identity)) return
    chapterSummary.value = res.summary
    chapterSummaryStatus.value = res.summary ? 'ready' : 'idle'
  } catch (error) {
    if (requestId !== chapterSummaryRequestId) return
    chapterSummaryStatus.value = chapterSummary.value ? 'ready' : 'error'
    chapterSummaryError.value = (error as Error).message || '摘要生成失败'
  }
}


function expandCollapsedChapterSummary() {
  chapterSummaryActiveTab.value = chapterSummary.value ? 'content' : 'settings'
  store.updateConfig('chapterSummaryLayout', 'auto')
}

function copyChapterSummary() {
  if (!chapterSummary.value) return
  const text = [
    chapterSummary.value.summary,
    chapterSummary.value.keyPoints.length ? `要点：${chapterSummary.value.keyPoints.join('；')}` : '',
  ].filter(Boolean).join('\n')
  void navigator.clipboard?.writeText(text)
  appStore.showToast('摘要已复制', 'success')
}

function buildPreviousChapterSummaryContext() {
  const end = Math.max(0, store.currentIndex)
  return store.chapters
    .slice(Math.max(0, end - 5), end)
    .map((chapter) => ({
      chapterUrl: chapter.url,
      chapterIndex: chapter.index,
      chapterTitle: chapter.title,
    }))
}

function adjustChapterSummaryFontSize(delta: number) {
  store.updateConfig('chapterSummaryFontSize', getChapterSummaryFontSize(config.value.chapterSummaryFontSize + delta))
}

function adjustChapterSummarySiderWidth(delta: number) {
  chapterSummarySiderWidth.value = clampChapterSummarySiderWidth(chapterSummarySiderWidth.value + delta)
  store.updateConfig('chapterSummarySiderWidth', chapterSummarySiderWidth.value)
}

function handleChapterSummarySiderResize(event: PointerEvent) {
  if (!chapterSummarySiderResizing.value) return
  chapterSummarySiderWidth.value = clampChapterSummarySiderWidth(chapterSummaryResizeStartWidth + chapterSummaryResizeStartX - event.clientX)
}

function stopChapterSummarySiderResize() {
  if (!chapterSummarySiderResizing.value) return
  chapterSummarySiderResizing.value = false
  window.removeEventListener('pointermove', handleChapterSummarySiderResize)
  window.removeEventListener('pointerup', stopChapterSummarySiderResize)
  store.updateConfig('chapterSummarySiderWidth', chapterSummarySiderWidth.value)
}

function startChapterSummarySiderResize(event: PointerEvent) {
  event.preventDefault()
  chapterSummarySiderResizing.value = true
  chapterSummaryResizeStartX = event.clientX
  chapterSummaryResizeStartWidth = chapterSummarySiderWidth.value
  window.addEventListener('pointermove', handleChapterSummarySiderResize)
  window.addEventListener('pointerup', stopChapterSummarySiderResize)
}

async function refreshOfflineCacheState() {
  if (!store.book) {
    offlineCachedCount.value = 0
    return
  }
  offlineCachedCount.value = await countBrowserBookCache(store.book.bookUrl).catch(() => 0)
}

let refreshOfflineCacheStateTimer: number | null = null

function scheduleRefreshOfflineCacheState() {
  if (refreshOfflineCacheStateTimer) clearTimeout(refreshOfflineCacheStateTimer)
  refreshOfflineCacheStateTimer = window.setTimeout(() => {
    void refreshOfflineCacheState()
  }, 120)
}

function checkMedia() {
  viewportWidth.value = window.innerWidth
  isMobile.value = window.innerWidth <= 768
  window.setTimeout(() => {
    updateHorizontalMetrics()
    if (isHorizontalPageMode.value) {
      rebuildHorizontalPages()
    }
  }, 0)
}

function handleViewportChange() {
  syncViewportSize()
  checkMedia()
  scheduleRestoreReadingPosition()
}

const currentFontFamily = computed(() => {
  const preset = fontPresets.find(p => p.value === config.value.fontFamily)
  return preset ? preset.family : ''
})

function formatChapterHtml(rawText: string) {
  if (!rawText) return ''
  let text = rawText

  if (showSearch.value && searchQuery.value) {
    try {
      const regex = new RegExp(`(${searchQuery.value})`, 'gi')
      text = text.replace(regex, '<mark class="search-highlight">$1</mark>')
    } catch { /* invalid regex */ }
  }

  const stripLeadingIndent = (line: string) => line.replace(/^[\u3000\u00A0 \t]+/, '')

  if (/<[a-z][\s\S]*>/i.test(text)) {
    const wrapper = document.createElement('div')
    wrapper.innerHTML = text
    const paragraphs = Array.from(wrapper.querySelectorAll('p')) as HTMLParagraphElement[]
    if (paragraphs.length) {
      paragraphs.forEach((paragraph) => {
        const plainText = (paragraph.textContent || '').replace(/^[\u3000\u00A0 \t]+/, '').trim()
        if (!plainText) {
          paragraph.remove()
          return
        }
        paragraph.innerHTML = paragraph.innerHTML.replace(/^[\u3000\u00A0 \t]+/, '')
        paragraph.style.marginTop = '0'
        paragraph.style.marginBottom = `${config.value.paragraphSpacing}em`
        paragraph.classList.toggle('reader-indent', config.value.firstLineIndent)
      })
      return wrapper.innerHTML
    }
  }

  return text
    .split(/\n/)
    .filter((line: string) => line.trim())
    .map((line: string) => {
      const shouldIndent = config.value.firstLineIndent
      const content = stripLeadingIndent(line.trimEnd())
      return `<p${shouldIndent ? ' class="reader-indent"' : ''} style="margin-top: 0; margin-bottom: ${config.value.paragraphSpacing}em;">${content}</p>`
    })
    .join('')
}

function renderChapterHtml(rawText: string) {
  return formatChapterHtml(store.processContentForDisplay(rawText || ''))
}

const formattedContent = computed(() => formatChapterHtml(store.displayContent || ''))

const {
  horizontalPageIndex,
  horizontalPageStep,
  horizontalPageStepStyle,
  horizontalPages,
  isHorizontalAtEnd,
  rebuildHorizontalPages,
  updateHorizontalMetrics,
  updateHorizontalEndState,
  alignHorizontalToNearestPage,
  resetHorizontalPagePosition,
} = useHorizontalPaging(
  store,
  computed(() => ({
    fontSize: config.value.fontSize,
    fontWeight: config.value.fontWeight,
    lineHeight: config.value.lineHeight,
  })),
  currentFontFamily,
  formattedContent,
  isHorizontalPageMode,
  scrollContainerRef,
)

const horizontalPageTransform = computed(() => {
  const offset = horizontalPageIndex.value * Math.max(1, horizontalPageStep.value)
  return `translate3d(${-offset}px, 0, 0)`
})
const horizontalPageTransitionDuration = computed(() => {
  const duration = Number(config.value.animateDuration) || 0
  if (duration <= 0) return '0ms'
  return `${Math.min(220, duration)}ms`
})
const {
  continuousChapters,
  continuousLoadingNext,
  continuousLoadingPrev,
  suppressContinuousSync,
  syncContinuousChapterHtml,
  getContinuousChapter,
  setContinuousActiveChapter,
  initializeContinuousChapters,
  syncContinuousToStoreState,
  loadContinuousNext,
  getContinuousSections,
  pruneReadChapters,
  clearContinuousChapters,
  disposeContinuousReading,
} = useContinuousReading(
  store,
  renderChapterHtml,
  isContinuousMode,
  hideReadChaptersMode,
  scrollContainerRef,
)

function syncHorizontalPageState() {
  const maxPage = Math.max(0, horizontalPages.value.length - 1)
  const progress = maxPage <= 0 ? 1 : horizontalPageIndex.value / maxPage
  store.setChapterScrollProgress(progress)
  updateHorizontalEndState()
  if (config.value.enablePreload && maxPage > 0 && horizontalPageIndex.value >= maxPage - 1) {
    store.preloadAroundChapter(store.currentIndex)
  }
  scheduleSaveReadingPosition()
  serverProgressAutoSaveScheduler.schedule()
}

function pageForward() {
  const container = scrollContainerRef.value
  if (!container) return
  if (isHorizontalPageMode.value) {
    const maxPage = Math.max(0, horizontalPages.value.length - 1)
    if (horizontalPageIndex.value >= maxPage) {
      nextChapter()
      return
    }
    horizontalPageIndex.value = Math.min(maxPage, horizontalPageIndex.value + 1)
    container.scrollTo({ left: 0, behavior: 'auto' })
    syncHorizontalPageState()
    return
  }
  const step = container.clientHeight * 0.88
  if (container.scrollTop + container.clientHeight >= container.scrollHeight - 10) {
    nextChapter()
    return
  }
  container.scrollBy({ top: step, behavior: 'smooth' })
}

function pageBackward() {
  const container = scrollContainerRef.value
  if (!container) return
  if (isHorizontalPageMode.value) {
    if (horizontalPageIndex.value <= 0) {
      prevChapter()
      return
    }
    horizontalPageIndex.value = Math.max(0, horizontalPageIndex.value - 1)
    container.scrollTo({ left: 0, behavior: 'auto' })
    syncHorizontalPageState()
    return
  }
  const step = container.clientHeight * 0.88
  if (container.scrollTop <= 10) {
    prevChapter()
    return
  }
  container.scrollBy({ top: -step, behavior: 'smooth' })
}

// Navigation
async function goHome() {
  await persistReadingProgressBeforeLeave()
  router.replace('/')
}

function handlePageHide() {
  persistReadingProgressKeepalive()
}

function handleBeforeUnload() {
  persistReadingProgressKeepalive()
}

function handleVisibilityChange() {
  if (document.visibilityState !== 'hidden') return
  persistReadingProgressTemporaryKeepalive()
}

async function persistReadingProgressBeforeLeave() {
  await readerProgressExitSaver.flushBeforeRouteLeave()
}

function persistReadingProgressKeepalive() {
  readerProgressExitSaver.flushKeepalive()
}

function persistReadingProgressTemporaryKeepalive() {
  readerProgressExitSaver.flushTemporaryKeepalive()
}

async function prevChapter() {
  const targetIndex = store.currentIndex - 1
  if (targetIndex < 0) return

  if (!isContinuousMode.value) {
    await store.prevChapter()
    scrollToTop()
    return
  }

  await rebuildContinuousAtChapter(targetIndex)
}

async function nextChapter() {
  const targetIndex = store.currentIndex + 1
  if (targetIndex >= store.chapters.length) return

  if (!isContinuousMode.value) {
    await store.nextChapter()
    scrollToTop()
    return
  }

  await rebuildContinuousAtChapter(targetIndex)
}

async function jumpFromCatalog(targetIndex: number) {
  if (targetIndex < 0 || targetIndex >= store.chapters.length) return

  if (!isContinuousMode.value) {
    await store.loadChapter(targetIndex)
    store.closePanel()
    scrollToTop()
    return
  }

  await rebuildContinuousAtChapter(targetIndex)
  store.closePanel()
}

async function rebuildContinuousAtChapter(targetIndex: number) {
  suppressContinuousScrollSyncUntil = Date.now() + 500
  suppressContinuousAutoLoadUntil = Date.now() + 500
  await initializeContinuousChapters(targetIndex, false)
}

function scrollToTop() {
  if (scrollContainerRef.value) {
    if (isHorizontalPageMode.value) {
      scrollContainerRef.value.scrollTo({ left: 0, behavior: 'smooth' })
    } else {
      scrollContainerRef.value.scrollTo({ top: 0, behavior: 'smooth' })
    }
  }
}

function scrollToBottom() {
  if (scrollContainerRef.value) {
    scrollContainerRef.value.scrollTo({ top: scrollContainerRef.value.scrollHeight, behavior: 'smooth' })
  }
}

function getPositionStorageKey() {
  return store.book?.bookUrl ? `${READER_POSITION_PREFIX}${store.book.bookUrl}` : ''
}

function normalizePositionTimestamp(value?: number | null) {
  if (typeof value !== 'number' || Number.isNaN(value) || value <= 0) return 0
  return value < 1_000_000_000_000 ? value * 1000 : value
}

function buildServerSavedPosition(): SavedReadingPosition | null {
  if (!store.book) return null
  if (store.book.durChapterIndex !== store.currentIndex) return null
  const rawPos = typeof store.book.durChapterPos === 'number' ? store.book.durChapterPos : 0
  const progress = rawPos > 1 ? rawPos / 10000 : rawPos
  return {
    chapterIndex: store.currentIndex,
    progress: Math.max(0, Math.min(1, progress || 0)),
    updatedAt: normalizePositionTimestamp(store.book.durChapterTime),
  }
}

function loadSavedReadingPosition() {
  const key = getPositionStorageKey()
  if (!key) {
    pendingRestorePosition.value = null
    pendingRestoreAttempts = 0
    debugPositionLog('skip load: no storage key')
    return
  }
  try {
    const raw = localStorage.getItem(key)
    const localSaved = raw ? JSON.parse(raw) as SavedReadingPosition : null
    const serverSaved = buildServerSavedPosition()

    let selected: SavedReadingPosition | null = null
    let source: 'local' | 'server' | 'none' = 'none'

    if (localSaved && localSaved.chapterIndex === store.currentIndex) {
      selected = localSaved
      source = 'local'
    }

    if (serverSaved && serverSaved.chapterIndex === store.currentIndex) {
      if (!selected || normalizePositionTimestamp(serverSaved.updatedAt) > normalizePositionTimestamp(selected.updatedAt)) {
        selected = serverSaved
        source = 'server'
      }
    }

    if (!selected) {
      pendingRestorePosition.value = null
      pendingRestoreAttempts = 0
      clearRestoreStabilizers()
      debugPositionLog(raw ? 'ignored saved position' : 'no saved position', {
        key,
        currentIndex: store.currentIndex,
        localSaved,
        serverSaved,
      })
      return
    }

    pendingRestorePosition.value = selected
    pendingRestoreAttempts = 0
    clearRestoreStabilizers()
    debugPositionLog('loaded saved position', {
      key,
      saved: selected,
      source,
      localSaved,
      serverSaved,
      currentIndex: store.currentIndex,
      accepted: !!pendingRestorePosition.value,
    })
    if (pendingRestorePosition.value) {
      suppressPositionSaveUntil = Date.now() + 2500
    }
  } catch {
    pendingRestorePosition.value = null
    pendingRestoreAttempts = 0
    clearRestoreStabilizers()
    debugPositionLog('failed to parse saved position', { key })
  }
}

function saveReadingPosition(options: { force?: boolean } = {}) {
  const key = getPositionStorageKey()
  const container = scrollContainerRef.value
  const suppressed = !options.force && Date.now() < suppressPositionSaveUntil
  if (!key || !container || store.loading || !store.book || suppressed) {
    debugPositionLog('skip save', {
      key,
      hasContainer: !!container,
      loading: store.loading,
      hasBook: !!store.book,
      suppressed,
      currentIndex: store.currentIndex,
    })
    return
  }

  const basePosition: SavedReadingPosition = {
    chapterIndex: store.currentIndex,
    progress: store.chapterScrollProgress,
    updatedAt: Date.now(),
  }

  const anchorRatio = isContinuousMode.value ? CONTINUOUS_POSITION_ANCHOR_RATIO : 0.3
  const anchorViewportY = container.getBoundingClientRect().top + container.clientHeight * anchorRatio
  if (isContinuousMode.value && continuousChapters.value.length) {
    const section = container.querySelector(`.continuous-chapter[data-chapter-index="${store.currentIndex}"]`) as HTMLElement | null
    const paragraphs = Array.from(section?.querySelectorAll('.chapter-text p') || []) as HTMLElement[]
    if (paragraphs.length) {
      let activeParagraph = paragraphs[0]
      let paragraphIndex = 0
      paragraphs.forEach((paragraph, index) => {
        if (paragraph.getBoundingClientRect().top <= anchorViewportY) {
          activeParagraph = paragraph
          paragraphIndex = index
        }
      })
      const rect = activeParagraph.getBoundingClientRect()
      const paragraphProgress = rect.height > 0 ? Math.max(0, Math.min(1, (anchorViewportY - rect.top) / rect.height)) : 0
      basePosition.paragraphIndex = paragraphIndex
      basePosition.paragraphProgress = paragraphProgress
    }
  } else if (!isHorizontalPageMode.value) {
    const paragraphs = Array.from(chapterTextRef.value?.querySelectorAll('p') || []) as HTMLElement[]
    if (paragraphs.length) {
      let activeParagraph = paragraphs[0]
      let paragraphIndex = 0
      paragraphs.forEach((paragraph, index) => {
        if (paragraph.getBoundingClientRect().top <= anchorViewportY) {
          activeParagraph = paragraph
          paragraphIndex = index
        }
      })
      const rect = activeParagraph.getBoundingClientRect()
      const paragraphProgress = rect.height > 0 ? Math.max(0, Math.min(1, (anchorViewportY - rect.top) / rect.height)) : 0
      basePosition.paragraphIndex = paragraphIndex
      basePosition.paragraphProgress = paragraphProgress
    }
  }

  localStorage.setItem(key, JSON.stringify(basePosition))
  debugPositionLog('saved position', { key, position: basePosition })
}

function scheduleSaveReadingPosition() {
  if (persistPositionTimer) clearTimeout(persistPositionTimer)
  persistPositionTimer = window.setTimeout(() => {
    saveReadingPosition()
  }, 120)
}

function restoreReadingPosition() {
  return restoreReadingPositionInternal(pendingRestorePosition.value, true)
}

function clearRestoreStabilizers() {
  while (restoreStabilizeTimers.length) {
    const timer = restoreStabilizeTimers.pop()
    if (typeof timer === 'number') clearTimeout(timer)
  }
}

function scheduleRestoreStabilization(saved: SavedReadingPosition) {
  clearRestoreStabilizers()
  if (!isIosWebkit.value || isHorizontalPageMode.value) return
  ;[140, 320, 680].forEach((delay) => {
    const timer = window.setTimeout(() => {
      if (store.loading || !scrollContainerRef.value || saved.chapterIndex !== store.currentIndex) return
      void nextTick(() => {
        restoreReadingPositionInternal(saved, false)
      })
    }, delay)
    restoreStabilizeTimers.push(timer)
  })
}

function restoreReadingPositionInternal(saved: SavedReadingPosition | null, finalize: boolean) {
  const container = scrollContainerRef.value
  if (!saved || !container || saved.chapterIndex !== store.currentIndex) {
    debugPositionLog('restore aborted', {
      hasSaved: !!saved,
      hasContainer: !!container,
      savedChapterIndex: saved?.chapterIndex,
      currentIndex: store.currentIndex,
    })
    return false
  }

  if (isHorizontalPageMode.value) {
    if (store.loading || container.scrollWidth <= container.clientWidth + 4) {
      debugPositionLog('restore waiting: horizontal content not ready', {
        saved,
        loading: store.loading,
        scrollWidth: container.scrollWidth,
        clientWidth: container.clientWidth,
      })
      return false
    }
    const maxScroll = Math.max(0, container.scrollWidth - container.clientWidth)
    container.scrollTo({ left: maxScroll * Math.max(0, Math.min(1, saved.progress || 0)), behavior: 'auto' })
    if (finalize) {
      pendingRestorePosition.value = null
      pendingRestoreAttempts = 0
    }
    debugPositionLog('restored horizontal position', { saved, maxScroll })
    return true
  }

  const anchorOffset = container.clientHeight * (isContinuousMode.value ? CONTINUOUS_POSITION_ANCHOR_RATIO : 0.3)
  let targetTop = 0

  if (isContinuousMode.value) {
    if (store.loading || !continuousChapters.value.length) {
      debugPositionLog('restore waiting: continuous content not ready', {
        saved,
        loading: store.loading,
        continuousCount: continuousChapters.value.length,
      })
      return false
    }
    const section = container.querySelector(`.continuous-chapter[data-chapter-index="${saved.chapterIndex}"]`) as HTMLElement | null
    if (!section) {
      debugPositionLog('restore failed: section not found', {
        saved,
        availableSections: Array.from(container.querySelectorAll('.continuous-chapter')).map((el) => (el as HTMLElement).dataset.chapterIndex),
      })
      return false
    }
    const paragraphs = Array.from(section.querySelectorAll('.chapter-text p')) as HTMLElement[]
    if (typeof saved.paragraphIndex === 'number' && !paragraphs.length) {
      debugPositionLog('restore waiting: continuous paragraphs not ready', {
        saved,
        sectionIndex: saved.chapterIndex,
      })
      return false
    }
    if (paragraphs.length && typeof saved.paragraphIndex === 'number') {
      const paragraph = paragraphs[Math.max(0, Math.min(paragraphs.length - 1, saved.paragraphIndex))]
      const top = paragraph.getBoundingClientRect().top - container.getBoundingClientRect().top + container.scrollTop
      const paragraphProgress = Math.max(0, Math.min(1, saved.paragraphProgress || 0))
      targetTop = Math.max(section.offsetTop, top + paragraph.offsetHeight * paragraphProgress - anchorOffset)
    } else {
      const nextSection = section.nextElementSibling as HTMLElement | null
      const sectionHeight = Math.max(1, (nextSection ? nextSection.offsetTop : container.scrollHeight) - section.offsetTop)
      if ((saved.progress || 0) > 0 && sectionHeight <= Math.max(1, container.clientHeight * 0.25)) {
        debugPositionLog('restore waiting: continuous section height not ready', {
          saved,
          sectionHeight,
          clientHeight: container.clientHeight,
        })
        return false
      }
      targetTop = Math.max(
        section.offsetTop,
        section.offsetTop + sectionHeight * Math.max(0, Math.min(1, saved.progress || 0)),
      )
    }
  } else {
    const paragraphs = Array.from(chapterTextRef.value?.querySelectorAll('p') || []) as HTMLElement[]
    if (store.loading || !chapterTextRef.value) {
      debugPositionLog('restore waiting: chapter content not ready', {
        saved,
        loading: store.loading,
        hasChapterText: !!chapterTextRef.value,
      })
      return false
    }
    if (typeof saved.paragraphIndex === 'number' && !paragraphs.length) {
      debugPositionLog('restore waiting: chapter paragraphs not ready', {
        saved,
      })
      return false
    }
    if (paragraphs.length && typeof saved.paragraphIndex === 'number') {
      const paragraph = paragraphs[Math.max(0, Math.min(paragraphs.length - 1, saved.paragraphIndex))]
      const top = paragraph.getBoundingClientRect().top - container.getBoundingClientRect().top + container.scrollTop
      const paragraphProgress = Math.max(0, Math.min(1, saved.paragraphProgress || 0))
      targetTop = top + paragraph.offsetHeight * paragraphProgress - anchorOffset
    } else {
      const maxScroll = Math.max(0, container.scrollHeight - container.clientHeight)
      if ((saved.progress || 0) > 0 && maxScroll <= 4) {
        debugPositionLog('restore waiting: max scroll not ready', {
          saved,
          scrollHeight: container.scrollHeight,
          clientHeight: container.clientHeight,
          maxScroll,
        })
        return false
      }
      targetTop = maxScroll * Math.max(0, Math.min(1, saved.progress || 0))
    }
  }

  container.scrollTo({ top: Math.max(0, targetTop), behavior: 'auto' })
  if (finalize) {
    pendingRestorePosition.value = null
    pendingRestoreAttempts = 0
    const suppressMs = isContinuousMode.value && isIosWebkit.value ? 1600 : 500
    suppressContinuousScrollSyncUntil = Date.now() + suppressMs
    suppressContinuousAutoLoadUntil = Date.now() + suppressMs
    scheduleRestoreStabilization(saved)
  }
  suppressPositionSaveUntil = Date.now() + 400
  debugPositionLog('restored vertical position', {
    saved,
    targetTop,
    isContinuous: isContinuousMode.value,
    finalize,
  })
  return true
}

function scheduleRestoreReadingPosition() {
  if (restorePositionTimer) clearTimeout(restorePositionTimer)
  debugPositionLog('schedule restore', {
    attempts: pendingRestoreAttempts,
    hasPending: !!pendingRestorePosition.value,
    currentIndex: store.currentIndex,
  })
  restorePositionTimer = window.setTimeout(() => {
    void nextTick(() => {
      const restored = restoreReadingPosition()
      if (!restored && pendingRestorePosition.value && pendingRestoreAttempts < 12) {
        pendingRestoreAttempts += 1
        debugPositionLog('restore retry', {
          attempts: pendingRestoreAttempts,
          pending: pendingRestorePosition.value,
          currentIndex: store.currentIndex,
        })
        scheduleRestoreReadingPosition()
      } else if (!restored) {
        debugPositionLog('restore gave up', {
          attempts: pendingRestoreAttempts,
          pending: pendingRestorePosition.value,
          currentIndex: store.currentIndex,
        })
        pendingRestorePosition.value = null
        pendingRestoreAttempts = 0
      }
    })
  }, pendingRestoreAttempts === 0 ? 0 : 80)
}

const {
  clearReadingClass,
  startAutoScroll,
  stopAutoScroll,
  startSpeech,
  speechPrev,
  speechNext,
  restartSpeechFromCurrentParagraph,
  cancelSpeechTransition,
  resetAutoParagraphIndex,
  handleContentChanged,
  disposeAutoPlayback,
} = useReaderAutoPlayback(
  store,
  computed(() => ({
    autoPageMode: config.value.autoPageMode,
    clickAction: config.value.clickAction,
    scrollPixel: config.value.scrollPixel,
    pageSpeed: config.value.pageSpeed,
    fontSize: config.value.fontSize,
    lineHeight: config.value.lineHeight,
  })),
  isContinuousMode,
  scrollContainerRef,
  chapterTextRef,
  nextChapter,
  prevChapter,
)

// Click behavior
function handleBackgroundClick(e: Event) {
  // If clicked directly on the reader-view wrapper, toggle controls
  if ((e.target as HTMLElement).classList.contains('reader-view')) {
    showControls.value = false
  }
}

function handleContextMenu(event: Event) {
  if (!disableSystemCallout.value) return
  event.preventDefault()
}

function handleGlobalClick(e: MouseEvent) {
  if (store.activePanel) return
  if (Date.now() < suppressNextTapUntil) return
  if (Date.now() < suppressSelectionCloseUntil.value) return
  if (selectionMenu.value.visible) {
    hideSelectionMenu()
    return
  }
  if (window.getSelection?.()?.toString().trim()) return

  const target = e.target as HTMLElement | null
  if (isReaderInteractiveClickTarget(target)) return
  if (showControls.value && !store.activePanel) {
    showControls.value = false
    return
  }
  if (store.isAutoScrolling) return
  
  if (isHorizontalPageMode.value && isMobile.value) {
    const x = e.clientX / window.innerWidth
    if (x < 0.3) {
      clickZoneAction('prev')
    } else if (x > 0.7) {
      clickZoneAction('next')
    } else {
      clickZoneAction('menu')
    }
  } else {
    const y = e.clientY / window.innerHeight
    if (y < 0.3) {
      clickZoneAction('prev')
    } else if (y > 0.7) {
      clickZoneAction('next')
    } else {
      clickZoneAction('menu')
    }
  }
}

function clickZoneAction(zone: 'prev' | 'menu' | 'next') {
  if (store.isAutoScrolling) return

  if (zone === 'menu') {
    if (isMobile.value) {
      showControls.value = !showControls.value
    }
    return
  }
  
  if (config.value.clickAction === 'none') return
  
  const container = scrollContainerRef.value
  if (!container) return
  
  if (isHorizontalPageMode.value) {
    if (zone === 'next') pageForward()
    else pageBackward()
    return
  }

  const h = container.clientHeight
  const delta = h * 0.8 // Page scroll amount

  if (config.value.clickAction === 'next') {
    pageForward()
    return
  }
  
  if (zone === 'next') {
    if (container.scrollTop + h >= container.scrollHeight - 10) {
      if (config.value.clickAction === 'auto') nextChapter()
    } else {
      container.scrollBy({ top: delta, behavior: 'smooth' })
    }
  } else {
    if (container.scrollTop === 0) {
      if (config.value.clickAction === 'auto') prevChapter()
    } else {
      container.scrollBy({ top: -delta, behavior: 'smooth' })
    }
  }
}

function handleScroll() {
  hideSelectionMenu()
  const container = scrollContainerRef.value
  if (container && isContinuousMode.value && continuousChapters.value.length) {
    if (Date.now() < suppressContinuousScrollSyncUntil) {
      scheduleSaveReadingPosition()
      return
    }
    const sections = getContinuousSections()
    if (sections.length) {
      const anchorLine = container.scrollTop + container.clientHeight * CONTINUOUS_POSITION_ANCHOR_RATIO
      let activeSection = sections[0]
      for (const section of sections) {
        if (section.offsetTop <= anchorLine) {
          activeSection = section
        } else {
          break
        }
      }

      const activeIndex = Number(activeSection.dataset.chapterIndex || 0)
      const activeChapter = getContinuousChapter(activeIndex)
      const nextSection = sections[sections.indexOf(activeSection) + 1] || null
      const sectionRange = Math.max(
        1,
        (nextSection ? nextSection.offsetTop : container.scrollHeight) - activeSection.offsetTop,
      )
      const progress = Math.max(0, Math.min(1, (container.scrollTop - activeSection.offsetTop) / sectionRange))
      if (activeChapter) {
        if (store.currentIndex !== activeIndex || store.content !== activeChapter.content) {
          setContinuousActiveChapter(activeIndex, activeChapter.content, progress)
        } else {
          store.setChapterScrollProgress(progress)
        }
      }
    }

    if (Date.now() >= suppressContinuousAutoLoadUntil && container.scrollHeight - (container.scrollTop + container.clientHeight) < 480) {
      loadContinuousNext()
    }
  } else if (container) {
    const maxScroll = Math.max(1, container.scrollHeight - container.clientHeight)
    const progress = isHorizontalPageMode.value
      ? (() => {
          const maxPage = Math.max(0, horizontalPages.value.length - 1)
          return maxPage <= 0 ? 1 : horizontalPageIndex.value / maxPage
        })()
      : (container.scrollHeight <= container.clientHeight ? 1 : container.scrollTop / maxScroll)
    store.setChapterScrollProgress(progress)
    if (isHorizontalPageMode.value) {
      updateHorizontalMetrics()
      const maxPage = Math.max(0, horizontalPages.value.length - 1)
      horizontalPageIndex.value = Math.max(0, Math.min(maxPage, horizontalPageIndex.value))
      if (container.scrollLeft !== 0) {
        container.scrollTo({ left: 0, behavior: 'auto' })
      }
      updateHorizontalEndState()
      if (config.value.enablePreload && maxPage > 0 && horizontalPageIndex.value >= maxPage - 1) {
        store.preloadAroundChapter(store.currentIndex)
      }
    } else if (config.value.enablePreload && container.scrollHeight - (container.scrollTop + container.clientHeight) < container.clientHeight * 1.5) {
      store.preloadAroundChapter(store.currentIndex)
    }
  }
  if (showControls.value && !store.activePanel) {
    showControls.value = false
  }
  scheduleSaveReadingPosition()
  serverProgressAutoSaveScheduler.schedule()
}

function handleTouchStart(event: TouchEvent) {
  stopAutoScroll()
  hideSelectionMenu()
  const touch = event.touches[0]
  if (!touch) return
  touchState.value = {
    startX: touch.clientX,
    startY: touch.clientY,
    startAt: Date.now(),
    moving: true,
    horizontalLocked: false,
  }
}

function handleTouchMove(event: TouchEvent) {
  if (!isMobile.value || config.value.readMethod !== '左右翻页' || !touchState.value.moving) return
  const selectedText = window.getSelection?.()?.toString().trim()
  if (selectedText) return
  // Keep long-press text selection gestures available on mobile.
  if (Date.now() - touchState.value.startAt > 220) return
  const touch = event.touches[0]
  if (!touch) return
  const deltaX = touch.clientX - touchState.value.startX
  const deltaY = touch.clientY - touchState.value.startY
  if (Math.abs(deltaX) > 12 && Math.abs(deltaX) > Math.abs(deltaY)) {
    touchState.value.horizontalLocked = true
    event.preventDefault()
  }
}

function handleTouchEnd(event: TouchEvent) {
  if (!isMobile.value || config.value.readMethod !== '左右翻页' || !touchState.value.moving) {
    touchState.value.moving = false
    return
  }
  const target = event.target as HTMLElement | null
  if (isReaderInteractiveClickTarget(target)) {
    touchState.value.moving = false
    return
  }
  const touchDuration = Date.now() - touchState.value.startAt
  const selectedText = window.getSelection?.()?.toString().trim()
  if (selectedText) {
    suppressNextTapUntil = Date.now() + 900
    touchState.value.moving = false
    scheduleSelectionMenuUpdate(260)
    return
  }
  const touch = event.changedTouches[0]
  if (!touch) {
    touchState.value.moving = false
    return
  }
  const deltaX = touch.clientX - touchState.value.startX
  const deltaY = touch.clientY - touchState.value.startY
  let didPageTurn = false
  if (Math.abs(deltaX) > 18 && Math.abs(deltaX) > Math.abs(deltaY)) {
    suppressNextTapUntil = Date.now() + 350
    if (deltaX < 0) {
      pageForward()
    } else {
      pageBackward()
    }
    didPageTurn = true
  }
  touchState.value.moving = false
  if (!didPageTurn && touchDuration > 260) {
    // Long-press should be reserved for native text selection, not page action.
    suppressNextTapUntil = Date.now() + 900
    scheduleSelectionMenuUpdate(260)
    return
  }
  if (!didPageTurn) {
    const moved = Math.hypot(deltaX, deltaY)
    if (touchDuration <= 260 && moved < 10) {
      suppressNextTapUntil = Date.now() + 350
      if (showControls.value && !store.activePanel) {
        showControls.value = false
      } else {
        const x = touch.clientX / window.innerWidth
        if (x < 0.3) {
          clickZoneAction('prev')
        } else if (x > 0.7) {
          clickZoneAction('next')
        } else {
          clickZoneAction('menu')
        }
      }
    } else {
      window.setTimeout(() => {
        alignHorizontalToNearestPage(touchState.value.moving)
      }, 120)
    }
  }
  scheduleSelectionMenuUpdate(260)
}

function openCachePanel() {
  store.togglePanel('cache')
}

// Keyboard shortcuts
function handleKeydown(e: KeyboardEvent) {
  const activeElement = document.activeElement as HTMLElement | null
  const tagName = activeElement?.tagName?.toLowerCase()
  if (tagName === 'input' || tagName === 'textarea' || tagName === 'select' || activeElement?.isContentEditable) {
    return
  }

  // Handle Escape key first - close panels or go home
  if (e.key === 'Escape') {
    if (store.activePanel) {
      store.closePanel()
      return
    }
    if (selectionMenu.value.visible) {
      hideSelectionMenu()
      return
    }
    if (showSearch.value) {
      closeSearch()
      return
    }
    if (showTTSPanel.value) {
      closeTTSPanel()
      return
    }
    if (showBookInfo.value) {
      showBookInfo.value = false
      return
    }
    if (showControls.value) {
      showControls.value = false
      return
    }
    // If nothing is open, go home
    goHome()
    return
  }

  // Don't process other keys when panels are open
  if (store.activePanel) return

  const container = scrollContainerRef.value
  if (!container) return

  const h = container.clientHeight

  switch (e.key) {
    case ' ':
    case 'Space':
      e.preventDefault()
      pageForward()
      break
    case 'ArrowDown':
    case 'PageDown':
      e.preventDefault()
      if (isHorizontalPageMode.value) {
        pageForward()
      } else {
        container.scrollBy({ top: h * 0.8, behavior: 'smooth' })
      }
      break
    case 'ArrowUp':
    case 'PageUp':
      e.preventDefault()
      if (isHorizontalPageMode.value) {
        pageBackward()
      } else {
        container.scrollBy({ top: -(h * 0.8), behavior: 'smooth' })
      }
      break
    case 'ArrowRight':
      e.preventDefault()
      nextChapter()
      break
    case 'ArrowLeft':
      e.preventDefault()
      prevChapter()
      break
    case 'Home':
      e.preventDefault()
      scrollToTop()
      break
    case 'End':
      e.preventDefault()
      scrollToBottom()
      break
  }
}

// Toolbar actions
async function toggleBookmark() {
  store.togglePanel('bookmark')
}

function handleTTS() {
  ttsPanelDismissed.value = false
  showTTSPanel.value = true
}

function closeTTSPanel() {
  showTTSPanel.value = false
  ttsPanelDismissed.value = true
}

function toggleSpeechFromPanel() {
  ttsPanelDismissed.value = false
  showTTSPanel.value = true
  if (!store.isSpeaking) {
    startSpeech()
    return
  }
  cancelSpeechTransition()
  store.pauseTTS()
}

function handleStopTTS() {
  cancelSpeechTransition()
  store.stopTTS()
}

watch(() => store.isAutoScrolling, (val) => {
  store.autoReading = val
  if (val) startAutoScroll()
  else stopAutoScroll()
})

watch(showTTSPanel, (visible) => {
  if (!visible) return
})

function changeVoice(name: string) {
  store.setVoiceName(name)
  ttsPanelDismissed.value = false
  showTTSPanel.value = true
  if (store.isSpeaking && !store.isPaused) {
    restartSpeechFromCurrentParagraph()
  }
}

function changeOpenAIVoice(voiceId: string) {
  if (store.speechConfig.openaiSource === 'server') return
  store.setOpenAISpeechVoice(voiceId)
  ttsPanelDismissed.value = false
  showTTSPanel.value = true
  if (store.isSpeaking && !store.isPaused) {
    restartSpeechFromCurrentParagraph()
  }
}

function adjustSpeechRate(delta: number) {
  const next = Math.max(0.5, Math.min(3, parseFloat((store.speechConfig.speechRate + delta).toFixed(1))))
  store.setSpeechRate(next)
  ttsPanelDismissed.value = false
  showTTSPanel.value = true
  if (store.isSpeaking && !store.isPaused) {
    restartSpeechFromCurrentParagraph()
  }
}

function adjustSpeechPitch(delta: number) {
  const next = Math.max(0.5, Math.min(2, parseFloat((store.speechConfig.speechPitch + delta).toFixed(1))))
  store.setSpeechPitch(next)
  ttsPanelDismissed.value = false
  showTTSPanel.value = true
  if (store.isSpeaking && !store.isPaused) {
    restartSpeechFromCurrentParagraph()
  }
}

function setSpeechTimer(minutes: number) {
  store.setSpeechStopTimer(minutes)
  ttsPanelDismissed.value = false
  showTTSPanel.value = true
}
async function openInfo() {
  if (!store.book) return
  showBookInfo.value = true
  bookInfoBook.value = {
    ...store.book,
    durChapterIndex: store.currentIndex,
    durChapterTitle: store.currentChapter?.title || store.book.durChapterTitle,
  }
  try {
    const latest = await getBookInfo(store.book.bookUrl, store.book.origin)
    bookInfoBook.value = {
      ...store.book,
      ...latest,
      durChapterIndex: store.currentIndex,
      durChapterTitle: store.currentChapter?.title || latest.durChapterTitle || store.book.durChapterTitle,
    }
  } catch {
    appStore.showToast('获取书籍详情失败，已显示当前缓存信息', 'warning')
  }
}

function toggleChapterSummary() {
  showChapterSummary.value = !showChapterSummary.value
  store.updateConfig('showChapterSummary', showChapterSummary.value)
  if (showChapterSummary.value && !chapterSummary.value && chapterSummaryStatus.value !== 'loading') {
    scheduleAutoChapterSummary(currentChapterSummaryIdentity.value)
  } else if (!showChapterSummary.value) {
    clearChapterSummaryTimer()
  }
  appStore.showToast(showChapterSummary.value ? '已显示摘要' : '已隐藏摘要', 'success')
}

function hideChapterSummary() {
  showChapterSummary.value = false
  store.updateConfig('showChapterSummary', false)
  clearChapterSummaryTimer()
}

function openAiBook() {
  if (!store.book) return
  router.push({
    name: 'ai-book',
    query: { bookUrl: store.book.bookUrl },
  })
}

function openAiBackendSettings() {
  const bookUrl = store.book?.bookUrl
  const query = bookUrl
    ? { bookUrl, tab: 'settings', section: 'server-model' }
    : { tab: 'settings', section: 'server-model' }
  void '打开 AI 后端设置'
  void router.push({ name: 'ai-book', query })
}

onBeforeRouteLeave(() => {
  clearChapterSummaryTimer()
  stopChapterSummarySiderResize()
  persistReadingProgressKeepalive()
  return true
})

onMounted(async () => {
  syncViewportSize()
  void loadChapterSummaryConfigForSider()
  appStore.startReadingSession()
  if (!store.book) {
    const restored = await store.restorePersistedSession()
    if (!restored) {
      router.replace('/')
      return
    }
    appStore.showToast('已恢复最近阅读的离线章节', 'success')
  }
  loadSavedReadingPosition()
  window.addEventListener('keydown', handleKeydown)
  document.addEventListener('mouseup', handleMouseUpSelection)
  document.addEventListener('touchend', handleTouchEndSelection)
    document.addEventListener('selectionchange', handleSelectionChange)
    checkMedia()
    window.addEventListener('resize', checkMedia)
    window.addEventListener(APP_VIEWPORT_CHANGE_EVENT, handleViewportChange)
    window.addEventListener('pagehide', handlePageHide)
    window.addEventListener('beforeunload', handleBeforeUnload)
    document.addEventListener('visibilitychange', handleVisibilityChange)
    store.fetchVoices()
  applySystemTheme(store.isNight ? 'dark' : appStore.theme, store.currentTheme.body)
  if (typeof window !== 'undefined' && window.speechSynthesis) {
    window.speechSynthesis.onvoiceschanged = () => store.fetchVoices()
  }
  speechTimerTicker = window.setInterval(() => {
    speechTimerNow.value = Date.now()
  }, 15000)
  await Promise.all([
    store.fetchBookmarks(),
    store.fetchReplaceRules(),
  ])
  scheduleRefreshOfflineCacheState()
  updateHorizontalMetrics()
  await rebuildHorizontalPages()
  if (isContinuousMode.value) {
    await initializeContinuousChapters(store.currentIndex, false)
  }
  scheduleRestoreReadingPosition()
})

onUnmounted(() => {
    clearChapterSummaryTimer()
    stopChapterSummarySiderResize()
    persistReadingProgressKeepalive()
    appStore.stopReadingSession()
    window.removeEventListener('keydown', handleKeydown)
  document.removeEventListener('mouseup', handleMouseUpSelection)
  document.removeEventListener('touchend', handleTouchEndSelection)
    document.removeEventListener('selectionchange', handleSelectionChange)
    window.removeEventListener('resize', checkMedia)
    window.removeEventListener(APP_VIEWPORT_CHANGE_EVENT, handleViewportChange)
    window.removeEventListener('pagehide', handlePageHide)
    window.removeEventListener('beforeunload', handleBeforeUnload)
    document.removeEventListener('visibilitychange', handleVisibilityChange)
  if (speechTimerTicker) clearInterval(speechTimerTicker)
  if (restorePositionTimer) clearTimeout(restorePositionTimer)
  if (persistPositionTimer) clearTimeout(persistPositionTimer)
  if (refreshOfflineCacheStateTimer) clearTimeout(refreshOfflineCacheStateTimer)
  clearRestoreStabilizers()
  disposeSelection()
  disposeContinuousReading()
  disposeAutoPlayback()
  store.stopTTS()
  if (typeof window !== 'undefined' && window.speechSynthesis) {
    window.speechSynthesis.onvoiceschanged = null
  }
  applySystemTheme(appStore.theme)
  store.closePanel()
})

watch(() => config.value.autoPageMode, () => {
  if (!store.isAutoScrolling) return
  stopAutoScroll()
  store.isAutoScrolling = true
  startAutoScroll()
})

watch(() => config.value.readMethod, async () => {
  clearSelectionState()
  if (isContinuousMode.value) {
    await initializeContinuousChapters(store.currentIndex, false)
  } else {
    clearContinuousChapters()
    await nextTick()
    if (scrollContainerRef.value) {
      scrollContainerRef.value.scrollTo({ top: 0, left: 0, behavior: 'auto' })
    }
  }
  if (isHorizontalPageMode.value && scrollContainerRef.value) {
    resetHorizontalPagePosition()
  }
  await rebuildHorizontalPages()
  updateHorizontalEndState()
  scheduleRestoreReadingPosition()
})

watch(() => store.currentIndex, () => {
  if (!isHorizontalPageMode.value) return
  resetHorizontalPagePosition()
  rebuildHorizontalPages()
  updateHorizontalEndState()
})

watch(
  () => [store.book?.bookUrl, store.currentChapter?.url, store.currentIndex, store.displayContent] as const,
  () => {
    resetChapterSummaryState()
    void loadChapterSummaryForCurrentChapter()
  },
  { immediate: true },
)

watch(
  () => config.value.chapterSummarySiderWidth,
  (width) => {
    if (!chapterSummarySiderResizing.value) {
      chapterSummarySiderWidth.value = clampChapterSummarySiderWidth(width)
    }
  },
)

watch(
  [() => store.content, () => config.value.fontSize, () => config.value.fontWeight, () => config.value.lineHeight, () => config.value.paragraphSpacing, () => config.value.firstLineIndent, showSearch, searchQuery],
  () => {
    if (isHorizontalPageMode.value) {
      horizontalPageIndex.value = 0
      rebuildHorizontalPages()
    }
  },
)

watch(() => store.currentIndex, async () => {
  loadSavedReadingPosition()
  resetAutoParagraphIndex()
  if (!store.isSpeaking) {
    clearReadingClass()
  }
  if (hideReadChaptersMode.value) {
    pruneReadChapters(store.currentIndex)
  }
  if (!isContinuousMode.value && config.value.enablePreload) {
    store.preloadAroundChapter(store.currentIndex)
  }
  if (isContinuousMode.value && !suppressContinuousSync.value) {
    await syncContinuousToStoreState()
  }
  scheduleRefreshOfflineCacheState()
  scheduleRestoreReadingPosition()
})

watch(
  [() => store.chapters.length, () => store.chaptersLoading, () => store.loading, isContinuousMode],
  async ([chapterCount, chaptersLoading, loadingNow, continuousMode]) => {
    if (!continuousMode || !chapterCount || chaptersLoading || loadingNow || continuousChapters.value.length) return
    await initializeContinuousChapters(store.currentIndex, false)
    scheduleRestoreReadingPosition()
  },
  { immediate: true },
)

watch(() => store.content, () => {
  resetAutoParagraphIndex()
  if (isContinuousMode.value) {
    const current = getContinuousChapter(store.currentIndex)
    if (current) {
      current.content = store.content
      current.html = renderChapterHtml(store.content)
    } else if (store.content) {
      void initializeContinuousChapters(store.currentIndex, false)
    }
  }
  handleContentChanged()
  handleContentUpdated()
  scheduleRefreshOfflineCacheState()
  scheduleRestoreReadingPosition()
})

watch(() => store.loading, (loading) => {
  if (!loading && pendingRestorePosition.value) {
    scheduleRestoreReadingPosition()
  }
})

watch(() => store.book?.bookUrl, () => {
  loadSavedReadingPosition()
  scheduleRefreshOfflineCacheState()
})

watch([showSearch, searchQuery, () => config.value.paragraphSpacing, () => config.value.firstLineIndent, () => config.value.chineseMode, () => store.replaceRules], () => {
  if (isContinuousMode.value) {
    syncContinuousChapterHtml()
  }
  handlePresentationUpdated()
})

watch(() => config.value.selectAction, (value) => {
  if (value !== 'popup') {
    clearSelectionState()
  }
})

watch(() => store.isSpeaking, (speaking) => {
  if (speaking && !ttsPanelDismissed.value) {
    showTTSPanel.value = true
  }
  if (!speaking && !store.isAutoScrolling) {
    clearReadingClass()
  }
})

watch(
  [() => store.isNight, () => store.currentTheme.body, () => appStore.theme],
  ([isNight, body]) => {
    applySystemTheme(isNight ? 'dark' : appStore.theme, body)
  },
  { immediate: true },
)
</script>

<style scoped>
.reader-view {
  height: 100vh;
  height: 100dvh;
  height: var(--app-visual-height, var(--app-height, 100dvh));
  width: 100%;
  display: flex;
  position: relative;
  overflow: hidden;
  transition: background 0.3s, color 0.3s;
  padding-top: var(--safe-area-top);
  padding-bottom: var(--safe-area-bottom);
  box-sizing: border-box;
}

.reader-view.disable-system-callout .chapter-text,
.reader-view.disable-system-callout .horizontal-page-content,
.reader-view.disable-system-callout .continuous-reading {
  -webkit-touch-callout: none;
}

.reader-scroll-container {
  flex: 1;
  height: 100%;
  overflow-y: auto;
  position: relative;
  scroll-behavior: smooth;
  overscroll-behavior: contain;
  -webkit-overflow-scrolling: touch;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.reader-scroll-container.horizontal-page-mode {
  overflow-x: hidden;
  overflow-y: hidden;
  touch-action: pan-y pinch-zoom;
  overscroll-behavior: none;
}

/* Hide scrollbar */
.reader-scroll-container::-webkit-scrollbar {
  width: 0;
  height: 0;
  display: none;
}
.reader-scroll-container::-webkit-scrollbar-thumb {
  background: rgba(0,0,0,0.1);
  border-radius: 4px;
}
.reader-view[style*="background: #1a1a2e"] .reader-scroll-container::-webkit-scrollbar-thumb {
  background: rgba(255,255,255,0.1);
}

.content-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.offline-banner {
  position: sticky;
  top: 0;
  z-index: 6;
  margin: 0 auto;
  width: min(100%, 880px);
  padding: 10px 16px;
  background: rgba(201, 127, 58, 0.12);
  color: var(--color-primary);
  border-bottom: 1px solid rgba(201, 127, 58, 0.18);
  font-size: 13px;
  line-height: 1.5;
  text-align: center;
  backdrop-filter: blur(6px);
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid rgba(0,0,0,0.1);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}
.reader-view[style*="background: #1a1a2e"] .loading-spinner {
  border-color: rgba(255,255,255,0.1);
  border-top-color: var(--color-primary);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.chapter-content {
  margin: 0 auto;
  padding: 80px 24px;
  min-height: 100%;
  transition: all 0.3s ease;
}

.chapter-content.horizontal-page-article {
  margin: 0;
  height: 100%;
  min-height: 100%;
  width: max-content;
  min-width: 100%;
  padding: 0;
}

.horizontal-page-layout {
  width: max-content;
  min-width: var(--reader-page-step);
  height: 100%;
}

.horizontal-content-page {
  width: max-content;
  min-width: var(--reader-page-step);
  height: 100%;
  min-height: 100%;
  padding: 0;
  box-sizing: border-box;
}

.horizontal-pages {
  display: flex;
  width: max-content;
  height: 100%;
  min-height: 100%;
  transform: translate3d(0, 0, 0);
  transition-property: transform;
  transition-timing-function: cubic-bezier(0.22, 0.61, 0.36, 1);
  will-change: transform;
}

.horizontal-page {
  width: var(--reader-page-step);
  min-width: var(--reader-page-step);
  height: 100%;
  min-height: 100%;
  padding: 24px var(--reader-side-padding);
  box-sizing: border-box;
}

.continuous-reading {
  margin: 0 auto;
  padding: 32px 0 80px;
}

.continuous-chapter {
  min-height: auto;
  padding-top: 48px;
  padding-bottom: 24px;
}

.chapter-title {
  font-size: 1.6em;
  font-weight: 700;
  margin-bottom: 2em;
  text-align: center;
  line-height: 1.4;
}

.chapter-summary-card {
  margin: -8px 0 32px;
  padding: 20px 22px;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 18px;
  background: color-mix(in srgb, currentColor 3%, transparent);
  transition: border-color 0.2s, background 0.2s;
}

.chapter-summary-card:hover {
  border-color: var(--color-primary, #c97f3a);
}

.chapter-summary-header {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  border: 0;
  padding: 0;
  color: inherit;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.summary-kicker {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--color-primary, #c97f3a);
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0;
}

.summary-kicker::before {
  content: '';
  width: 7px;
  height: 7px;
  border-radius: 99px;
  background: currentColor;
  opacity: 0.75;
}

.summary-muted {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.62;
  font-size: 0.92em;
}

.chapter-summary-body {
  margin-top: 10px;
  line-height: 1.75;
}

.summary-main {
  margin: 0 0 12px;
  max-width: 68ch;
  font-weight: 400;
  text-indent: 2em;
  text-wrap: pretty;
}

.summary-main.summary-muted {
  text-indent: 0;
}

.summary-list {
  margin-top: 14px;
  padding: 12px 14px;
  border: 1px solid color-mix(in srgb, var(--color-primary, #c97f3a) 16%, transparent);
  border-radius: 14px;
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 7%, transparent);
  font-size: 0.88em;
  line-height: 1.65;
}

.summary-list strong {
  display: block;
  margin-bottom: 6px;
  color: inherit;
  font-size: 0.9em;
  font-weight: 600;
  opacity: 0.72;
  letter-spacing: 0;
}

.summary-list ul {
  display: grid;
  gap: 1px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.summary-list li {
  position: relative;
  padding: 1px 0 1px 18px;
  text-wrap: pretty;
}

.summary-list li::before {
  content: '';
  position: absolute;
  top: 0.95em;
  left: 3px;
  width: 4px;
  height: 4px;
  border-radius: 99px;
  background: var(--color-primary, #c97f3a);
}

.summary-list.style-card ul {
  display: grid;
  gap: 1px;
}

.summary-list.style-card li {
  padding: 1px 0 1px 18px;
}

.summary-list.style-card li::before {
  display: block;
}

.summary-list.style-list {
  padding: 12px 0 0;
  border: 0;
  border-top: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 0;
  background: transparent;
}

.summary-list.style-list li {
  position: relative;
  padding: 1px 0 1px 18px;
}

.summary-skeleton {
  display: grid;
  gap: 10px;
  margin-bottom: 18px;
}

.summary-skeleton span {
  height: 12px;
  border-radius: 99px;
  background: linear-gradient(
    90deg,
    color-mix(in srgb, currentColor 7%, transparent),
    color-mix(in srgb, currentColor 13%, transparent),
    color-mix(in srgb, currentColor 7%, transparent)
  );
}

.summary-skeleton span:nth-child(2) {
  width: 86%;
}

.summary-skeleton span:nth-child(3) {
  width: 62%;
}

.summary-error {
  margin: 12px 0 0;
  color: #d25f4f;
  font-size: 0.9em;
}

.summary-actions {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}

.summary-actions.compact {
  margin-top: 12px;
}

.summary-action {
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
  border-radius: 20px;
  padding: 6px 14px;
  color: inherit;
  background: transparent;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  opacity: 0.78;
  transition: border-color 0.2s, color 0.2s, background 0.2s, transform 0.18s;
}

.summary-action:hover:not(:disabled) {
  border-color: var(--color-primary, #c97f3a);
  color: var(--color-primary, #c97f3a);
  background: transparent;
  opacity: 1;
  transform: translateY(-1px);
}

.summary-action:active:not(:disabled) {
  transform: translateY(0);
}

.summary-action:focus-visible,
.summary-panel-close:focus-visible,
.chapter-summary-header:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--color-primary, #c97f3a) 70%, transparent);
  outline-offset: 3px;
}

.summary-action:disabled {
  cursor: default;
  opacity: 0.5;
}

.reader-ui-font,
.reader-drawer,
.reader-toolbar,
.reader-mobile-controls,
.reader-overlay,
.chapter-summary-sider,
.selection-menu,
.summary-action,
.summary-tabs,
.summary-setting-group,
.summary-setting-field,
.summary-prompt-input {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", Arial, sans-serif;
}


.chapter-summary-card.side {
  margin: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.chapter-summary-sider {
  position: relative;
  flex: 0 0 auto;
  height: auto;
  overflow-y: auto;
  margin: 16px 16px 16px 0;
  border: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  border-radius: 24px;
  box-shadow: -10px 10px 30px rgba(0, 0, 0, 0.08);
  padding: 0 20px 24px;
  box-sizing: border-box;
  transition: background 0.3s, color 0.3s, box-shadow 0.2s;
}

.chapter-summary-sider.resizing {
  user-select: none;
}

.chapter-summary-resize-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  left: -4px;
  width: 8px;
  cursor: col-resize;
}

.chapter-summary-resize-handle::after {
  content: '';
  position: absolute;
  top: 28px;
  bottom: 28px;
  left: 3px;
  width: 2px;
  border-radius: 99px;
  background: color-mix(in srgb, currentColor 10%, transparent);
}

.chapter-summary-sider-head {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin: 0 -20px 10px;
  padding: 14px 20px 10px;
  background: color-mix(in srgb, currentColor 3%, transparent);
  backdrop-filter: blur(10px);
  border-bottom: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  font-size: 12px;
}

.chapter-summary-sider-head .summary-kicker {
  font-size: 12px;
}

.chapter-summary-sider-head .summary-muted {
  font-size: 11px;
}

.chapter-summary-card.side .chapter-summary-body {
  margin-top: 0;
}

.chapter-summary-settings-panel {
  display: grid;
  gap: 12px;
}

.summary-tabs {
  display: inline-flex;
  gap: 4px;
  padding: 4px;
  border: 1px solid color-mix(in srgb, currentColor 12%, transparent);
  border-radius: 11px;
  background: color-mix(in srgb, currentColor 4%, transparent);
}

.summary-tab {
  border: 0;
  border-radius: 8px;
  padding: 5px 9px;
  color: inherit;
  background: transparent;
  font-size: 12px;
  opacity: 0.68;
  cursor: pointer;
}

.summary-tab.active {
  color: var(--color-primary, #c97f3a);
  background: color-mix(in srgb, currentColor 4%, transparent);
  opacity: 1;
}

.summary-setting-group {
  display: grid;
  gap: 10px;
  padding: 14px;
  border: 1px solid color-mix(in srgb, currentColor 9%, transparent);
  border-radius: 16px;
  background: color-mix(in srgb, currentColor 3%, transparent);
}

.summary-setting-title {
  color: var(--color-primary, #c97f3a);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
}

.summary-setting-row,
.summary-setting-field {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: inherit;
  font-size: 12px;
}

.summary-setting-row > span,
.summary-setting-field > span {
  opacity: 0.72;
}

.summary-setting-field input,
.summary-prompt-input {
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
  border-radius: 10px;
  padding: 7px 9px;
  color: inherit;
  background: color-mix(in srgb, currentColor 3%, transparent);
}

.summary-setting-field input {
  width: 96px;
  box-sizing: border-box;
}

.summary-prompt-input {
  width: 100%;
  box-sizing: border-box;
  resize: vertical;
  line-height: 1.55;
  font-size: 12px;
}

.summary-switch,
.summary-stepper {
  display: inline-flex;
  gap: 3px;
  padding: 3px;
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, currentColor 3%, transparent);
}

.summary-switch button,
.summary-stepper button,
.summary-stepper span {
  border: 0;
  border-radius: 7px;
  padding: 5px 9px;
  color: inherit;
  background: transparent;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.16s, color 0.16s, opacity 0.16s;
}

.summary-switch button.active {
  color: var(--color-primary, #c97f3a);
  background: color-mix(in srgb, currentColor 5%, transparent);
}

.summary-switch button:disabled,
.summary-stepper button:disabled {
  cursor: default;
  opacity: 0.42;
}

.summary-stepper span {
  min-width: 40px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.summary-setting-note {
  margin: 0;
  color: inherit;
  opacity: 0.62;
  font-size: 12px;
  line-height: 1.5;
}

.chapter-summary-collapsed-pill {
  position: fixed;
  right: 88px;
  top: calc(var(--safe-area-top) + 84px);
  z-index: 25;
  border: 1px solid color-mix(in srgb, var(--color-primary, #c97f3a) 45%, transparent);
  border-radius: 999px;
  padding: 8px 14px;
  color: var(--color-primary, #c97f3a);
  background: color-mix(in srgb, var(--color-primary, #c97f3a) 8%, transparent);
  backdrop-filter: blur(8px);
  cursor: pointer;
}

.summary-panel-close {
  flex: 0 0 auto;
  border: 0;
  color: inherit;
  background: transparent;
  opacity: 0.6;
  cursor: pointer;
}

.chapter-text {
  word-break: normal;
  overflow-wrap: anywhere;
  text-align: justify;
  user-select: text;
  -webkit-user-select: text;
  -webkit-touch-callout: default;
}

.horizontal-page-content {
  height: 100%;
  overflow: hidden;
  overflow-wrap: break-word;
  text-align: left;
  word-break: normal;
}

:deep(.horizontal-page-content .horizontal-flow-title) {
  margin: 0 0 1em 0;
  font-size: 1.5em;
  line-height: 1.35;
  font-weight: 700;
  text-align: center;
  break-inside: avoid;
}

:deep(.horizontal-page-content p:first-child) {
  margin-top: 0 !important;
}

:deep(.horizontal-page-content p:last-child) {
  margin-bottom: 0 !important;
}

:deep(.chapter-text p.reading) {
  background: rgba(201, 127, 58, 0.12);
  border-radius: 10px;
  box-shadow: inset 0 0 0 1px rgba(201, 127, 58, 0.18);
}

:deep(.chapter-text p.reader-indent) {
  text-indent: 2em !important;
}

:deep(.chapter-text p) {
  text-indent: 0;
  user-select: text;
  -webkit-user-select: text;
}

.chapter-footer {
  margin-top: 60px;
  text-align: center;
  padding-bottom: 40px;
}

.horizontal-next-floating {
  position: absolute;
  left: 50%;
  bottom: calc(20px + var(--safe-area-bottom));
  transform: translateX(-50%);
  z-index: 12;
  pointer-events: none;
}

.horizontal-next-floating .next-btn {
  pointer-events: auto;
  background: rgba(255, 255, 255, 0.75);
  backdrop-filter: blur(6px);
}

.continuous-loading-inline {
  text-align: center;
  padding: 18px 24px;
  opacity: 0.6;
  font-size: 13px;
}

.next-btn {
  padding: 12px 36px;
  border-radius: 30px;
  background: transparent;
  border: 1px solid currentColor;
  color: inherit;
  font-size: 14px;
  opacity: 0.6;
  cursor: pointer;
  transition: all 0.2s;
}

.next-btn:hover:not(:disabled) {
  opacity: 1;
  background: rgba(0,0,0,0.05);
}

.next-btn:disabled {
  opacity: 0.2;
  cursor: not-allowed;
}



/* Slide Drawer Overlay */
.reader-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.4);
  z-index: 40;
}

.reader-drawer {
  position: fixed;
  top: var(--safe-area-top);
  bottom: var(--safe-area-bottom);
  left: 0;
  width: min(340px, 85vw);
  z-index: 50;
  box-shadow: 4px 0 24px rgba(0,0,0,0.15);
  transition: background 0.3s;
}

.selection-menu {
  position: fixed;
  z-index: 60;
  min-width: 220px;
  max-width: min(320px, calc(100vw - 32px));
  border-radius: 14px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.18);
  border: 1px solid rgba(0, 0, 0, 0.06);
  overflow: hidden;
}

.selection-menu-text {
  padding: 12px 14px 8px;
  font-size: 13px;
  line-height: 1.5;
  opacity: 0.72;
  word-break: break-all;
}

.selection-menu-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  padding: 0 12px 12px;
}

.selection-menu-actions button {
  border: none;
  border-radius: 10px;
  padding: 10px 12px;
  background: var(--color-primary);
  color: #fff;
  font-size: 13px;
  cursor: pointer;
}

.selection-menu-actions button:first-child {
  grid-column: 1 / -1;
}

:deep(.search-highlight) {
  background: yellow;
  color: black;
  border-radius: 2px;
}

:deep(.search-highlight.current-match) {
  background: orange;
}

@media (max-width: 768px) {
  .reader-scroll-container.horizontal-page-mode {
    scroll-behavior: auto;
  }

  .chapter-content {
    padding: 24px 20px 8px;
    min-height: auto;
    height: auto;
  }

  .continuous-reading {
    padding: 16px 0 8px;
  }

  .continuous-chapter {
    padding-top: 20px;
    padding-bottom: 8px;
  }

  .chapter-title {
    margin-bottom: 0.9em;
  }

  .chapter-footer {
    margin-top: 12px;
    padding-bottom: 0;
  }

  .chapter-summary-sider,
  .chapter-summary-collapsed-pill {
    display: none;
  }

  .reader-drawer {
    top: var(--safe-area-top);
    bottom: var(--safe-area-bottom);
    width: min(340px, 85vw);
    padding-top: var(--safe-area-top);
    padding-bottom: var(--safe-area-bottom);
    box-sizing: border-box;
  }
}

/* Transitions */
.fade-enter-active, .fade-leave-active { transition: opacity 0.3s; }
.fade-enter-from, .fade-leave-to { opacity: 0; }

.slide-left-enter-active, .slide-left-leave-active { transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1); }
.slide-left-enter-from, .slide-left-leave-to { transform: translateX(-100%); }

.fade-slide-right-enter-active, .fade-slide-right-leave-active { transition: all 0.3s ease; }
.fade-slide-right-enter-from, .fade-slide-right-leave-to { transform: translateX(-20px); opacity: 0; }

.fade-slide-left-enter-active, .fade-slide-left-leave-active { transition: all 0.3s ease; }
.fade-slide-left-enter-from, .fade-slide-left-leave-to { transform: translateX(20px); opacity: 0; }
</style>
