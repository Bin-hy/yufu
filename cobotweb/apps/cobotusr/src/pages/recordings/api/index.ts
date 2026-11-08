import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type { RecordingVO } from '@/types/models'
import type { PageResult } from '@/types/pagination'

// 根据用户Id分页获取用户所有操作记录
export async function get_recording_by_user_id_page(
    user_id: number,
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
) {
    return await request<PageResult<RecordingVO>>({
        ...options,
        url: `/recordings/${user_id}`,
        method: 'GET',
        params: { page_number, page_size },
    })
}

// 分页获取所有用户所有操作记录
export async function get_recording_by_page(
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
) {
    return await request<PageResult<RecordingVO>>({
        ...options,
        url: '/recordings',
        method: 'GET',
        params: { page_number, page_size },
    })
}
