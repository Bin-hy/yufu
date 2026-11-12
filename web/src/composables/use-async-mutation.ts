import { ref } from 'vue'
import { sessionEpoch } from '@/workspace/session'

const codeMessages: Record<string, string> = {
    deadline_exceeded: '认证服务响应超时，请稍后重试',
    dependency_unavailable: '认证服务暂时不可用，请稍后重试',
    capacity_exceeded: '当前请求量过大，请稍后重试',
    commit_uncertain: '提交结果尚未确认，请刷新后核对',
}

export function errorMessage(error: unknown): string {
    const e = error as { response?: { data?: unknown }; message?: string }
    const data = e?.response?.data
    if (typeof data === 'string' && data.trim()) return data
    if (data && typeof data === 'object') {
        if ('message' in data && typeof data.message === 'string' && data.message.trim())
            return data.message
        if ('code' in data && typeof data.code === 'string' && codeMessages[data.code])
            return codeMessages[data.code]!
    }
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
