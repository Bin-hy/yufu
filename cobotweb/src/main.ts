import './style.css'
import { createApp } from 'vue'
import App from './App.vue'
// core
import { pinia } from '@/pinia'
import { router } from '@/router'
import { useUserStore } from '@/pinia/stores/user'

const app = createApp(App)

app.use(pinia)
app.use(router)

// 初始化用户信息（从本地存储恢复）
const userStore = useUserStore()
userStore.initUserInfo()

app.mount('#app')
