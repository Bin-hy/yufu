<script setup lang="ts">
import { computed, watch } from 'vue'
import type { EnvironmentConfig } from '../api/type'
import type { EnvironmentConfigField } from '../api/validation'
import UiInput from '@/components/ui/input/input.vue'

const props = defineProps<{
    prefix: string
    environment: 'development' | 'production'
    administrator?: boolean
    disabled?: boolean
    activation?: boolean
    errors?: Partial<Record<EnvironmentConfigField, string>>
}>()
const config = defineModel<EnvironmentConfig>({ required: true })

watch(() => config.value.client_kind, kind => {
    if (kind === 'server_web') config.value.browser_origins = []
})

const textList = (key: 'redirect_uris' | 'post_logout_redirect_uris' | 'browser_origins') => computed({
    get: () => config.value[key].join('\n'),
    set: (value: string) => {
        config.value[key] = value.split('\n').map(value => value.trim()).filter(Boolean)
    },
})
const redirects = textList('redirect_uris')
const logouts = textList('post_logout_redirect_uris')
const origins = textList('browser_origins')
const backchannel = computed({
    get: () => config.value.backchannel_logout_uri ?? '',
    set: (value: string) => {
        config.value.backchannel_logout_uri = value.trim() || null
    },
})
const redirectsRequired = computed(() => props.environment === 'development' || !!props.activation)
const uriHint = computed(() => props.environment === 'development'
    ? '支持 https；本机调试可使用 http://localhost 或 http://127.0.0.1。'
    : props.activation
        ? '必须使用 https；http 地址（包括 127.0.0.1）会被拒绝。'
        : '可先留空，生产配置经审批后生效；填写时必须使用 https，http 地址（包括 127.0.0.1）会被拒绝。')
const errorFor = (field: EnvironmentConfigField) => props.errors?.[field]
const errorId = (field: EnvironmentConfigField) => `${props.prefix}-${field}-error`
const isInvalid = (field: EnvironmentConfigField) => !!errorFor(field)
</script>

