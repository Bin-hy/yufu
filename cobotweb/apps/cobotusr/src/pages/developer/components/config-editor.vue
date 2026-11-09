<script setup lang="ts">
import { computed, watch } from 'vue'
import type { EnvironmentConfig } from '../api/type'
import UiInput from '@/components/ui/input/input.vue'
const props = defineProps<{ prefix: string; administrator?: boolean; disabled?: boolean }>()
const config = defineModel<EnvironmentConfig>({ required: true })
watch(() => config.value.client_kind, kind => { if (kind === 'server_web') config.value.browser_origins = [] })
const textList = (key: 'redirect_uris' | 'post_logout_redirect_uris' | 'browser_origins') => computed({ get: () => config.value[key].join('\n'), set: (value: string) => { config.value[key] = value.split('\n').map(v => v.trim()).filter(Boolean) } })
const redirects = textList('redirect_uris'), logouts = textList('post_logout_redirect_uris'), origins = textList('browser_origins')
const backchannel = computed({ get: () => config.value.backchannel_logout_uri ?? '', set: (v: string) => { config.value.backchannel_logout_uri = v.trim() || null } })
</script>
<template>
    <fieldset class="space-y-4" :disabled="disabled">
        <div class="form-field"><label :for="props.prefix + '-kind'">应用类型</label><select :id="props.prefix + '-kind'" v-model="config.client_kind" class="ui-select"><option value="server_web">服务端 Web（服务端保存客户端密钥）</option><option value="browser_spa">浏览器 SPA（S256 PKCE，无客户端密钥）</option></select></div>
        <div class="form-field"><label :for="props.prefix + '-redirects'">登录回调地址（每行一个，精确匹配）</label><textarea :id="props.prefix + '-redirects'" v-model="redirects" class="ui-input min-h-20" required spellcheck="false" /></div>
        <div class="form-field"><label :for="props.prefix + '-logout-redirects'">退出后回调地址（可选，每行一个）</label><textarea :id="props.prefix + '-logout-redirects'" v-model="logouts" class="ui-input min-h-16" spellcheck="false" /></div>
        <div v-if="config.client_kind === 'browser_spa'" class="form-field"><label :for="props.prefix + '-origins'">浏览器来源（含协议与端口，每行一个）</label><textarea :id="props.prefix + '-origins'" v-model="origins" class="ui-input min-h-16" required spellcheck="false" /></div>
        <div class="form-field"><label :for="props.prefix + '-audience'">保护 API 的访问 Audience</label><UiInput :id="props.prefix + '-audience'" v-model="config.access_audience" required /></div>
        <div class="form-field"><label :for="props.prefix + '-scopes'">允许的账户资料范围</label><select :id="props.prefix + '-scopes'" v-model="config.identity_scopes" class="ui-select"><option value="openid">账户身份</option><option value="openid profile">身份、显示名称</option><option value="openid email">身份、邮箱</option><option value="openid profile email">身份、显示名称、邮箱</option></select></div>
        <div v-if="administrator" class="form-field"><label :for="props.prefix + '-consent'">资料同意策略</label><select :id="props.prefix + '-consent'" v-model="config.consent_policy" class="ui-select"><option value="user_consent">用户同意</option><option value="enterprise_preconsent">企业预先同意</option></select></div>
        <div class="form-field"><label :for="props.prefix + '-logout'">退出能力</label><select :id="props.prefix + '-logout'" v-model="config.logout_capability" class="ui-select"><option value="backchannel_and_online">后端退出通知与在线校验</option><option value="online">在线校验（不接收退出通知）</option><option value="local_only">仅应用本地退出</option></select></div>
        <div v-if="config.logout_capability === 'backchannel_and_online'" class="form-field"><label :for="props.prefix + '-backchannel'">后端退出通知 HTTPS 地址</label><UiInput :id="props.prefix + '-backchannel'" v-model="backchannel" type="url" required /></div>
    </fieldset>
</template>
