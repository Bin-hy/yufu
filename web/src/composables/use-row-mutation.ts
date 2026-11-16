import { ref } from 'vue'
import { useAsyncMutation } from './use-async-mutation'
import type { ModalOptions } from '@/utils/modal'

/**
 * useAsyncMutation plus row attribution: writes stay serialized and keep the
 * uncertain-outcome guard, but only the row that started the write shows its
 * spinner instead of every action button in the table.
 */
export function useRowMutation() {
    const mutation = useAsyncMutation()
    const activeRow = ref<string | number | null>(null)
    function rowPending(id: string | number) {
        return mutation.pending.value && activeRow.value === id
    }
    async function runRow<T>(
        id: string | number,
        loader: () => Promise<T>,
        success?: (data: T) => void | Promise<void>,
    ): Promise<boolean> {
        activeRow.value = id
        try {
            return await mutation.run(loader, success)
        } finally {
            activeRow.value = null
        }
    }
    async function confirmRow(
        id: string | number,
        options: Omit<ModalOptions, 'confirmCallback'>,
        loader: () => Promise<unknown>,
    ): Promise<boolean> {
        activeRow.value = id
        try {
            return await mutation.confirm(options, loader)
        } finally {
            activeRow.value = null
        }
    }
    return { ...mutation, activeRow, rowPending, runRow, confirmRow }
}