<template>
    <fieldset class="space-y-5" :disabled="disabled">
        <legend class="sr-only">{{ props.environment === 'development' ? '开发环境' : '生产环境' }}接入配置</legend>
        <div class="form-field">
            <label :for="props.prefix + '-kind'">应用类型</label>
            <select
                :id="props.prefix + '-kind'"
                v-model="config.client_kind"
                class="ui-select"
                :name="props.prefix + '-kind'"
                :aria-invalid="isInvalid('client_kind')"
            >
                <option value="server_web">服务端 Web（服务端保存客户端密钥）</option>
                <option value="browser_spa">浏览器 SPA（S256 PKCE，无客户端密钥）</option>
            </select>
            <p v-if="errorFor('client_kind')" :id="errorId('client_kind')" class="field-error" role="alert">{{ errorFor('client_kind') }}</p>
        </div>
        <div class="form-field">
            <label :for="props.prefix + '-redirects'">登录回调地址（每行一个，精确匹配）</label>
            <textarea
                :id="props.prefix + '-redirects'"
                v-model="redirects"
                class="ui-input min-h-20"
                :class="{ 'has-error': isInvalid('redirect_uris') }"
                :name="props.prefix + '-redirects'"
                :required="redirectsRequired"
                autocomplete="off"
                spellcheck="false"
                :placeholder="props.environment === 'development' ? 'http://127.0.0.1:5173/callback…' : 'https://app.example/callback…'"
                :aria-invalid="isInvalid('redirect_uris')"
                :aria-describedby="isInvalid('redirect_uris') ? errorId('redirect_uris') : undefined"
            />
            <p v-if="errorFor('redirect_uris')" :id="errorId('redirect_uris')" class="field-error" role="alert">{{ errorFor('redirect_uris') }}</p>
            <p class="form-help">{{ uriHint }}</p>
        </div>
        <div class="form-field">
            <label :for="props.prefix + '-logout-redirects'">退出后回调地址（可选，每行一个）</label>
            <textarea
                :id="props.prefix + '-logout-redirects'"
                v-model="logouts"
                class="ui-input min-h-16"
                :class="{ 'has-error': isInvalid('post_logout_redirect_uris') }"
                :name="props.prefix + '-logout-redirects'"
                autocomplete="off"
                spellcheck="false"
                :placeholder="props.environment === 'development' ? 'http://127.0.0.1:5173/logged-out…' : 'https://app.example/logged-out…'"
                :aria-invalid="isInvalid('post_logout_redirect_uris')"
                :aria-describedby="isInvalid('post_logout_redirect_uris') ? errorId('post_logout_redirect_uris') : undefined"
            />
            <p v-if="errorFor('post_logout_redirect_uris')" :id="errorId('post_logout_redirect_uris')" class="field-error" role="alert">{{ errorFor('post_logout_redirect_uris') }}</p>
            <p class="form-help">{{ uriHint }}</p>
        </div>
        <div v-if="config.client_kind === 'browser_spa'" class="form-field">
            <label :for="props.prefix + '-origins'">浏览器来源（含协议与端口，每行一个）</label>
            <textarea
                :id="props.prefix + '-origins'"
                v-model="origins"
                class="ui-input min-h-16"
                :class="{ 'has-error': isInvalid('browser_origins') }"
                :name="props.prefix + '-origins'"
                :required="config.redirect_uris.length > 0 || redirectsRequired"
                autocomplete="off"
                spellcheck="false"
                :placeholder="props.environment === 'development' ? 'http://127.0.0.1:5173…' : 'https://app.example…'"
                :aria-invalid="isInvalid('browser_origins')"
                :aria-describedby="isInvalid('browser_origins') ? errorId('browser_origins') : undefined"
            />
            <p v-if="errorFor('browser_origins')" :id="errorId('browser_origins')" class="field-error" role="alert">{{ errorFor('browser_origins') }}</p>
            <p class="form-help">须与登录回调地址的来源完全一致（协议、主机与端口；http 80 / https 443 默认端口需省略）。</p>
        </div>
        <div class="form-field">
            <label :for="props.prefix + '-audience'">保护 API 的访问 Audience</label>
            <UiInput
                :id="props.prefix + '-audience'"
                v-model="config.access_audience"
                :class="isInvalid('access_audience') ? 'has-error' : ''"
                :name="props.prefix + '-audience'"
                required
                autocomplete="off"
                spellcheck="false"
                placeholder="urn:my-app:api…"
                :aria-invalid="isInvalid('access_audience')"
                :aria-describedby="isInvalid('access_audience') ? errorId('access_audience') : undefined"
            />
            <p v-if="errorFor('access_audience')" :id="errorId('access_audience')" class="field-error" role="alert">{{ errorFor('access_audience') }}</p>
        </div>
        <div class="form-field">
            <label :for="props.prefix + '-scopes'">允许的账户资料范围</label>
            <select
                :id="props.prefix + '-scopes'"
                v-model="config.identity_scopes"
                class="ui-select"
                :name="props.prefix + '-scopes'"
                :aria-invalid="isInvalid('identity_scopes')"
            >
                <option value="openid">账户身份</option>
                <option value="openid profile">身份、显示名称</option>
                <option value="openid email">身份、邮箱</option>
                <option value="openid profile email">身份、显示名称、邮箱</option>
            </select>
            <p v-if="errorFor('identity_scopes')" :id="errorId('identity_scopes')" class="field-error" role="alert">{{ errorFor('identity_scopes') }}</p>
        </div>
        <div v-if="administrator" class="form-field">
            <label :for="props.prefix + '-consent'">资料同意策略</label>
            <select
                :id="props.prefix + '-consent'"
                v-model="config.consent_policy"
                class="ui-select"
                :name="props.prefix + '-consent'"
                :aria-invalid="isInvalid('consent_policy')"
            >
                <option value="user_consent">用户同意</option>
                <option value="enterprise_preconsent">企业预先同意</option>
            </select>
            <p v-if="errorFor('consent_policy')" :id="errorId('consent_policy')" class="field-error" role="alert">{{ errorFor('consent_policy') }}</p>
        </div>
        <div class="form-field">
            <label :for="props.prefix + '-logout'">退出能力</label>
            <select
                :id="props.prefix + '-logout'"
                v-model="config.logout_capability"
                class="ui-select"
                :name="props.prefix + '-logout'"
                :aria-invalid="isInvalid('logout_capability')"
            >
                <option value="backchannel_and_online">后端退出通知与在线校验</option>
                <option value="online">在线校验（不接收退出通知）</option>
                <option value="local_only">仅应用本地退出</option>
            </select>
            <p v-if="errorFor('logout_capability')" :id="errorId('logout_capability')" class="field-error" role="alert">{{ errorFor('logout_capability') }}</p>
        </div>
        <div v-if="config.logout_capability === 'backchannel_and_online'" class="form-field">
            <label :for="props.prefix + '-backchannel'">后端退出通知 HTTPS 地址</label>
            <UiInput
                :id="props.prefix + '-backchannel'"
                v-model="backchannel"
                type="url"
                :class="isInvalid('backchannel_logout_uri') ? 'has-error' : ''"
                :name="props.prefix + '-backchannel'"
                required
                autocomplete="off"
                spellcheck="false"
                placeholder="https://app.example/logout/backchannel…"
                :aria-invalid="isInvalid('backchannel_logout_uri')"
                :aria-describedby="isInvalid('backchannel_logout_uri') ? errorId('backchannel_logout_uri') : undefined"
            />
            <p v-if="errorFor('backchannel_logout_uri')" :id="errorId('backchannel_logout_uri')" class="field-error" role="alert">{{ errorFor('backchannel_logout_uri') }}</p>
        </div>
    </fieldset>
</template>
