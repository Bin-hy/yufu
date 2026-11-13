<script setup lang="ts">
import { computed, watch } from 'vue'
import type { EnvironmentConfig } from '../api/type'
import type { EnvironmentConfigField } from '../api/validation'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'

const props = defineProps<{
    prefix: string
    environment: 'development' | 'production'
    administrator?: boolean
    disabled?: boolean
    activation?: boolean
    errors?: Partial<Record<EnvironmentConfigField, string>>
}>()
const config = defineModel<EnvironmentConfig>({ required: true })

watch(
    () => config.value.client_kind,
    (kind) => {
        if (kind === 'server_web') config.value.browser_origins = []
    },
)

const textList = (key: 'redirect_uris' | 'post_logout_redirect_uris' | 'browser_origins') =>
    computed({
        get: () => config.value[key].join('\n'),
        set: (value: string) => {
            config.value[key] = value
                .split('\n')
                .map((value) => value.trim())
                .filter(Boolean)
        },
    })
const redirects = textList('redirect_uris')
const logouts = textList('post_logout_redirect_uris')
const origins = textList('browser_origins')
const optionalText = (key: 'launch_url' | 'launch_description') => computed({
    get: () => config.value[key] ?? '',
    set: (value: string) => { config.value[key] = value || null },
})
const launchUrl = optionalText('launch_url')
const launchDescription = optionalText('launch_description')
const backchannel = computed({
    get: () => config.value.backchannel_logout_uri ?? '',
    set: (value: string) => {
        config.value.backchannel_logout_uri = value.trim() || null
    },
})
const redirectsRequired = computed(
    () => props.environment === 'development' || !!props.activation,
)
const uriHint = computed(() =>
    props.environment === 'development'
        ? '支持 https；本机调试可使用 http://localhost 或 http://127.0.0.1。'
        : props.activation
          ? '必须使用 https；http 地址（包括 127.0.0.1）会被拒绝。'
          : '可先留空，生产配置经审批后生效；填写时必须使用 https，http 地址（包括 127.0.0.1）会被拒绝。',
)
const errorFor = (field: EnvironmentConfigField) => props.errors?.[field]
const errorId = (field: EnvironmentConfigField) => `${props.prefix}-${field}-error`
const isInvalid = (field: EnvironmentConfigField) => !!errorFor(field)
const count = (value: string) => value.split('\n').filter(Boolean).length
</script>

