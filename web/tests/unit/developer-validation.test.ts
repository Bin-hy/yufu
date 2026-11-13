import { it, expect } from 'vitest'
import { validateEnvironmentConfig, validateRegistration } from '@/pages/developer/api/validation'
import { emptyConfig, type EnvironmentConfig, type Registration } from '@/pages/developer/api/type'

const config = (environment: 'development' | 'production', overrides: Partial<EnvironmentConfig> = {}): EnvironmentConfig => ({
    ...emptyConfig(),
    redirect_uris: [environment === 'development' ? 'http://127.0.0.1:5173/callback' : 'https://app.example/callback'],
    access_audience: 'urn:app:api',
    ...overrides,
})

const registration = (overrides: { development?: Partial<EnvironmentConfig>; production?: Partial<EnvironmentConfig> } = {}): Registration => ({
    app_key: 'yufu-spa-example',
    name: 'yufu SPA 示例',
    description: null,
    third_party: false,
    development: config('development', { client_kind: 'browser_spa', post_logout_redirect_uris: [], browser_origins: ['http://127.0.0.1:5173'], ...overrides.development }),
    production: config('production', { redirect_uris: [], ...overrides.production }),
})

it('生产环境 http 回调被拦截并指明原因（原始缺陷载荷）', () => {
    const problem = validateRegistration(registration({
        production: { redirect_uris: ['http://127.0.0.1:5173/callback'], post_logout_redirect_uris: ['http://127.0.0.1:5173/callback'] },
    }))
    expect(problem).toContain('生产环境「登录回调地址」')
    expect(problem).toContain('http://127.0.0.1:5173/callback')
    expect(problem).toContain('生产环境必须使用 https')
})

it('开发环境 http 回环回调与 SPA 来源一致时校验通过', () => {
    expect(validateRegistration(registration())).toBeNull()
    expect(validateEnvironmentConfig(config('development', { client_kind: 'browser_spa', browser_origins: ['http://127.0.0.1:5173'] }), 'development', true)).toBeNull()
})

it('生产回调可留空，管理员立即生效场景必填', () => {
    expect(validateEnvironmentConfig(config('production', { redirect_uris: [] }), 'production', false)).toBeNull()
    expect(validateEnvironmentConfig(config('production', { redirect_uris: [] }), 'production', true)).toContain('至少填写一条')
})

it('SPA 浏览器来源必须与回调来源完全一致', () => {
    const problem = validateEnvironmentConfig(config('development', { client_kind: 'browser_spa', browser_origins: ['http://localhost:5173'] }), 'development', true)
    expect(problem).toContain('须与登录回调地址的来源完全一致')
})

it('后端退出通知与退出能力必须成对出现且为 https', () => {
    expect(validateEnvironmentConfig(config('development', { logout_capability: 'backchannel_and_online' }), 'development', true)).toContain('必须填写')
    expect(validateEnvironmentConfig(config('development', { logout_capability: 'backchannel_and_online', backchannel_logout_uri: 'http://127.0.0.1/backchannel' }), 'development', true)).toContain('必须使用 https')
})

it('启动地址按环境校验，旧配置和空值保持可编辑', () => {
    expect(validateEnvironmentConfig(config('production'), 'production')).toBeNull()
    expect(validateEnvironmentConfig(config('development', { launch_url: 'http://localhost:3000/' }), 'development')).toBeNull()
    for (const launch_url of ['http://localhost:3000/', 'https:app.example/home', '/home', 'https://user:pass@app.example/', 'https://app.example/#x', 'https://app.example/%0d', 'https://app.example/\\home']) {
        expect(validateEnvironmentConfig(config('production', { launch_url }), 'production')).toContain('应用启动地址')
    }
    expect(validateEnvironmentConfig(config('production', { launch_description: 'a'.repeat(1001) }), 'production')).toContain('启动说明')
    expect(validateEnvironmentConfig(config('production', { launch_description: 'hello\n' }), 'production')).toContain('启动说明')
})
