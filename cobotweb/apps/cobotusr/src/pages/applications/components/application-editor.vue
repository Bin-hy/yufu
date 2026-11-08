<script setup lang="ts">
import UiInput from '@/components/ui/input/input.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
export interface AppForm {
    name: string
    description: string
    redirect_uris: string
    grant_types: string[]
    allowed_scopes: string[]
    public_client: boolean
}
const form = defineModel<AppForm>({ required: true })
const props = defineProps<{ disabled: boolean; publicLocked: boolean; scopes: string[]; invalidField?: string }>()
function toggle(key: 'grant_types' | 'allowed_scopes', value: string, checked: boolean) {
    if (props.disabled) return
    form.value[key] = checked
        ? [...new Set([...form.value[key], value])]
        : form.value[key].filter((item) => item !== value)
}
function setPublic(value: boolean) {
    if (props.disabled || props.publicLocked) return
    form.value.public_client = value
    if (value) form.value.grant_types = form.value.grant_types.filter((item) => item !== 'client_credentials')
}
</script>
<template>
    <div class="space-y-4">
        <div class="form-field">
            <label for="application-name">应用名称</label
            ><UiInput
                id="application-name"
                v-model="form.name"
                :disabled="disabled"
                required
                :aria-invalid="invalidField === 'application-name'"
                :aria-describedby="invalidField === 'application-name' ? 'application-validation' : undefined"
            />
        </div>
        <div class="form-field">
            <label for="application-description">描述</label
            ><UiInput id="application-description" v-model="form.description" :disabled="disabled" />
        </div>
        <div class="form-field">
            <label for="application-callback">授权回调地址</label
            ><UiInput
                id="application-callback"
                v-model="form.redirect_uris"
                :disabled="disabled"
                required
                placeholder="https://example.com/callback"
                :aria-invalid="invalidField === 'application-callback'"
                :aria-describedby="
                    invalidField === 'application-callback' ? 'callback-help application-validation' : 'callback-help'
                "
            />
            <p id="callback-help" class="form-help">多个回调地址使用逗号分隔。</p>
        </div>
        <label class="flex items-start gap-3 border border-border rounded-lg p-3"
            ><UiCheckbox
                :model-value="form.public_client"
                :disabled="disabled || publicLocked"
                @update:model-value="setPublic"
            /><span
                ><span class="block font-medium">Public client · PKCE</span
                ><span class="form-help"
                    >适用于浏览器或移动端应用，不分配客户端密钥。{{
                        publicLocked ? '现有 Public client 不支持转回保密客户端。' : ''
                    }}</span
                ></span
            ></label
        >
        <fieldset :disabled="disabled" class="space-y-2">
            <legend class="font-medium text-xs mb-2">授权类型</legend>
            <label
                v-for="grant in ['authorization_code', 'refresh_token', 'client_credentials']"
                :key="grant"
                class="flex items-center gap-2"
                ><UiCheckbox
                    :id="grant === 'authorization_code' ? 'application-grants' : undefined"
                    :aria-invalid="invalidField === 'application-grants'"
                    :aria-describedby="invalidField === 'application-grants' ? 'application-validation' : undefined"
                    :model-value="form.grant_types.includes(grant)"
                    :disabled="disabled || (form.public_client && grant === 'client_credentials')"
                    @update:model-value="(value) => toggle('grant_types', grant, value)"
                /><span class="text-xs">{{ grant }}</span></label
            >
        </fieldset>
        <fieldset :disabled="disabled" class="space-y-2">
            <legend class="font-medium text-xs mb-2">可请求的 Scope</legend>
            <div class="flex flex-wrap gap-x-5 gap-y-3">
                <label v-for="scope in scopes" :key="scope" class="flex gap-2 items-center"
                    ><UiCheckbox
                        :model-value="form.allowed_scopes.includes(scope)"
                        :disabled="disabled"
                        @update:model-value="(value) => toggle('allowed_scopes', scope, value)"
                    /><span class="text-xs">{{ scope }}</span></label
                >
            </div>
        </fieldset>
    </div>
</template>
