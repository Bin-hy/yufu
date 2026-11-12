<script setup lang="ts">
const props = withDefaults(
    defineProps<{
        id?: string
        label: string
        hint?: string
        error?: string | null
        errorId?: string
        required?: boolean
        optional?: boolean
    }>(),
    { required: false, optional: false },
)
const errorElementId = () => props.errorId || (props.id ? props.id + '-error' : undefined)
</script>
<template>
    <div class="form-field" :class="{ 'is-invalid': !!error, 'is-required': required }">
        <div class="field-label-row">
            <label :for="id">{{ label }}</label>
            <span v-if="optional" class="field-optional">可选</span>
            <slot name="aside" />
        </div>
        <slot />
        <p v-if="error" :id="errorElementId()" class="field-error" role="alert">
            {{ error }}
        </p>
        <p v-else-if="hint" class="form-help">{{ hint }}</p>
        <slot name="help" />
    </div>
</template>
