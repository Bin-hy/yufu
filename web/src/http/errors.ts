import axios from 'axios'

export type ErrorRecovery = 'correct-input' | 'reload' | 'check-result' | 'sign-in' | 'contact-admin' | 'try-later' | 'none'
export interface BusinessFailure {
    code: string
    title: string
    message: string
    requestId?: string
    status?: number
    recovery: ErrorRecovery
    cancelled: boolean
    outcomeUncertain: boolean
}
export interface FailureContext { operation?: 'login' | 'read' | 'write' }
type Description = [title: string, message: string, recovery: ErrorRecovery]
const messages: Record<string, Description> = {
    invalid_input: ['输入内容不符合要求', '输入内容不符合要求，请按表单提示检查后重试', 'correct-input'],
    invalid_scope: ['访问范围无效', '请求的访问范围不符合要求，请检查应用配置或联系应用管理员。', 'contact-admin'],
    unauthenticated: ['需要重新登录', '中心会话已失效，请重新登录。', 'sign-in'],
    login_required: ['需要登录', '请重新登录后继续当前操作。', 'sign-in'],
    forbidden: ['没有操作权限', '你没有执行此操作的权限，请联系管理员确认授权。', 'contact-admin'],
    access_denied: ['应用访问被拒绝', '当前账户未获准访问此应用，请联系管理员确认应用分配。', 'contact-admin'],
    consent_required: ['需要确认资料授权', '请返回应用登录入口，确认资料授权后继续。', 'none'],
    not_found: ['对象不存在', '该对象不存在或已不可访问，请重新加载列表核对。', 'reload'],
    conflict: ['数据已发生变化', '数据已被其他操作修改或与现有记录冲突，请先重新加载并核对，再决定是否提交。', 'reload'],
    expired: ['链接或操作已过期', '此链接或操作已过期，请重新发起流程，或联系管理员重新签发链接。', 'none'],
    already_consumed: ['链接或操作已使用', '此链接或操作已被使用，请先核对处理结果，不要重复提交。', 'check-result'],
    owner_required: ['需要先转交负责人', '该账户仍是应用负责人，请先转交负责人名下的应用，再执行此操作。', 'contact-admin'],
    last_administrator: ['需要保留有效管理员', '不能停用或移除最后一个有效管理员，请先安排其他管理员。', 'contact-admin'],
    approval_obsolete: ['审批已过期', '申请对应的配置已发生变化，请重新加载核对最新版本，再重新提交审批。', 'reload'],
    self_approval: ['不能审批自己的申请', '申请人不能审批自己的申请，请由其他有权限的管理员审批。', 'contact-admin'],
    dependency_unavailable: ['认证服务暂不可用', '认证服务暂时不可用，请稍后重试；持续失败请联系管理员。', 'try-later'],
    deadline_exceeded: ['服务响应超时', '认证服务响应超时，请稍后重试。', 'try-later'],
    capacity_exceeded: ['请求过于频繁', '当前请求量过大，请稍后重试。', 'try-later'],
    commit_uncertain: ['提交结果未知', '提交结果尚未确认，操作可能已经生效。请先刷新后核对结果，避免重复提交。', 'check-result'],
    csrf_origin_missing: ['认证来源缺失', '请求缺少来源信息，请通过浏览器页面登录', 'reload'],
    csrf_origin_invalid: ['认证来源无效', '请求来源信息无效，请使用完整的服务地址访问', 'reload'],
    csrf_origin_not_allowed: ['访问地址未获授权', '当前访问地址未获授权，请使用服务地址或在 auth.origins 中配置完整来源（含端口）', 'contact-admin'],
    csrf_cookie_invalid: ['认证 Cookie 无效', '浏览器认证 Cookie 无效，请清除此站点的 Cookie 后重试', 'sign-in'],
    csrf_cookie_missing: ['缺少认证 Cookie', '浏览器未发送认证 Cookie，请检查 Cookie 设置并使用同一地址访问', 'reload'],
    csrf_cookie_requires_https: ['需要安全连接', '浏览器未发送安全认证 Cookie；本机 HTTP 开发启动需加 --allow-insecure-loopback-cookies，其他环境请使用 HTTPS', 'contact-admin'],
    csrf_proof_missing: ['缺少认证校验信息', '请求缺少认证校验信息，请强制刷新页面后重试', 'reload'],
    csrf_proof_mismatch: ['认证信息已变化', '认证校验信息已变化，请重新点击登录或刷新页面后重试', 'reload'],
    csrf_proof_invalid: ['认证信息已失效', '认证校验信息已失效，请刷新页面后重试', 'reload'],
}
const legacyCodes: Record<string, string> = {
    'security dependency unavailable': 'security_dependency_unavailable',
    'invalid credential': 'unauthenticated',
    'credential revoked': 'unauthenticated',
    'operation forbidden': 'forbidden',
    'invalid resource': 'invalid_input',
    'request conflict': 'conflict',
    'security deadline exceeded': 'deadline_exceeded',
    'security capacity exceeded': 'capacity_exceeded',
    'security commit uncertain': 'commit_uncertain',
    internal_server_error: 'internal_server_error',
    AlreadyExists: 'conflict',
}
function record(value: unknown): Record<string, unknown> {
    return value !== null && typeof value === 'object' ? value as Record<string, unknown> : {}
}
export function isHtmlResponse(response?: { data?: unknown; headers?: unknown }): boolean {
    return String(record(response?.headers)['content-type'] ?? '').toLowerCase().includes('text/html')
        || (typeof response?.data === 'string' && /^\s*(?:<!doctype\s+html|<html\b)/i.test(response.data))
}
export class UncertainSubmissionError extends Error {
    readonly requestId?: string
    constructor(cause?: unknown) {
        super('创建结果未知，无法确认服务端是否已完成。请先核对现有记录，避免重复提交。')
        this.name = 'UncertainSubmissionError'
        this.requestId = normalizeFailure(cause).requestId
    }
}
export function isCancelledFailure(error: unknown): boolean {
    const e = record(error)
    return axios.isCancel(error) || e.code === 'ERR_CANCELED' || e.name === 'AbortError'
        || (error instanceof Error && error.message === '会话已改变')
}
function requestId(value: unknown): string | undefined {
    return typeof value === 'string' && /^[a-zA-Z0-9][a-zA-Z0-9._:-]{0,127}$/.test(value) ? value : undefined
}
export function normalizeFailure(error: unknown, context: FailureContext = {}): BusinessFailure {
    const e = record(error), response = record(e.response), data = record(response.data), config = record(e.config)
    const status = typeof response.status === 'number' ? response.status : undefined
    const operation = context.operation ?? (/\/(?:login)$|^login$/i.test(String(config.url ?? '')) ? 'login'
        : /^(post|put|patch|delete)$/i.test(String(config.method ?? '')) ? 'write' : 'read')
    const rawCode = typeof data.code === 'string' ? data.code
        : typeof response.data === 'string' ? (legacyCodes[response.data] ?? response.data) : ''
    let code = rawCode, description: Description | undefined
    if (isCancelledFailure(error)) {
        return { code: 'cancelled', title: '', message: '', recovery: 'none', cancelled: true, outcomeUncertain: false }
    }
    if (isHtmlResponse(response) || e.code === 'ERR_API_HTML_RESPONSE') {
        code = 'api_html_response'
        description = ['接口地址配置异常', '接口返回了 HTML 页面，请检查 API 地址或代理配置', 'contact-admin']
    } else if (operation === 'login' && (status === 401 || rawCode === 'unauthenticated' || rawCode === 'login_required')) {
        code = 'invalid_credentials'
        description = ['登录失败', '账户或密码不正确，请检查后重试。', 'correct-input']
    } else if (error instanceof UncertainSubmissionError || (!e.response && error instanceof Error && /^(创建结果未知|提交结果尚未确认)/.test(error.message))) {
        code = 'commit_uncertain'
        description = messages.commit_uncertain
    } else if (Object.prototype.hasOwnProperty.call(messages, code)) {
        description = messages[code]
    } else if (code === 'security_dependency_unavailable') {
        description = ['认证服务暂不可用', '认证服务暂不可用，请检查后端时钟同步与依赖状态', 'try-later']
    } else if (!e.response && ['ECONNABORTED', 'ETIMEDOUT', 'ERR_NETWORK'].includes(String(e.code))) {
        if (operation === 'write') {
            code = 'commit_uncertain'
            description = messages.commit_uncertain
        } else if (e.code === 'ERR_NETWORK') {
            code = 'network_unavailable'
            description = ['无法连接服务', '无法连接服务，请检查网络连接后重试。', 'try-later']
        } else {
            code = 'request_timeout'
            description = ['请求超时', '请求超时，请检查网络或稍后重试。', 'try-later']
        }
    }
    if (!description) {
        const statusCodes: Record<number, string> = { 400: 'invalid_input', 401: 'unauthenticated', 403: 'forbidden', 404: 'not_found', 409: 'conflict', 410: 'expired', 429: 'capacity_exceeded' }
        code = statusCodes[status ?? 0] ?? 'unknown_error'
        description = messages[code]
        if (!description) {
            description = ['操作未完成', status && status >= 500 ? '服务暂时出现异常，请稍后重试；持续失败请联系管理员。' : '操作失败，请稍后重试。', 'try-later']
            // Only application-authored local Error messages are eligible; never echo an HTTP response.
            const local = error instanceof Error && !axios.isAxiosError(error) && !e.response ? error.message : ''
            if (local && local.length <= 300 && /[\u3400-\u9fff]/.test(local)
                && !/[<>\r\n]|password|token|secret|cookie|select\s|stack|postgres|mysql/i.test(local)) {
                description = ['操作未完成', local, 'none']
            }
        }
    }
    return {
        code, title: description[0], message: description[1], recovery: description[2], status,
        requestId: requestId(data.request_id) ?? requestId(record(response.headers)['x-request-id'])
            ?? (error instanceof UncertainSubmissionError ? error.requestId : undefined),
        cancelled: false, outcomeUncertain: code === 'commit_uncertain',
    }
}
export function formatFailure(failure: BusinessFailure): string {
    return failure.cancelled ? '' : failure.message + (failure.requestId ? `（请求标识 ${failure.requestId}）` : '')
}
