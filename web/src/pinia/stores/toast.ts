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
    const recent = new Map<string, { id: number; at: number }>()
    const clear = () => {
        timers.forEach((timer) => window.clearTimeout(timer))
        timers.clear()
        recent.clear()
        toasts.value = []
    }
    onSessionReset(clear)
    const remove = (id: number) => {
        window.clearTimeout(timers.get(id))
        timers.delete(id)
        for (const [key, value] of recent) if (value.id === id) recent.delete(key)
        toasts.value = toasts.value.filter((t) => t.id !== id)
    }
    const push = (type: ToastType, message: string, durationMs = type === 'error' || type === 'warning' ? 6000 : 3000, dedupeKey?: string) => {
        const key = `${type}:${dedupeKey ?? message.replace(/（请求标识 [a-zA-Z0-9._:-]+）$/, '')}`
        const now = Date.now()
        for (const [entry, value] of recent) if (now - value.at >= 3000) recent.delete(entry)
        if (type === 'error' || type === 'warning') {
            const existing = recent.get(key)
            if (existing) return existing.id
        }
        const id = ++uid
        if (toasts.value.length >= 4) remove(toasts.value[0]!.id)
        toasts.value.push({ id, type, message, durationMs })
        if (type === 'error' || type === 'warning') recent.set(key, { id, at: now })
        if (durationMs > 0) timers.set(id, window.setTimeout(() => remove(id), durationMs))
        return id
    }
    const success = (message: string, durationMs?: number) => push('success', message, durationMs)
    const error = (message: string, durationMs?: number) => push('error', message, durationMs)
    const info = (message: string, durationMs?: number) => push('info', message, durationMs)
    const warning = (message: string, durationMs?: number) => push('warning', message, durationMs)
    return { toasts, push, remove, clear, success, error, info, warning }
})
