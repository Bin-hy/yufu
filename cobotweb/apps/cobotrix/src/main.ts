import { createApp } from 'vue'
import App from './app.vue'
import { router, registerGuard } from './router'
import './style.css'

registerGuard(router)

createApp(App).use(router).mount('#app')
