<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="modelValue" class="drawer-overlay" @click="close"></div>
    </Transition>
    <Transition name="slide-right">
      <aside v-if="modelValue" class="settings-drawer">
        <div class="drawer-header">
          <h2>&#35774;&#32622;</h2>
          <button class="close-btn" @click="close">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div class="drawer-body">
          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
              &#29992;&#25143;
            </h3>
            <div v-if="appStore.isLoggedIn" class="user-info-card">
              <div class="user-avatar-lg">
                {{ appStore.userInfo?.username?.charAt(0)?.toUpperCase() || 'U' }}
              </div>
              <div class="user-panel">
                <div class="user-card-header">
                  <div class="user-detail">
                    <span class="user-name">{{ appStore.userInfo?.username }}</span>
                    <span class="user-role">{{ appStore.userInfo?.isAdmin ? '\u7ba1\u7406\u5458' : '\u666e\u901a\u7528\u6237' }}</span>
                  </div>
                  <button class="action-btn danger" @click="handleLogout">&#27880;&#38144;</button>
                </div>
                <button class="action-btn inline-link" @click="togglePasswordPanel">
                  {{ showPasswordPanel ? '\u6536\u8d77\u4fee\u6539\u5bc6\u7801' : '\u4fee\u6539\u5bc6\u7801' }}
                </button>
                <div v-if="showPasswordPanel" class="password-panel embedded">
                  <label class="password-field">
                    <span>&#24403;&#21069;&#23494;&#30721;</span>
                    <input v-model="passwordForm.oldPassword" type="password" autocomplete="current-password" />
                  </label>
                  <label class="password-field">
                    <span>&#26032;&#23494;&#30721;</span>
                    <input v-model="passwordForm.newPassword" type="password" autocomplete="new-password" />
                  </label>
                  <label class="password-field">
                    <span>&#30830;&#35748;&#26032;&#23494;&#30721;</span>
                    <input v-model="passwordForm.confirmPassword" type="password" autocomplete="new-password" />
                  </label>
                  <div class="password-actions">
                    <button class="action-btn primary" :disabled="changingPassword" @click="handleChangePassword">
                      {{ changingPassword ? '\u63d0\u4ea4\u4e2d...' : '\u4fdd\u5b58\u65b0\u5bc6\u7801' }}
                    </button>
                  </div>
                </div>
              </div>
            </div>
            <button v-else class="action-btn primary full" @click="handleLogin">
              &#30331;&#24405; / &#27880;&#20876;
            </button>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M12 3a4 4 0 0 0-4 4v2H7a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-1V7a4 4 0 0 0-4-4Z" />
                <path d="M9 9V7a3 3 0 0 1 6 0v2" />
              </svg>
              管理密码
            </h3>
            <div class="status-card">
              <span>{{ appStore.secureKeyRequired ? '服务端已配置管理密码' : '服务端未配置管理密码' }}</span>
              <small>
                {{
                  appStore.secureKeyRequired
                    ? (appStore.adminAuthorized ? '当前请求已具备管理员权限。' : '保存后会随请求自动附带 X-Secure-Key。')
                    : '未配置时只依赖管理员账号登录态。'
                }}
              </small>
            </div>
            <div class="password-panel embedded">
              <label class="password-field">
                <span>管理密码</span>
                <input v-model="secureKeyInput" type="password" autocomplete="off" placeholder="输入服务端 SECURE_KEY" />
              </label>
              <div class="password-actions">
                <button class="action-btn primary" @click="handleSaveSecureKey">保存管理密码</button>
                <button class="action-btn" :disabled="!appStore.secureKey" @click="handleClearSecureKey">清除</button>
              </div>
            </div>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" />
              </svg>
              &#20070;&#28304;&#31649;&#29702;
            </h3>
            <div class="btn-group">
              <button class="action-btn" @click="openSourceManager">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                  <path d="M12 20h9M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
                </svg>
                &#20070;&#28304;&#31649;&#29702;
              </button>
            </div>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
                <circle cx="9" cy="7" r="4" />
                <path d="M19 8v6" />
                <path d="M22 11h-6" />
              </svg>
              &#29992;&#25143;&#31649;&#29702;
            </h3>
            <div v-if="appStore.isSecureMode" class="status-card">
              <span>{{ userManagerTitle }}</span>
              <small>{{ userManagerMessage }}</small>
            </div>
            <div v-else class="status-card">
              <span>&#24403;&#21069;&#26410;&#24320;&#21551;&#23433;&#20840;&#27169;&#24335;</span>
              <small>&#29992;&#25143;&#31649;&#29702;&#20165;&#22312;&#22810;&#29992;&#25143;&#23433;&#20840;&#27169;&#24335;&#19979;&#21487;&#29992;&#12290;</small>
            </div>
            <div class="btn-group">
              <button class="action-btn" :disabled="!canManageUsers" @click="openUserManager">
                &#29992;&#25143;&#31649;&#29702;
              </button>
            </div>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M12 3v18" />
                <path d="M5 8h14" />
                <path d="M5 16h14" />
              </svg>
              后端模型
            </h3>
            <div class="status-card">
              <span>{{ aiModelStatusTitle }}</span>
              <small>{{ aiModelStatusMessage }}</small>
            </div>
            <details class="model-config">
              <summary>模型设置</summary>
              <div class="password-panel embedded">
                <label class="switch-line">
                  <input v-model="aiModelConfig.text.enabled" type="checkbox" />
                  <span class="switch-ui"></span>
                  <span>启用文本模型</span>
                </label>
                <label class="password-field">
                  <span>文本 Base URL</span>
                  <input v-model="aiModelConfig.text.baseUrl" placeholder="https://api.openai.com" />
                </label>
                <label class="password-field">
                  <span>文本模型</span>
                  <input v-model="aiModelConfig.text.model" placeholder="gpt-4o-mini" />
                </label>
                <label class="password-field">
                  <span>文本路径</span>
                  <input v-model="aiModelConfig.text.path" placeholder="/v1/chat/completions" />
                </label>
                <label class="password-field">
                  <span>文本 API Key</span>
                  <input v-model="aiModelConfig.text.apiKey" type="password" autocomplete="off" />
                </label>

                <label class="switch-line">
                  <input v-model="aiModelConfig.image.enabled" type="checkbox" />
                  <span class="switch-ui"></span>
                  <span>启用图片模型</span>
                </label>
                <label class="password-field">
                  <span>图片 Base URL</span>
                  <input v-model="aiModelConfig.image.baseUrl" />
                </label>
                <label class="password-field">
                  <span>图片模型</span>
                  <input v-model="aiModelConfig.image.model" placeholder="gpt-image-1" />
                </label>
                <label class="password-field">
                  <span>图片路径</span>
                  <input v-model="aiModelConfig.image.path" placeholder="/v1/images/generations" />
                </label>
                <label class="password-field">
                  <span>图片 API Key</span>
                  <input v-model="aiModelConfig.image.apiKey" type="password" autocomplete="off" />
                </label>

                <label class="switch-line">
                  <input v-model="aiModelConfig.speech.enabled" type="checkbox" />
                  <span class="switch-ui"></span>
                  <span>启用语音模型</span>
                </label>
                <label class="password-field">
                  <span>语音 Base URL</span>
                  <input v-model="aiModelConfig.speech.baseUrl" />
                </label>
                <label class="password-field">
                  <span>语音模型</span>
                  <input v-model="aiModelConfig.speech.model" placeholder="gpt-4o-mini-tts" />
                </label>
                <label class="password-field">
                  <span>语音路径</span>
                  <input v-model="aiModelConfig.speech.path" placeholder="/v1/audio/speech" />
                </label>
                <label class="password-field">
                  <span>语音 API Key</span>
                  <input v-model="aiModelConfig.speech.apiKey" type="password" autocomplete="off" />
                </label>

                <div class="password-actions">
                  <button class="action-btn primary" :disabled="!aiModelIsAdmin || aiModelSaving" @click="handleSaveAiModelConfig">
                    {{ aiModelSaving ? '保存中...' : '保存后端模型' }}
                  </button>
                </div>
              </div>
            </details>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <path d="M7 10l5 5 5-5" />
                <path d="M12 15V3" />
              </svg>
              &#26381;&#21153;&#22120;&#22791;&#20221;
            </h3>
            <div class="status-card">
              <span>{{ webdavStatusTitle }}</span>
              <small>{{ webdavStatusMessage }}</small>
            </div>
            <div class="btn-group">
              <button class="action-btn" :disabled="!canOpenWebdav" @click="openWebdavManager">
                &#22791;&#20221;&#19982;&#24674;&#22797;
              </button>
            </div>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M12 16V4" />
                <path d="m7 9 5-5 5 5" />
                <path d="M20 16.5a2.5 2.5 0 0 1-2.5 2.5h-11A2.5 2.5 0 0 1 4 16.5" />
              </svg>
              &#24212;&#29992;
            </h3>
            <div class="status-card">
              <span>{{ appStore.isOnline ? '\u5728\u7ebf' : '\u79bb\u7ebf' }}</span>
              <small>{{ appStore.pwaReady ? '\u5df2\u542f\u7528\u79bb\u7ebf\u5916\u58f3\u7f13\u5b58' : '\u79bb\u7ebf\u5916\u58f3\u672a\u542f\u7528' }}</small>
            </div>
            <div class="status-card">
              <span>{{ appVersion }}</span>
              <small>当前应用版本</small>
            </div>
            <template v-if="appStore.canCheckVersionUpdate">
              <div
                class="status-card"
                :class="{ accent: appStore.hasVersionUpdateReminder, muted: appStore.versionUpdateLoading }"
              >
                <span>{{ versionUpdateTitle }}</span>
                <small>{{ versionUpdateMessage }}</small>
              </div>
              <div class="btn-group version-actions">
                <button class="action-btn" :disabled="!appStore.versionUpdate?.releaseUrl" @click="handleOpenRelease">
                  查看 Release
                </button>
                <button
                  class="action-btn"
                  :disabled="!appStore.hasVersionUpdateReminder || appStore.versionUpdateLoading"
                  @click="handleDismissVersionUpdate"
                >
                  本版本不再提醒
                </button>
                <button class="action-btn" :disabled="appStore.versionUpdateLoading" @click="handleCheckVersionUpdate">
                  {{ appStore.versionUpdateLoading ? '检查中...' : '重新检查' }}
                </button>
              </div>
            </template>
            <div v-if="appStore.pwaUpdateAvailable" class="status-card accent">
              <span>&#21457;&#29616;&#26032;&#29256;&#26412;</span>
              <small>&#21047;&#26032;&#21518;&#21487;&#20351;&#29992;&#26368;&#26032;&#31163;&#32447;&#36164;&#28304;</small>
            </div>
            <div class="btn-group">
              <button class="action-btn" :disabled="!appStore.deferredInstallPrompt" @click="handleInstallPwa">
                &#23433;&#35013;&#21040;&#20027;&#23631;&#24149;
              </button>
              <button class="action-btn primary" :disabled="!appStore.pwaUpdateAvailable" @click="handleApplyUpdate">
                &#26356;&#26032;&#24212;&#29992;
              </button>
            </div>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <rect width="7" height="7" x="3" y="3" rx="1" />
                <rect width="7" height="7" x="14" y="3" rx="1" />
                <rect width="7" height="7" x="3" y="14" rx="1" />
                <rect width="7" height="7" x="14" y="14" rx="1" />
              </svg>
              &#20070;&#26550;&#35774;&#32622;
            </h3>
            <div class="btn-group">
              <button class="action-btn" @click="refreshCache">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
                  <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                  <path d="M3 3v5h5" />
                  <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
                  <path d="M16 16h5v5" />
                </svg>
                &#21047;&#26032;&#32531;&#23384;
              </button>
            </div>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <path d="M12 8v4l3 3" />
                <circle cx="12" cy="12" r="9" />
              </svg>
              &#38405;&#35835;&#32479;&#35745;
            </h3>
            <div class="stats-grid">
              <div class="status-card">
                <span>{{ appStore.readingStatsSummary.totalTimeText }}</span>
                <small>&#32047;&#35745;&#38405;&#35835;&#26102;&#38271;</small>
              </div>
              <div class="status-card">
                <span>{{ appStore.readingStatsSummary.openedBooks }}</span>
                <small>&#25171;&#24320;&#36807;&#30340;&#20070;&#31821;</small>
              </div>
              <div class="status-card">
                <span>{{ appStore.readingStatsSummary.readChapters }}</span>
                <small>&#38405;&#35835;&#31456;&#33410;&#25968;</small>
              </div>
              <div class="status-card">
                <span>{{ appStore.readingStatsSummary.completedBooks }}</span>
                <small>&#35835;&#23436;&#20070;&#31821;&#25968;</small>
              </div>
            </div>
          </section>

          <section class="drawer-section">
            <h3 class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
                <circle cx="12" cy="12" r="4" />
                <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
              </svg>
              &#22806;&#35266;
            </h3>
            <div class="theme-toggle">
              <button
                class="theme-option"
                :class="{ active: appStore.theme === 'light' }"
                @click="setTheme('light')"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20">
                  <circle cx="12" cy="12" r="4" />
                  <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
                </svg>
                &#20142;&#33394;
              </button>
              <button
                class="theme-option"
                :class="{ active: appStore.theme === 'dark' }"
                @click="setTheme('dark')"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20">
                  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
                </svg>
                &#26263;&#33394;
              </button>
            </div>
          </section>
        </div>
      </aside>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useAppStore } from '../stores/app'
