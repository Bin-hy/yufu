import './style.css'
import { createApp } from 'vue'
import App from './app.vue'
import { pinia } from '@/pinia'
import { router } from '@/router'
import { RegistryRouterGuard } from '@/router/guard'
import { canEnter } from '@/router/guard'
import { useUserStore } from '@/pinia/stores/user'
import { useTabsStore } from '@/pinia/stores/tabs'
import { sessionOwner, onSessionReset, setAuthExpiredHandler } from '@/workspace/session'
import { loadWorkspace } from '@/storage/workspace'
import { resolveTabIdentity } from '@/workspace/tab-identity'
import { initializeTheme } from '@/composables/use-theme'
initializeTheme()
async function start() {
    const app = createApp(App)
    app.use(pinia)
    const user = useUserStore(pinia),
        tabs = useTabsStore(pinia)
    onSessionReset(() => tabs.reset())
    let booted = false
    setAuthExpiredHandler(() => {
        if (booted)
            void router.replace(
                router.currentRoute.value.path.startsWith('/oidc/')
                    ? { name: 'oidc-consent', query: { flow_id: router.currentRoute.value.query.flow_id } }
                    : { name: 'login' },
            )
    })
    user.initUserInfo()
    await user.ensureSession()
    tabs.initHome()
    const snapshot = loadWorkspace(router, sessionOwner.value)
    if (snapshot)
        for (const tab of snapshot.tabs) {
            const route = router.resolve(tab.fullPath)
            if (!canEnter(route.meta, user.centerSession)) continue
            tabs.sync(route, false)
            const restored = tabs.visited.find((item) => item.key === resolveTabIdentity(route))
            if (restored) restored.scrollTop = tab.scrollTop
        }
    RegistryRouterGuard(router)
    app.use(router)
    await router.isReady()
    booted = true
    app.mount('#app')
}
void start()
