export interface PickerOption {
    value: string | number
    label: string
    secondary?: string
    disabled?: boolean
}

export type PickerLoader = (query: string, signal: AbortSignal) => Promise<PickerOption[]>