import { useBookshelfStore } from '../stores/bookshelf'
import { getAiModelConfig, saveAiModelConfig } from '../api/aiModel'
import { changePassword, logout as apiLogout } from '../api/user'
import type { AiServerModelConfig } from '../types'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

const appStore = useAppStore()
const shelfStore = useBookshelfStore()
const appVersion = __APP_VERSION__
const showPasswordPanel = ref(false)
const changingPassword = ref(false)
const secureKeyInput = ref(appStore.secureKey)
const aiModelConfig = reactive<AiServerModelConfig>(emptyAiModelConfig())
const aiModelLoaded = ref(false)
const aiModelLoading = ref(false)
const aiModelSaving = ref(false)
const aiModelIsAdmin = ref(false)
const aiModelCanUse = ref(false)
const passwordForm = reactive({
  oldPassword: '',
  newPassword: '',
  confirmPassword: '',
})

const canManageUsers = computed(() => appStore.isSecureMode && appStore.adminAuthorized)
const userManagerTitle = computed(() => {
  if (appStore.adminAuthorized) return '\u5f53\u524d\u8bf7\u6c42\u5df2\u5177\u5907\u7ba1\u7406\u5458\u6743\u9650'
  if (!appStore.isLoggedIn) return '\u767b\u5f55\u540e\u53ef\u67e5\u770b\u72b6\u6001'
  if (appStore.needSecureKey) return '\u5f53\u524d\u9700\u8981\u7ba1\u7406\u5bc6\u7801'
  return appStore.userInfo?.isAdmin ? '\u5f53\u524d\u8d26\u53f7\u62e5\u6709\u7ba1\u7406\u5458\u6743\u9650' : '\u5f53\u524d\u8d26\u53f7\u4e0d\u662f\u7ba1\u7406\u5458'
})
const userManagerMessage = computed(() => {
  if (appStore.adminAuthorized) return '\u5df2\u7ecf\u901a\u8fc7\u7ba1\u7406\u5458\u6743\u9650\u6821\u9a8c\uff0c\u53ef\u8bfb\u53d6\u5e76\u4fee\u6539\u7528\u6237\u5217\u8868\u3002'
  if (!appStore.isLoggedIn) return '\u8bf7\u5148\u767b\u5f55\u7ba1\u7406\u5458\u8d26\u53f7\u540e\u7ba1\u7406\u5176\u4ed6\u7528\u6237\u3002'
  if (appStore.needSecureKey) return '\u670d\u52a1\u7aef\u5df2\u5f00\u542f\u7ba1\u7406\u5bc6\u7801\u6821\u9a8c\uff0c\u672a\u9a8c\u8bc1\u524d\u65e0\u6cd5\u8bfb\u53d6\u7528\u6237\u5217\u8868\u3002'
  return appStore.userInfo?.isAdmin
    ? '\u652f\u6301\u65b0\u589e\u7528\u6237\u3001\u91cd\u7f6e\u5bc6\u7801\u3001\u5220\u9664\u7528\u6237\u548c\u8c03\u6574\u6743\u9650\u3002'
    : '\u8bf7\u4f7f\u7528\u7ba1\u7406\u5458\u8d26\u53f7\u767b\u5f55\u540e\u518d\u8fdb\u884c\u7528\u6237\u7ba1\u7406\u3002'
})
const canOpenWebdav = computed(() => appStore.isSecureMode && appStore.isLoggedIn && !!appStore.userInfo?.enableWebdav)
const aiModelStatusTitle = computed(() => {
  if (aiModelLoading.value) return '正在读取后端模型配置'
  if (!appStore.isLoggedIn) return '登录后可查看'
  if (aiModelIsAdmin.value) return '管理员可编辑后端模型配置'
  return aiModelCanUse.value ? '当前账号可使用后端模型' : '当前账号未开启 AI 模型权限'
})
const aiModelStatusMessage = computed(() => {
  if (aiModelIsAdmin.value) return '配置保存到服务器，AI资料生成会使用这里的文本模型。'
  if (aiModelCanUse.value) return '可使用管理员配置；API Key 不会展示。'
  return '需要管理员账号或管理密码才能保存配置。'
})
const webdavStatusTitle = computed(() => {
  if (!appStore.isSecureMode) return '\u4ec5\u5b89\u5168\u6a21\u5f0f\u652f\u6301\u670d\u52a1\u5668\u5907\u4efd'
  if (!appStore.isLoggedIn) return '\u767b\u5f55\u540e\u53ef\u7528'
  return appStore.userInfo?.enableWebdav ? '\u5f53\u524d\u8d26\u53f7\u5df2\u5f00\u542f\u670d\u52a1\u5668\u5907\u4efd' : '\u5f53\u524d\u8d26\u53f7\u672a\u5f00\u542f\u670d\u52a1\u5668\u5907\u4efd'
})
const webdavStatusMessage = computed(() => {
  if (!appStore.isSecureMode) return '\u4e3a\u907f\u514d\u5171\u4eab\u5907\u4efd\u7a7a\u95f4\uff0c\u8bf7\u5148\u5f00\u542f\u591a\u7528\u6237\u5b89\u5168\u6a21\u5f0f\u3002'
  if (!appStore.isLoggedIn) return '\u767b\u5f55\u5e76\u5177\u5907\u5907\u4efd\u6743\u9650\u540e\uff0c\u53ef\u7ba1\u7406\u670d\u52a1\u5668\u4e2d\u7684\u5907\u4efd\u6587\u4ef6\u3002'
  return appStore.userInfo?.enableWebdav
    ? '\u652f\u6301\u5c06\u6570\u636e\u5907\u4efd\u5230\u670d\u52a1\u5668\u3001\u4e0b\u8f7d\u5907\u4efd\u6587\u4ef6\u3001\u4e0a\u4f20\u5907\u4efd\u6587\u4ef6\u5e76\u6267\u884c\u6062\u590d\u3002'
    : '\u8bf7\u5728\u7528\u6237\u7ba1\u7406\u4e2d\u4e3a\u5f53\u524d\u8d26\u53f7\u5f00\u542f\u670d\u52a1\u5668\u5907\u4efd\u6743\u9650\u3002'
})
const versionUpdateTitle = computed(() => {
  const info = appStore.versionUpdate
  if (appStore.versionUpdateLoading && !info) return '正在检查服务端版本'
  if (!info) return '服务端版本检查'
  if (info.error && !info.latestVersion) return '版本检查失败'
  if (info.updateAvailable) return `发现服务端新版本 ${info.latestVersion}`
  return '服务端已是最新版本'
})
const versionUpdateMessage = computed(() => {
  const info = appStore.versionUpdate
  if (appStore.versionUpdateLoading && !info) return '正在从 GitHub Release 获取最新版本。'
  if (!info) return '管理员可检查 GitHub Release，发现新版后会在设置入口提示。'
  if (info.error && !info.latestVersion) return info.error
  if (info.updateAvailable && info.shouldRemind) {
    return `当前 ${info.currentVersion}，最新 ${info.latestVersion}。`
  }
  if (info.updateAvailable) {
    return `当前 ${info.currentVersion}，最新 ${info.latestVersion}，本版本已设置不再提醒。`
  }
  if (info.error) return `当前 ${info.currentVersion}，上次检查失败：${info.error}`
  return `当前 ${info.currentVersion}。`
})

