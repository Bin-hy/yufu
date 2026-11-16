import { ref } from 'vue'

/** Shared 30s clock so relative timestamps update without per-cell timers. */
export const nowSeconds = ref(Math.floor(Date.now() / 1000))
let timer: ReturnType<typeof setInterval> | undefined
let subscribers = 0
export function subscribeClock() {
    subscribers++
    if (subscribers === 1 && timer === undefined)
        timer = setInterval(() => {
            nowSeconds.value = Math.floor(Date.now() / 1000)
        }, 30_000)
    return () => {
        subscribers--
        if (subscribers <= 0 && timer !== undefined) {
            clearInterval(timer)
            timer = undefined
            subscribers = 0
        }
    }
}

export function formatAbsolute(epochSeconds: number) {
    return new Date(epochSeconds * 1000).toLocaleString('zh-CN')
}

/** 过去时间的相对描述；超过 7 天退回绝对时间。 */
export function formatRelative(epochSeconds: number, now: number): string {
    const delta = now - epochSeconds
    if (delta < 0) return formatUntil(epochSeconds, now)
    if (delta < 45) return '刚刚'
    if (delta < 3600) return `${Math.max(1, Math.round(delta / 60))} 分钟前`
    if (delta < 86_400) return `${Math.round(delta / 3600)} 小时前`
    if (delta < 7 * 86_400) return `${Math.round(delta / 86_400)} 天前`
    return formatAbsolute(epochSeconds)
}

/** 未来时间的剩余描述，用于一次性链接的有效期倒计时。 */
export function formatUntil(epochSeconds: number, now: number): string {
    const delta = epochSeconds - now
    if (delta <= 0) return '已到期'
    if (delta < 60) return `${delta} 秒后到期`
    if (delta < 3600) return `${Math.ceil(delta / 60)} 分钟后到期`
    if (delta < 86_400) return `约 ${Math.round(delta / 3600)} 小时后到期`
    return `约 ${Math.round(delta / 86_400)} 天后到期`
}
