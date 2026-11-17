import { expect, it } from 'vitest'
import {
    auditActionName,
    auditObjectName,
    auditResultLabel,
    auditResultTone,
    conflictDescription,
    deliveryRetryable,
    deliveryStatusName,
    deliveryStatusTone,
    deliveryStatuses,
    managementStateName,
    personKindName,
} from '@/lib/identity-labels'
import { formatRelative, formatUntil } from '@/utils/time'
import { buildCsv } from '@/utils/csv'

it('审计结果按语义归并为成功/拒绝/失败，未知值原样展示', () => {
    expect(auditResultLabel('success')).toBe('成功')
    expect(auditResultLabel('Allowed')).toBe('成功')
    expect(auditResultLabel('denied')).toBe('拒绝')
    expect(auditResultLabel('failure')).toBe('失败')
    expect(auditResultLabel('weird_state')).toBe('weird_state')
    expect(auditResultTone('denied')).toBe('status-danger')
    expect(auditResultTone('weird_state')).toBe('status-muted')
    expect(auditActionName('site_settings.saved')).toBe('保存站点设置')
    expect(auditActionName('site_settings.reset')).toBe('恢复默认站点设置')
    expect(auditObjectName('site_settings')).toBe('站点设置')
})
it('投递状态集合封闭且全部有中文名、色调与可重试判定', () => {
    for (const status of deliveryStatuses) {
        expect(deliveryStatusName(status)).not.toBe(status)
        expect(deliveryStatusTone(status)).toMatch(/^status-/)
    }
    expect(deliveryRetryable('permanent_failure')).toBe(true)
    expect(deliveryRetryable('retry')).toBe(true)
    expect(deliveryRetryable('delivered')).toBe(false)
    expect(deliveryRetryable('leased')).toBe(false)
})
it('冲突类型与管理状态有中文解释，未知冲突默认阻断并提示人工确认', () => {
    expect(conflictDescription('orphan_client').blocking).toBe(false)
    for (const kind of ['managed_binding', 'multiple_applications', 'issuer_conflict'])
        expect(conflictDescription(kind).blocking).toBe(true)
    const unknown = conflictDescription('future_kind')
    expect(unknown.blocking).toBe(true)
    expect(unknown.name).toBe('future_kind')
    expect(managementStateName('legacy_unmanaged')).toBe('待纳管')
    expect(personKindName('collaborator')).toBe('合作方')
})
it('相对时间覆盖过去与未来，过久的时间退回绝对时间', () => {
    const now = 1_700_000_000
    expect(formatRelative(now - 10, now)).toBe('刚刚')
    expect(formatRelative(now - 120, now)).toBe('2 分钟前')
    expect(formatRelative(now - 7200, now)).toBe('2 小时前')
    expect(formatRelative(now - 2 * 86_400, now)).toBe('2 天前')
    expect(formatRelative(now - 30 * 86_400, now)).toContain('2023')
    expect(formatUntil(now + 30, now)).toBe('30 秒后到期')
    expect(formatUntil(now + 600, now)).toBe('10 分钟后到期')
    expect(formatUntil(now - 1, now)).toBe('已到期')
})
it('CSV 转义引号、逗号与换行，并带 BOM 方便 Excel 识别编码', () => {
    const csv = buildCsv(['a', 'b'], [['x,y', 'he said "hi"'], ['line\nbreak', null]])
    expect(csv.charCodeAt(0)).toBe(0xfeff)
    const lines = csv.slice(1).split('\r\n')
    expect(lines[0]).toBe('a,b')
    expect(lines[1]).toBe('"x,y","he said ""hi"""')
    expect(lines[2]).toBe('"line\nbreak",')
})
