import { ref } from 'vue'
import { sessionEpoch } from '@/workspace/session'
export function errorMessage(error: unknown): string {
    const e = error as { response?: { data?: unknown }; message?: string }
    if (typeof e?.response?.data === 'string') return e.response.data
    if (e?.response?.data && typeof e.response.data === 'object' && 'message' in e.response.data)
        return String(e.response.data.message)
    return e?.message || '操作失败，请稍后重试'
}
export function useAsyncMutation() {
    const pending = ref(false)
    const error = ref<string | null>(null)
    async function run<T>(
        loader: () => Promise<T>,
        success?: (data: T) => void | Promise<void>,
    ): Promise<boolean> {
        if (pending.value) return false
        const epoch = sessionEpoch.value
        pending.value = true
        error.value = null
        try {
            const result = await loader()
            if (epoch !== sessionEpoch.value) return false
            await success?.(result)
            return true
        } catch (e) {
            if (epoch === sessionEpoch.value) error.value = errorMessage(e)
            return false
        } finally {
            if (epoch === sessionEpoch.value) pending.value = false
        }
    }
    return { pending, error, run }
}
