import { createApp } from 'vue'
import { createPinia } from 'pinia'
import router from './router'
import App from './App.vue'
import './styles/global.css'
import './styles/v4-console.css'
import { useAppStore } from './stores/app'
import { registerPwa } from './utils/pwa'
import { registerViewportSync } from './utils/viewport'

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(router)
registerViewportSync()
registerPwa(useAppStore(pinia))

app.mount('#app')