watch(
  () => appStore.secureKey,
  (value) => {
    secureKeyInput.value = value
  },
)

watch(
  () => props.modelValue,
  (open) => {
    if (open && !aiModelLoaded.value) {
      loadAiModelConfig()
    }
  },
)

function close() {
  emit('update:modelValue', false)
}

function handleLogin() {
  close()
  appStore.showLoginModal = true
}

async function handleLogout() {
  await apiLogout()
  appStore.clearUser()
  await appStore.fetchUserInfo()
  close()
  shelfStore.fetchBooks()
}

async function handleSaveSecureKey() {
  appStore.setSecureKey(secureKeyInput.value)
  await appStore.fetchUserInfo()
  appStore.showToast(appStore.adminAuthorized ? '管理密码已生效' : '管理密码已保存，但当前仍未通过管理员校验', appStore.adminAuthorized ? 'success' : 'warning')
}

async function handleClearSecureKey() {
  secureKeyInput.value = ''
  appStore.setSecureKey('')
  await appStore.fetchUserInfo()
  appStore.showToast('已清除管理密码', 'success')
}

function resetPasswordForm() {
  passwordForm.oldPassword = ''
  passwordForm.newPassword = ''
  passwordForm.confirmPassword = ''
}

function togglePasswordPanel() {
  showPasswordPanel.value = !showPasswordPanel.value
  if (!showPasswordPanel.value) {
    resetPasswordForm()
  }
}

