import { defineStore } from 'pinia'
import { ref } from 'vue'
import { onSessionReset } from '@/workspace/session'

export type ToastType = 'success' | 'error' | 'info' | 'warning'

export interface ToastItem {
    id: number
    type: ToastType
    message: string
    durationMs?: number
}

export const useToastStore = defineStore('toast', () => {
    const toasts = ref<ToastItem[]>([])
    let uid = 0
    const timers = new Map<number, number>()
    const clear = () => {
        timers.forEach((timer) => window.clearTimeout(timer))
        timers.clear()
        toasts.value = []
    }
    onSessionReset(clear)

    const remove = (id: number) => {
        window.clearTimeout(timers.get(id))
        timers.delete(id)
        toasts.value = toasts.value.filter((t) => t.id !== id)
    }

    const push = (type: ToastType, message: string, durationMs = 3000) => {
        const id = ++uid
        toasts.value.push({ id, type, message, durationMs })
        if (durationMs > 0) {
            timers.set(
                id,
                window.setTimeout(() => remove(id), durationMs),
            )
        }
        return id
    }

    const success = (message: string, durationMs?: number) => push('success', message, durationMs)
    const error = (message: string, durationMs?: number) => push('error', message, durationMs)
    const info = (message: string, durationMs?: number) => push('info', message, durationMs)
    const warning = (message: string, durationMs?: number) => push('warning', message, durationMs)

    return { toasts, push, remove, clear, success, error, info, warning }
})
