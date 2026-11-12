import type { PickerOption } from '@/components/ui/entity-picker/types'
import type { Group } from '@/pages/admin/api/type'
import type { EnterpriseAccount } from '@/pages/admin/api/type'

export function accountOption(account: EnterpriseAccount): PickerOption {
    return {
        value: account.user_id,
        label: account.nickname || account.username,
        secondary: `@${account.username} · #${account.user_id}`,
    }
}

export function groupOption(group: Group): PickerOption {
    return {
        value: group.group_id,
        label: group.name,
        secondary: `用户组 · ${group.members.length} 位成员 · ${group.group_id.slice(0, 8)}…`,
    }
}

export function accountLabel(
    account: Pick<EnterpriseAccount, 'user_id' | 'username' | 'nickname'>,
    fallback = '账户',
) {
    const label = account.nickname || account.username || fallback
    return { label, secondary: `@${account.username} · #${account.user_id}` }
}