async function handleChangePassword() {
  if (!passwordForm.oldPassword || !passwordForm.newPassword || !passwordForm.confirmPassword) {
    appStore.showToast('\u8bf7\u586b\u5199\u5b8c\u6574\u7684\u5bc6\u7801\u4fe1\u606f', 'warning')
    return
  }
  if (passwordForm.newPassword !== passwordForm.confirmPassword) {
    appStore.showToast('\u4e24\u6b21\u8f93\u5165\u7684\u65b0\u5bc6\u7801\u4e0d\u4e00\u81f4', 'warning')
    return
  }
  changingPassword.value = true
  try {
    await changePassword(passwordForm.oldPassword, passwordForm.newPassword)
    appStore.showToast('\u5bc6\u7801\u4fee\u6539\u6210\u529f', 'success')
    showPasswordPanel.value = false
    resetPasswordForm()
  } catch (error) {
    appStore.showToast((error as Error).message || '\u5bc6\u7801\u4fee\u6539\u5931\u8d25', 'error')
  } finally {
    changingPassword.value = false
  }
}

function openSourceManager() {
  close()
  appStore.showSourceManager = true
}

function openUserManager() {
  close()
  appStore.showUserManager = true
}

function openWebdavManager() {
  close()
  appStore.showWebdavManager = true
}

