<template>
  <div id="app">
    <AppTopBar v-if="showHeader" />
    <main class="app-main" :class="{ 'with-bottom-nav': showBottomNav, 'without-header': !showHeader }">
      <router-view />
    </main>
    <AppBottomNav v-if="showBottomNav" />
    <SettingsDrawer v-model="appStore.showSettingsDrawer" />
    <LoginModal v-model="appStore.showLoginModal" />
    <SourceManager v-model="appStore.showSourceManager" />
    <UserManager v-model="appStore.showUserManager" />
    <WebdavManager v-model="appStore.showWebdavManager" />

    <!-- Toast notifications -->
    <div class="toast-container">
      <TransitionGroup name="slide-up">
        <div
          v-for="toast in appStore.toasts"
          :key="toast.id"
          class="toast"
          :class="toast.type"
        >
          {{ toast.message }}
        </div>
      </TransitionGroup>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from './stores/app'
import AppTopBar from './components/AppTopBar.vue'
import AppBottomNav from './components/AppBottomNav.vue'
import SettingsDrawer from './components/SettingsDrawer.vue'
import LoginModal from './components/LoginModal.vue'
import SourceManager from './components/SourceManager.vue'
import UserManager from './components/UserManager.vue'
import WebdavManager from './components/WebdavManager.vue'

const route = useRoute()
const appStore = useAppStore()

const immersiveRouteNames = new Set(['reader', 'ai-book'])
const isImmersiveRoute = computed(() => immersiveRouteNames.has(String(route.name || '')))
const showHeader = computed(() => !isImmersiveRoute.value)
const showBottomNav = computed(() => !isImmersiveRoute.value)

onMounted(() => {
  appStore.fetchUserInfo()
})

async function handleNeedLogin() {
  await appStore.fetchUserInfo()
  if (!appStore.isLoggedIn) {
    appStore.showLoginModal = true
  }
}

onMounted(() => {
  window.addEventListener('need-login', handleNeedLogin)
})

onUnmounted(() => {
  window.removeEventListener('need-login', handleNeedLogin)
})
</script>

<style>
html,
body {
  height: var(--app-height, 100dvh);
  overflow: hidden;
}

#app {
  height: var(--app-height, 100dvh);
  overflow: hidden;
}

.app-main {
  height: calc(var(--app-height, 100dvh) - var(--header-height) - var(--safe-area-top));
  min-height: 0;
  overflow: hidden;
}

.app-main.without-header {
  height: var(--app-height, 100dvh);
}

.app-main.with-bottom-nav {
  padding-bottom: calc(104px + var(--safe-area-bottom));
  height: calc(var(--app-height, 100dvh) - var(--header-height) - var(--safe-area-top));
}
</style>
