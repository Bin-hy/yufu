import { ref, computed, watch, type WatchStopHandle } from 'vue'
export type ThemePreference = 'light' | 'dark' | 'system'
const key = 'yufu:theme',
    preference = ref<ThemePreference>('system'),
    systemDark = ref(false)
let dispose: (() => void) | null = null
export function disposeTheme() {
    dispose?.()
    dispose = null
}
export function initializeTheme() {
    if (dispose) return
    try {
        const saved = localStorage.getItem(key)
        preference.value = saved === 'light' || saved === 'dark' ? saved : 'system'
    } catch {
        preference.value = 'system'
    }
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    systemDark.value = media.matches
    const listener = (event: MediaQueryListEvent) => {
        systemDark.value = event.matches
    }
    media.addEventListener('change', listener)
    const stop: WatchStopHandle = watch(
        [preference, systemDark],
        () => {
            const dark =
                preference.value === 'dark' || (preference.value === 'system' && systemDark.value)
            document.documentElement.classList.toggle('dark', dark)
            document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
            document
                .querySelector('meta[name="theme-color"]')
                ?.setAttribute('content', dark ? '#14232a' : '#f5f8f9')
            try {
                localStorage.setItem(key, preference.value)
            } catch {
                /* 内存继续 */
            }
        },
        { immediate: true },
    )
    dispose = () => {
        stop()
        media.removeEventListener('change', listener)
    }
}
export function useTheme() {
    initializeTheme()
    return {
        preference,
        resolved: computed(() =>
            preference.value === 'system'
                ? systemDark.value
                    ? 'dark'
                    : 'light'
                : preference.value,
        ),
        setTheme: (value: ThemePreference) => {
            preference.value = value
        },
    }
}
if (import.meta.hot) import.meta.hot.dispose(disposeTheme)