async function loadAiModelConfig() {
  aiModelLoading.value = true
  try {
    const response = await getAiModelConfig()
    Object.assign(aiModelConfig, cloneAiModelConfig(response.config))
    aiModelIsAdmin.value = response.isAdmin
    aiModelCanUse.value = response.canUseServerModel
    aiModelLoaded.value = true
  } catch (error) {
    appStore.showToast((error as Error).message || '后端模型配置读取失败', 'error')
  } finally {
    aiModelLoading.value = false
  }
}

async function handleSaveAiModelConfig() {
  if (!aiModelIsAdmin.value) return
  aiModelSaving.value = true
  try {
    const response = await saveAiModelConfig(cloneAiModelConfig(aiModelConfig))
    Object.assign(aiModelConfig, cloneAiModelConfig(response.config))
    aiModelIsAdmin.value = response.isAdmin
    aiModelCanUse.value = response.canUseServerModel
    appStore.showToast('后端模型配置已保存', 'success')
  } catch (error) {
    appStore.showToast((error as Error).message || '后端模型配置保存失败', 'error')
  } finally {
    aiModelSaving.value = false
  }
}

function refreshCache() {
  shelfStore.fetchBooks()
  appStore.showToast('\u4e66\u67b6\u5df2\u5237\u65b0', 'success')
  close()
}

