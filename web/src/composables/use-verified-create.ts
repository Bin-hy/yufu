import { ref } from 'vue'
import { sessionEpoch } from '@/workspace/session'

export function createVerifiedCreate<T>(write: () => Promise<T>, find: () => Promise<T | null>) {
    const uncertain = ref(false)
    async function run(): Promise<T> {
        const epoch = sessionEpoch.value
        if (!uncertain.value) {
            try {
                return await write()
            } catch (error) {
                const failure = error as { response?: unknown; code?: string }
                if (
                    failure.response ||
                    !['ECONNABORTED', 'ETIMEDOUT', 'ERR_NETWORK'].includes(failure.code ?? '')
                )
                    throw error
                uncertain.value = true
            }
        }
        if (epoch !== sessionEpoch.value) throw Error('会话已改变')
        let result: T | null = null
        try {
            result = await find()
        } catch {
            /* 保持结果未知，不重发创建 */
        }
        if (epoch !== sessionEpoch.value) throw Error('会话已改变')
        if (result !== null) {
            uncertain.value = false
            return result
        }
        throw Error(
            '创建结果未知，无法确认服务端是否已完成。为避免重复创建，请核对现有记录；重试仅重新核对，不会再次提交。',
        )
    }
    return { uncertain, run }
}
