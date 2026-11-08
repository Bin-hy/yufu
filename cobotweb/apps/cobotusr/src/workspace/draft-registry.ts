import { computed, ref } from 'vue'
import type { DraftEntry } from '@/types/workspace'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import { onSessionReset } from './session'
const entries = new Map<string, DraftEntry>()
const revision = ref(0)
export function registerDraft(entry: DraftEntry) {
    entries.set(entry.id, entry)
    revision.value++
    return () => {
        entries.delete(entry.id)
        revision.value++
    }
}
export const hasUnsavedWork = computed(() => {
    void revision.value
    return [...entries.values()].some((entry) => entry.dirty() || entry.submitting())
})
export async function checkDrafts(keys: string[], closing = false): Promise<boolean> {
    const target = [...entries.values()].filter((entry) => keys.includes(entry.tabKey))
    if (target.some((entry) => entry.submitting())) {
        emitToast({ type: 'info', message: '正在提交，请等待操作完成' })
        return false
    }
    const dirty = target.filter((entry) => entry.dirty())
    if (!dirty.length) return true
    return showModal({
        title: closing ? '放弃未保存的修改？' : '保留草稿并切换页面？',
        content:
            dirty.map((entry) => entry.title).join('、') +
            (closing
                ? '的修改尚未保存，关闭后将丢失。'
                : '的修改尚未保存，切换后可以返回继续编辑。'),
        confirmText: closing ? '放弃修改' : '保留并切换',
        cancelText: '继续编辑',
        destructive: closing,
    })
}
export function discardDrafts(keys: string[]) {
    for (const entry of entries.values()) if (keys.includes(entry.tabKey)) entry.discard()
}
export function resetDrafts() {
    entries.clear()
    revision.value++
}
onSessionReset(resetDrafts)
