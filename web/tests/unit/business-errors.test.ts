import { describe, expect, it } from 'vitest'
import axios from 'axios'
import { formatFailure, normalizeFailure } from '@/http/errors'
const failure = (code: string, status = 409) => ({ response: { status, data: { code, request_id: 'req-1' } } })
describe('公开业务错误', () => {
    it.each([
        ['owner_required', '转交负责人'], ['last_administrator', '有效管理员'],
        ['self_approval', '其他有权限'], ['approval_obsolete', '最新版本'],
        ['expired', '已过期'], ['already_consumed', '已被使用'], ['forbidden', '权限'],
        ['conflict', '核对'], ['invalid_input', '表单提示'], ['invalid_scope', '访问范围'],
        ['unauthenticated', '重新登录'], ['login_required', '登录'], ['access_denied', '分配'],
        ['consent_required', '资料授权'], ['not_found', '不存在'],
        ['dependency_unavailable', '稍后重试'], ['deadline_exceeded', '超时'],
        ['capacity_exceeded', '请求量'], ['commit_uncertain', '可能已经生效'],
    ])('%s 显示明确原因', (code, message) => {
        const result = normalizeFailure(failure(code))
        expect(result.code).toBe(code)
        expect(formatFailure(result)).toContain(message)
        expect(formatFailure(result).match(/req-1/g)).toHaveLength(1)
    })
    it('已知码优先于未知服务端 message，非法标识回退到响应头', () => {
        const result = normalizeFailure({ response: { status: 409, data: { code: 'owner_required', message: 'secret-value', request_id: '<script>' }, headers: { 'x-request-id': 'header-1' } } })
        expect(formatFailure(result)).not.toContain('secret-value')
        expect(result.requestId).toBe('header-1')
        expect(normalizeFailure({ response: { data: { request_id: 'x'.repeat(129) } } }).requestId).toBeUndefined()
    })
    it.each([null, undefined, 4, { message: 'secret-value' }, { response: { status: 500, data: 'postgres password=secret-value' } }, { response: { data: { message: 'secret-value', code: 'unknown' } } }])('未知输入安全降级 %#', value => {
        const result = formatFailure(normalizeFailure(value))
        expect(result).not.toMatch(/secret-value|postgres|\[object Object\]/)
        expect(result).toContain('重试')
    })
    it('HTML 响应和传输异常有明确建议，写入超时结果未知', () => {
        expect(normalizeFailure({ response: { data: '<html>secret-value</html>' } }).code).toBe('api_html_response')
        expect(normalizeFailure({ code: 'ERR_NETWORK' }).code).toBe('network_unavailable')
        expect(normalizeFailure({ code: 'ETIMEDOUT' }).code).toBe('request_timeout')
        expect(normalizeFailure({ code: 'ECONNABORTED' }, { operation: 'write' }).outcomeUncertain).toBe(true)
    })
    it('取消不产生可见文案，登录失败不暴露账户存在性', () => {
        for (const e of [new axios.CanceledError(), { name: 'AbortError' }, Error('会话已改变')]) expect(formatFailure(normalizeFailure(e))).toBe('')
        for (const code of ['unauthenticated', 'login_required', 'unknown']) {
            expect(normalizeFailure(failure(code, 401), { operation: 'login' }).message).toBe('账户或密码不正确，请检查后重试。')
        }
    })
})

it.each(['csrf_origin_missing', 'csrf_origin_invalid', 'csrf_origin_not_allowed', 'csrf_cookie_invalid', 'csrf_cookie_missing', 'csrf_cookie_requires_https', 'csrf_proof_missing', 'csrf_proof_mismatch', 'csrf_proof_invalid'])('浏览器校验 %s 不退回通用错误', code => {
    const result = normalizeFailure(failure(code, 403))
    expect(result.code).toBe(code)
    expect(result.message).not.toContain('操作失败')
})
it.each([
    ['invalid credential', 'unauthenticated'], ['credential revoked', 'unauthenticated'],
    ['operation forbidden', 'forbidden'], ['invalid resource', 'invalid_input'],
    ['request conflict', 'conflict'], ['security deadline exceeded', 'deadline_exceeded'],
    ['security capacity exceeded', 'capacity_exceeded'], ['security commit uncertain', 'commit_uncertain'],
])('旧安全响应 %s 保留业务分类', (data, code) => {
    expect(normalizeFailure({ response: { status: 503, data } }).code).toBe(code)
})

it.each([[400, 'invalid_input'], [401, 'unauthenticated'], [403, 'forbidden'], [404, 'not_found'], [409, 'conflict'], [410, 'expired'], [429, 'capacity_exceeded']])('未知响应按 HTTP %i 安全降级', (status, code) => {
    expect(normalizeFailure({ response: { status, data: { message: 'private-backend-detail' } } }).code).toBe(code)
})
