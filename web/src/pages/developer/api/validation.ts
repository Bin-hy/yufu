import type { EnvironmentConfig, Registration } from './type'

export type EnvironmentKind = 'development' | 'production'
export type EnvironmentConfigField =
    | 'launch_url'
    | 'launch_description'
    | 'client_kind'
    | 'redirect_uris'
    | 'post_logout_redirect_uris'
    | 'browser_origins'
    | 'access_audience'
    | 'identity_scopes'
    | 'consent_policy'
    | 'logout_capability'
    | 'backchannel_logout_uri'

export interface ValidationIssue {
    field: string
    message: string
}

const allowedScopes = new Set(['openid', 'profile', 'email'])

function hasControl(value: string) {
    for (let index = 0; index < value.length; index++) {
        const code = value.charCodeAt(index)
        if (code < 0x20 || code === 0x7f) return true
    }
    return false
}

function environmentLabel(environment: EnvironmentKind) {
    return environment === 'development' ? '开发环境' : '生产环境'
}

function isLoopback(hostname: string) {
    return hostname === 'localhost' || hostname === '[::1]' || /^127\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(hostname)
}

function uriIssue(uri: string, environment: EnvironmentKind, field: EnvironmentConfigField, label: string): ValidationIssue | null {
    if (!uri || uri.length > 2048 || /\s/.test(uri) || hasControl(uri) || uri.includes('*') || uri.includes('\\') || uri.includes('#'))
        return { field, message: `${label} ${uri || '(空)'} 无效：不能为空，且不能包含空格、控制字符、*、\\ 或 #` }
    let parsed: URL
    try {
        parsed = new URL(uri)
    } catch {
        return { field, message: `${label} ${uri} 无效：必须是包含协议与主机的完整地址` }
    }
    if (!parsed.hostname || parsed.username || parsed.password)
        return { field, message: `${label} ${uri} 无效：缺少主机，或包含用户名与密码` }
    if (parsed.protocol !== 'https:' && !(environment === 'development' && parsed.protocol === 'http:' && isLoopback(parsed.hostname)))
        return {
            field,
            message: environment === 'development'
                ? `${label} ${uri} 无效：必须使用 https；本机调试仅 http://localhost、http://127.0.0.1 等回环地址可用`
                : `${label} ${uri} 无效：生产环境必须使用 https，不接受任何 http 地址（包括 127.0.0.1）`,
        }
    return null
}