function setTheme(t: 'light' | 'dark') {
  appStore.setTheme(t)
}

async function handleInstallPwa() {
  const accepted = await appStore.installPwa()
  if (!accepted) {
    appStore.showToast('\u5f53\u524d\u73af\u5883\u6682\u4e0d\u652f\u6301\u5b89\u88c5\uff0c\u6216\u7528\u6237\u5df2\u53d6\u6d88', 'warning')
    return
  }
  appStore.showToast('\u5b89\u88c5\u8bf7\u6c42\u5df2\u63d0\u4ea4', 'success')
}

function handleApplyUpdate() {
  const ok = appStore.applyPwaUpdate()
  if (!ok) {
    appStore.showToast('\u5f53\u524d\u6ca1\u6709\u53ef\u5e94\u7528\u7684\u65b0\u7248\u672c', 'warning')
  }
}

function handleOpenRelease() {
  const url = appStore.versionUpdate?.releaseUrl
  if (!url) return
  window.open(url, '_blank', 'noopener,noreferrer')
}

async function handleDismissVersionUpdate() {
  await appStore.dismissVersionUpdateReminder()
}

async function handleCheckVersionUpdate() {
  await appStore.checkVersionUpdate(true)
}

function emptyAiModelConfig(): AiServerModelConfig {
  return {
    text: { enabled: false, baseUrl: '', apiKey: '', model: '', path: '/v1/chat/completions', useFullUrl: false },
    image: { enabled: false, baseUrl: '', apiKey: '', model: 'gpt-image-1', path: '/v1/images/generations', useFullUrl: false, imageSize: '1024x1024' },
    speech: { enabled: false, baseUrl: '', apiKey: '', model: 'gpt-4o-mini-tts', path: '/v1/audio/speech', useFullUrl: false, voice: 'alloy', responseFormat: 'mp3' },
  }
}