<template>
    <fieldset class="space-y-5" :disabled="disabled">
        <legend class="sr-only">
            {{ props.environment === 'development' ? '开发环境' : '生产环境' }}接入配置
        </legend>

        <UiFormField
            :id="props.prefix + '-launch-url'"
            label="应用启动地址"
            optional
            :hint="'填写应用首页；' + uriHint + ' 留空时成员应用卡片不提供启动入口。生产变更经审批后生效。'"
            :error="errorFor('launch_url')"
            :error-id="errorId('launch_url')"
        >
            <UiInput
                :id="props.prefix + '-launch-url'"
                v-model="launchUrl"
                :name="props.prefix + '-launch-url'"
                autocomplete="off"
                spellcheck="false"
                placeholder="https://app.example/"
                :invalid="isInvalid('launch_url')"
                :disabled="disabled"
            />
        </UiFormField>
        <UiFormField
            :id="props.prefix + '-launch-description'"
            label="启动说明"
            optional
            hint="最多 1000 字符，向成员说明应用用途。"
            :error="errorFor('launch_description')"
            :error-id="errorId('launch_description')"
        >
            <UiInput
                :id="props.prefix + '-launch-description'"
                v-model="launchDescription"
                :name="props.prefix + '-launch-description'"
                :invalid="isInvalid('launch_description')"
                :disabled="disabled"
            />
        </UiFormField>
        <div class="grid gap-4 md:grid-cols-2">
            <UiFormField
                :id="props.prefix + '-kind'"
                label="应用类型"
                required
                :error="errorFor('client_kind')"
                :error-id="errorId('client_kind')"
            >
                <UiSelect
                    :id="props.prefix + '-kind'"
                    v-model="config.client_kind"
                    :name="props.prefix + '-kind'"
                    :invalid="isInvalid('client_kind')"
                    :disabled="disabled"
                >
                    <option value="server_web">服务端 Web（服务端保存客户端密钥）</option>
                    <option value="browser_spa">浏览器 SPA（S256 PKCE，无客户端密钥）</option>
                </UiSelect>
            </UiFormField>
            <UiFormField
                :id="props.prefix + '-audience'"
                label="API 资源标识（Audience）"
                hint="已为每个环境自动生成。保护业务 API 时需与后端校验值一致；仅接入登录可保留默认值。这不是 Client ID。"
                required
                :error="errorFor('access_audience')"
                :error-id="errorId('access_audience')"
            >
                <UiInput
                    :id="props.prefix + '-audience'"
                    v-model="config.access_audience"
                    :name="props.prefix + '-audience'"
                    required
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="urn:my-app:api…"
                    :invalid="isInvalid('access_audience')"
                    :disabled="disabled"
                />
            </UiFormField>
        </div>

        <div class="rounded-xl border border-border bg-muted/20 p-4">
            <h3 class="mb-4 flex items-center gap-2 text-xs font-semibold">
                回调与浏览器来源
                <span class="status-badge status-muted">{{ props.environment === 'development' ? '开发' : '生产' }}</span>
            </h3>
            <div class="space-y-4">
                <UiFormField
                    :id="props.prefix + '-redirects'"
                    label="登录回调地址"
                    :required="redirectsRequired"
                    :hint="uriHint"
                    :error="errorFor('redirect_uris')"
                    :error-id="errorId('redirect_uris')"
                >
                    <textarea
                        :id="props.prefix + '-redirects'"
                        v-model="redirects"
                        class="ui-textarea"
                        :class="{ 'has-error': isInvalid('redirect_uris') }"
                        :name="props.prefix + '-redirects'"
                        :required="redirectsRequired"
                        autocomplete="off"
                        spellcheck="false"
                        :placeholder="props.environment === 'development' ? 'http://127.0.0.1:5173/callback…' : 'https://app.example/callback…'"
                        :aria-invalid="isInvalid('redirect_uris')"
                        :aria-describedby="isInvalid('redirect_uris') ? errorId('redirect_uris') : undefined"
                    />
                    <p class="form-help">每行一个地址，精确匹配；当前 {{ count(redirects) }} 个。</p>
                </UiFormField>
                <UiFormField
                    :id="props.prefix + '-logout-redirects'"
                    label="退出后回调地址"
                    optional
                    :hint="uriHint"
                    :error="errorFor('post_logout_redirect_uris')"
                    :error-id="errorId('post_logout_redirect_uris')"
                >
                    <textarea
                        :id="props.prefix + '-logout-redirects'"
                        v-model="logouts"
                        class="ui-textarea min-h-[72px]"
                        :class="{ 'has-error': isInvalid('post_logout_redirect_uris') }"
                        :name="props.prefix + '-logout-redirects'"
                        autocomplete="off"
                        spellcheck="false"
                        :placeholder="props.environment === 'development' ? 'http://127.0.0.1:5173/logged-out…' : 'https://app.example/logged-out…'"
                        :aria-invalid="isInvalid('post_logout_redirect_uris')"
                        :aria-describedby="isInvalid('post_logout_redirect_uris') ? errorId('post_logout_redirect_uris') : undefined"
                    />
                </UiFormField>
                <UiFormField
                    v-if="config.client_kind === 'browser_spa'"
                    :id="props.prefix + '-origins'"
                    label="浏览器来源"
                    required
                    hint="须与登录回调地址的来源完全一致（协议、主机与端口；http 80 / https 443 默认端口需省略）。"
                    :error="errorFor('browser_origins')"
                    :error-id="errorId('browser_origins')"
                >
                    <textarea
                        :id="props.prefix + '-origins'"
                        v-model="origins"
                        class="ui-textarea min-h-[72px]"
                        :class="{ 'has-error': isInvalid('browser_origins') }"
                        :name="props.prefix + '-origins'"
                        :required="config.redirect_uris.length > 0 || redirectsRequired"
                        autocomplete="off"
                        spellcheck="false"
                        :placeholder="props.environment === 'development' ? 'http://127.0.0.1:5173…' : 'https://app.example…'"
                        :aria-invalid="isInvalid('browser_origins')"
                        :aria-describedby="isInvalid('browser_origins') ? errorId('browser_origins') : undefined"
                    />
                </UiFormField>
            </div>
        </div>

        <div class="rounded-xl border border-border bg-muted/20 p-4">
            <h3 class="mb-4 text-xs font-semibold">授权与退出能力</h3>
            <div class="grid gap-4 md:grid-cols-2">
                <UiFormField
                    :id="props.prefix + '-scopes'"
                    label="允许的账户资料范围"
                    :error="errorFor('identity_scopes')"
                    :error-id="errorId('identity_scopes')"
                >
                    <UiSelect
                        :id="props.prefix + '-scopes'"
                        v-model="config.identity_scopes"
                        :name="props.prefix + '-scopes'"
                        :invalid="isInvalid('identity_scopes')"
                        :disabled="disabled"
                    >
                        <option value="openid">账户身份</option>
                        <option value="openid profile">身份、显示名称</option>
                        <option value="openid email">身份、邮箱</option>
                        <option value="openid profile email">身份、显示名称、邮箱</option>
                    </UiSelect>
                </UiFormField>
                <UiFormField
                    v-if="administrator"
                    :id="props.prefix + '-consent'"
                    label="资料同意策略"
                    :error="errorFor('consent_policy')"
                    :error-id="errorId('consent_policy')"
                >
                    <UiSelect
                        :id="props.prefix + '-consent'"
                        v-model="config.consent_policy"
                        :name="props.prefix + '-consent'"
                        :invalid="isInvalid('consent_policy')"
                        :disabled="disabled"
                    >
                        <option value="user_consent">用户同意</option>
                        <option value="enterprise_preconsent">企业预先同意</option>
                    </UiSelect>
                </UiFormField>
                <UiFormField
                    :id="props.prefix + '-logout'"
                    label="退出能力"
                    :error="errorFor('logout_capability')"
                    :error-id="errorId('logout_capability')"
                >
                    <UiSelect
                        :id="props.prefix + '-logout'"
                        v-model="config.logout_capability"
                        :name="props.prefix + '-logout'"
                        :invalid="isInvalid('logout_capability')"
                        :disabled="disabled"
                    >
                        <option value="backchannel_and_online">后端退出通知与在线校验</option>
                        <option value="online">在线校验（不接收退出通知）</option>
                        <option value="local_only">仅应用本地退出</option>
                    </UiSelect>
                </UiFormField>
            </div>
            <UiFormField
                v-if="config.logout_capability === 'backchannel_and_online'"
                :id="props.prefix + '-backchannel'"
                label="后端退出通知 HTTPS 地址"
                required
                class="mt-4"
                :error="errorFor('backchannel_logout_uri')"
                :error-id="errorId('backchannel_logout_uri')"
            >
                <UiInput
                    :id="props.prefix + '-backchannel'"
                    v-model="backchannel"
                    type="url"
                    :name="props.prefix + '-backchannel'"
                    required
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="https://app.example/logout/backchannel…"
                    :invalid="isInvalid('backchannel_logout_uri')"
                    :disabled="disabled"
                />
            </UiFormField>
        </div>
    </fieldset>
</template>
