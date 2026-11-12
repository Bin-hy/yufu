<script setup lang="ts">
import { computed, ref, useAttrs } from 'vue'
import { Eye, EyeOff, X } from '@lucide/vue'
import { cn } from '@/lib/utils'

defineOptions({ inheritAttrs: false })

const props = withDefaults(
    defineProps<{
        modelValue?: string | number | null
        type?: string
        placeholder?: string
        class?: string
        disabled?: boolean
        invalid?: boolean
        clearable?: boolean
        modelModifiers?: { number?: boolean; trim?: boolean }
    }>(),
    { type: 'text', placeholder: '', invalid: false, clearable: false },
)
const emit = defineEmits<{ 'update:modelValue': [value: string | number] }>()
// Attrs are forwarded verbatim; Vue types them as Data, which is too broad for the input bindings.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const attrs = useAttrs() as Record<string, any>
const revealed = ref(false)
const hasAction = computed(() => props.type === 'password' || props.clearable)
const inputType = computed(() =>
    props.type === 'password' && revealed.value ? 'text' : props.type,
)
const showClear = computed(
    () =>
        props.clearable &&
        props.type !== 'password' &&
        props.modelValue !== null &&
        props.modelValue !== undefined &&
        props.modelValue !== '',
)
const ariaInvalid = computed(() => props.invalid || attrs['aria-invalid'] || undefined)

function update(event: Event) {
    const target = event.target as HTMLInputElement
    const value = props.modelModifiers?.trim ? target.value.trim() : target.value
    const number = Number.parseFloat(value)
    emit(
        'update:modelValue',
        props.modelModifiers?.number && !Number.isNaN(number) ? number : value,
    )
}

function clear() {
    emit('update:modelValue', '')
}
</script>

<template>
    <div
        v-if="hasAction"
        class="ui-input-shell"
        :class="{ 'is-invalid': invalid, 'is-disabled': disabled }"
    >
        <slot name="leading" />
        <input
            v-bind="$attrs"
            :type="inputType"
            :value="modelValue ?? ''"
            :placeholder="placeholder"
            :disabled="disabled"
            :aria-invalid="ariaInvalid"
            :class="cn('ui-input', 'ui-input-with-action', $props.class)"
            @input="update"
        />
        <button
            v-if="type === 'password'"
            type="button"
            class="ui-input-action"
            :aria-label="revealed ? '隐藏密码' : '显示密码'"
            :title="revealed ? '隐藏密码' : '显示密码'"
            :disabled="disabled"
            @click="revealed = !revealed"
        >
            <EyeOff v-if="revealed" :size="15" aria-hidden="true" />
            <Eye v-else :size="15" aria-hidden="true" />
        </button>
        <button
            v-else-if="showClear"
            type="button"
            class="ui-input-action"
            aria-label="清除输入"
            title="清除输入"
            :disabled="disabled"
            @click="clear"
        >
            <X :size="14" aria-hidden="true" />
        </button>
    </div>
    <input
        v-else
        v-bind="$attrs"
        :type="type"
        :value="modelValue ?? ''"
        :placeholder="placeholder"
        :disabled="disabled"
        :aria-invalid="ariaInvalid"
        :class="cn('ui-input', invalid && 'has-error', $props.class)"
        @input="update"
    />
</template>
