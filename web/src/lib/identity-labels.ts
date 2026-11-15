/**
 * Shared Chinese labels for identity enumerations. Pages must render these
 * instead of raw backend values so terminology stays consistent and a future
 * locale only has to replace this one module.
 */
export const personKindNames: Record<string, string> = {
    employee: '企业员工',
    collaborator: '合作方',
}
export const personKindName = (kind: string) => personKindNames[kind] ?? kind

export const environmentNames: Record<string, string> = {
    development: '开发环境',
    production: '生产环境',
}
export const environmentName = (kind: string) => environmentNames[kind] ?? kind

export const deliveryStatusNames: Record<string, string> = {
    pending: '等待发送',
    leased: '发送处理中',
    retry: '等待重试',
    delivered: '已送达',
    permanent_failure: '发送失败',
    expired: '已超过期限',
}
/** The closed set of delivery states, in presentation order. */
export const deliveryStatuses = [
    'pending',
    'leased',
    'retry',
    'delivered',
    'permanent_failure',
    'expired',
] as const
export const deliveryStatusName = (status: string) => deliveryStatusNames[status] ?? status
export const deliveryStatusTone = (status: string) =>
    status === 'delivered'
        ? 'status-success'
        : ['pending', 'leased', 'retry'].includes(status)
          ? 'status-info'
          : status === 'expired'
            ? 'status-muted'
            : 'status-danger'
/** States an administrator may requeue; delivered/expired are final, leased is held by a worker. */
export const deliveryRetryable = (status: string) =>
    !['delivered', 'expired', 'leased'].includes(status)

export const auditResultLabels: Record<string, string> = {
    success: '成功',
    denied: '拒绝',
    failure: '失败',
}
export function auditResultLabel(result: string) {
    const normalized = result.toLowerCase()
    if (['success', 'ok', 'allowed', 'accepted'].some((value) => normalized.includes(value)))
        return auditResultLabels.success!
    if (['denied', 'rejected'].some((value) => normalized.includes(value)))
        return auditResultLabels.denied!
    if (['failure', 'failed', 'error'].some((value) => normalized.includes(value)))
        return auditResultLabels.failure!
    return result
}
export function auditResultTone(result: string) {
    const normalized = result.toLowerCase()
    if (['success', 'ok', 'allowed', 'accepted'].some((value) => normalized.includes(value)))
        return 'status-success'
    if (
        ['denied', 'rejected', 'failure', 'failed', 'error'].some((value) =>
            normalized.includes(value),
        )
    )
        return 'status-danger'
    return 'status-muted'
}

export const managementStateNames: Record<string, string> = {
    managed: '已纳管',
    legacy_unmanaged: '待纳管',
    retired: '已退役',
}
export const managementStateName = (state: string) => managementStateNames[state] ?? state

export interface ConflictDescription {
    name: string
    explanation: string
    /** Whether this conflict blocks adoption until it is resolved elsewhere. */
    blocking: boolean
}
export const conflictKindDescriptions: Record<string, ConflictDescription> = {
    orphan_client: {
        name: '孤立客户端',
        explanation: '该历史客户端没有关联任何应用记录，可以直接纳管并补建应用。',
        blocking: false,
    },
    managed_binding: {
        name: '已有纳管绑定',
        explanation: '关联的应用记录已处于纳管状态，重复纳管会破坏现有归属，需先核对关联应用。',
        blocking: true,
    },
    multiple_applications: {
        name: '关联多个应用',
        explanation: '多个应用记录都关联这个客户端，无法自动确定唯一归属，需要先在数据层确认。',
        blocking: true,
    },
    issuer_conflict: {
        name: '签发方不一致',
        explanation: '客户端登记的签发方与当前认证中心不一致，直接纳管会使对方验签失败，需先与接入方确认。',
        blocking: true,
    },
}
export function conflictDescription(kind: string): ConflictDescription {
    return (
        conflictKindDescriptions[kind] ?? {
            name: kind,
            explanation: '未知冲突类型，请结合审计记录人工确认后再纳管。',
            blocking: true,
        }
    )
}