function cloneAiModelConfig(config: AiServerModelConfig): AiServerModelConfig {
  return JSON.parse(JSON.stringify(config))
}
</script>

<style scoped>
.drawer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  z-index: var(--z-overlay);
  backdrop-filter: blur(4px);
}

.settings-drawer {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  width: min(380px, 90vw);
  background: var(--color-bg-elevated);
  z-index: var(--z-modal);
  display: flex;
  flex-direction: column;
  box-shadow: var(--shadow-xl);
}

.drawer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: calc(var(--space-5) + var(--safe-area-top)) calc(var(--space-6) + var(--safe-area-right)) var(--space-5) var(--space-6);
  border-bottom: 1px solid var(--color-border-light);
  flex-shrink: 0;
}

.drawer-header h2 {
  font-size: var(--text-xl);
  font-weight: 700;
  letter-spacing: -0.01em;
}

.close-btn {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  color: var(--color-text-secondary);
  transition: all var(--duration-fast);
}

.close-btn:hover {
  background: var(--color-bg-hover);
  color: var(--color-text);
}

.close-btn svg {
  width: 20px;
  height: 20px;
}

.drawer-body {
  flex: 1;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  padding: var(--space-4) calc(var(--space-6) + var(--safe-area-right)) calc(var(--space-4) + var(--safe-area-bottom)) var(--space-6);
}