export function environmentConfigIssue(config: EnvironmentConfig, environment: EnvironmentKind, activation = false): ValidationIssue | null {
    const label = environmentLabel(environment)
    const lists: readonly (readonly [string[], string, EnvironmentConfigField])[] = [
        [config.redirect_uris, '登录回调地址', 'redirect_uris'],
        [config.post_logout_redirect_uris, '退出后回调地址', 'post_logout_redirect_uris'],
        [config.browser_origins, '浏览器来源', 'browser_origins'],
    ]
    for (const [values, name, field] of lists) {
        if (values.length > 16) return { field, message: `${label}「${name}」最多填写 16 条` }
        if (new Set(values).size !== values.length) return { field, message: `${label}「${name}」存在重复项` }
    }
    if (!config.redirect_uris.length && (environment === 'development' || activation))
        return { field: 'redirect_uris', message: `${label}「登录回调地址」至少填写一条` }
    for (const uri of config.redirect_uris) {
        const issue = uriIssue(uri, environment, 'redirect_uris', `${label}「登录回调地址」`)
        if (issue) return issue
    }
    for (const uri of config.post_logout_redirect_uris) {
        const issue = uriIssue(uri, environment, 'post_logout_redirect_uris', `${label}「退出后回调地址」`)
        if (issue) return issue
    }
    if (config.client_kind === 'browser_spa') {
        for (const origin of config.browser_origins) {
            const issue = uriIssue(origin, environment, 'browser_origins', `${label}「浏览器来源」`)
            if (issue) return issue
            let parsed: URL
            try {
                parsed = new URL(origin)
            } catch {
                return { field: 'browser_origins', message: `${label}「浏览器来源」${origin} 无效：必须是包含协议与主机的完整来源` }
            }
            if (parsed.origin !== origin)
                return { field: 'browser_origins', message: `${label}「浏览器来源」${origin} 无效：必须是精确来源（协议、主机与端口；无路径与查询，默认端口需省略）` }
        }
        const origins = new Set(config.redirect_uris.map(uri => new URL(uri).origin))
        if (origins.size !== config.browser_origins.length || config.browser_origins.some(origin => !origins.has(origin)))
            return { field: 'browser_origins', message: `${label}「浏览器来源」须与登录回调地址的来源完全一致` }
    } else if (config.browser_origins.length) {
        return { field: 'browser_origins', message: `${label} 服务端 Web 类型不能填写浏览器来源` }
    }
    if (config.launch_url) {
        if (!config.launch_url.includes('://'))
            return { field: 'launch_url', message: `${label}「应用启动地址」必须包含协议和主机` }
        const issue = uriIssue(config.launch_url, environment, 'launch_url', `${label}「应用启动地址」`)
        if (issue) return { field: 'launch_url', message: `${label}「应用启动地址」无效：使用完整 HTTPS 地址；开发环境另支持本机回环 HTTP，禁止凭据、空白、控制字符、反斜杠和 fragment` }
        try {
            const decoded = decodeURIComponent(config.launch_url)
            if (hasControl(decoded) || decoded.includes('*'))
                return { field: 'launch_url', message: `${label}「应用启动地址」不能包含编码后的控制字符或通配符` }
        } catch {
            return { field: 'launch_url', message: `${label}「应用启动地址」编码无效` }
        }
    }
    if (config.launch_description && ([...config.launch_description.trim()].length > 1000 || /\p{Cc}/u.test(config.launch_description)))
        return { field: 'launch_description', message: `${label}「启动说明」最多 1000 字符，不能包含控制字符` }
    const backchannel = config.backchannel_logout_uri
    if ((config.logout_capability === 'backchannel_and_online') !== (backchannel != null))
        return { field: 'backchannel_logout_uri', message: `${label}「后端退出通知」仅在退出能力为“后端退出通知与在线校验”时填写，且此时必须填写` }
    if (backchannel) {
        const issue = uriIssue(backchannel, 'production', 'backchannel_logout_uri', `${label}「后端退出通知地址」`)
        if (issue) return issue
    }
    const audience = config.access_audience
    if (!audience || audience.length > 512 || audience.includes('*') || /\s/.test(audience) || hasControl(audience))
        return { field: 'access_audience', message: `${label}「访问 Audience」不能为空，且不能包含空格、* 或控制字符` }
    const scopes = config.identity_scopes.split(' ').filter(Boolean)
    if (config.identity_scopes.length > 128 || hasControl(config.identity_scopes) || !scopes.includes('openid') || scopes.some(scope => !allowedScopes.has(scope)))
        return { field: 'identity_scopes', message: `${label}「资料范围」必须包含 openid，且只能由 openid、profile、email 组成` }
    return null
}

export function validateEnvironmentConfig(config: EnvironmentConfig, environment: EnvironmentKind, activation = false): string | null {
    return environmentConfigIssue(config, environment, activation)?.message ?? null
}

export function registrationBasicIssue(registration: Registration, administrator = false): ValidationIssue | null {
    if (!registration.app_key.trim() || registration.app_key.length > 512 || hasControl(registration.app_key))
        return { field: 'app_key', message: '「应用标识」不能为空，长度不超过 512，且不能包含控制字符' }
    const name = registration.name.trim()
    if (!name || name.length > 200 || hasControl(registration.name))
        return { field: 'name', message: '「应用名称」不能为空，长度不超过 200，且不能包含控制字符' }
    if (registration.description && (registration.description.length > 2000 || hasControl(registration.description)))
        return { field: 'description', message: '「说明」长度不超过 2000，且不能包含控制字符' }
    if (!administrator && registration.development.consent_policy !== 'user_consent')
        return { field: 'development.consent_policy', message: '开发者注册的资料同意策略必须为用户同意' }
    if (!administrator && registration.production.consent_policy !== 'user_consent')
        return { field: 'production.consent_policy', message: '开发者注册的资料同意策略必须为用户同意' }
    return null
}

export function registrationIssue(registration: Registration, administrator = false): ValidationIssue | null {
    const basic = registrationBasicIssue(registration, administrator)
    if (basic) return basic
    const development = environmentConfigIssue(registration.development, 'development', true)
    if (development) return { field: `development.${development.field}`, message: development.message }
    const production = environmentConfigIssue(registration.production, 'production', administrator)
    if (production) return { field: `production.${production.field}`, message: production.message }
    return null
}

export function validateRegistration(registration: Registration, administrator = false): string | null {
    return registrationIssue(registration, administrator)?.message ?? null
}