@media (max-width: 768px) {
  .settings-drawer {
    width: min(420px, 92vw);
  }
}

.drawer-section {
  padding: var(--space-4) 0;
  border-bottom: 1px solid var(--color-divider);
}

.drawer-section:last-child {
  border-bottom: none;
}

.section-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: var(--space-3);
}

.user-info-card {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--color-bg-sunken);
  border-radius: var(--radius-md);
}

.user-panel {
  flex: 1;
  display: grid;
  gap: var(--space-3);
}

.user-card-header {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
}

.user-avatar-lg {
  width: 40px;
  height: 40px;
  border-radius: var(--radius-full);
  background: linear-gradient(135deg, var(--color-primary), var(--color-primary-light));
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  font-size: var(--text-lg);
  flex-shrink: 0;
}

.user-detail {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.user-name {
  font-weight: 600;
  font-size: var(--text-sm);
}

.user-role {
  font-size: var(--text-xs);
  color: var(--color-text-tertiary);
}

.password-panel {
  display: grid;
  gap: var(--space-3);
  padding: var(--space-3);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-md);
  background: var(--color-bg);
}

.password-panel.embedded {
  background: var(--color-bg-elevated);
}

.password-field {
  display: grid;
  gap: var(--space-2);
}

.password-field span {
  font-size: var(--text-sm);
  color: var(--color-text-secondary);
}

.password-field input {
  min-height: 40px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  background: var(--color-bg);
  color: inherit;
}

.password-actions {
  display: flex;
  justify-content: flex-start;
}

.btn-group {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.version-actions {
  margin-bottom: var(--space-3);
}

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: 500;
  background: var(--color-bg-sunken);
  color: var(--color-text);
  border: 1px solid var(--color-border-light);
  transition: all var(--duration-fast) var(--ease-out);
}

.action-btn:hover {
  background: var(--color-bg-hover);
  border-color: var(--color-border);
}

.action-btn:active {
  transform: scale(0.97);
}

.action-btn.primary {
  background: var(--color-primary);
  color: white;
  border-color: var(--color-primary);
}

.action-btn.primary:hover {
  background: var(--color-primary-dark);
}

.action-btn.danger {
  color: var(--color-danger);
  border-color: transparent;
  background: transparent;
  padding: var(--space-1) var(--space-2);
}

.action-btn.danger:hover {
  background: rgba(245, 34, 45, 0.08);
}

.action-btn.full {
  width: 100%;
  justify-content: center;
}

.inline-link {
  padding: 0;
  background: transparent;
  border: none;
  color: var(--color-primary);
  font-weight: 500;
  justify-content: flex-start;
}

.inline-link:hover {
  background: transparent;
  border: none;
  color: var(--color-primary-dark);
}

.theme-toggle {
  display: flex;
  gap: var(--space-2);
}

.status-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: var(--space-3);
  background: var(--color-bg-sunken);
  border-radius: var(--radius-md);
  margin-bottom: var(--space-3);
}

.status-card span {
  font-size: var(--text-sm);
  font-weight: 600;
}

.status-card small {
  color: var(--color-text-tertiary);
}

.status-card.accent {
  background: rgba(201, 127, 58, 0.12);
  border: 1px solid rgba(201, 127, 58, 0.18);
}

.status-card.muted {
  opacity: 0.72;
}

.action-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.theme-option {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-4);
  border-radius: var(--radius-md);
  border: 2px solid var(--color-border-light);
  background: var(--color-bg);
  font-size: var(--text-sm);
  font-weight: 500;
  transition: all var(--duration-fast);
  color: var(--color-text-secondary);
}

.theme-option.active {
  border-color: var(--color-primary);
  color: var(--color-primary);
  background: var(--color-primary-bg);
}

.theme-option:hover:not(.active) {
  border-color: var(--color-border);
}
</style>
