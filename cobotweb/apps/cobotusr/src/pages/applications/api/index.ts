import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type { ApplicationPage, ApplicationInsertReq, ApplicationPatchReq } from './type'
import type { ApplicationVO } from '@/types/models'

export async function getApplicationsByPage(
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
): Promise<ApplicationPage> {
    return request<ApplicationPage>({
        ...options,
        url: '/application',
        method: 'GET',
        params: { page_number, page_size },
    })
}

export async function createApplication(
    payload: ApplicationInsertReq,
    options: RequestOptions = {},
): Promise<ApplicationVO> {
    return request<ApplicationVO>({
        ...options,
        url: '/application',
        method: 'POST',
        data: payload,
    })
}

export async function patchApplication(
    payload: ApplicationPatchReq,
    options: RequestOptions = {},
): Promise<ApplicationVO> {
    return request<ApplicationVO>({
        ...options,
        url: '/application',
        method: 'PATCH',
        data: payload,
    })
}

export async function deleteApplication(id: number, options: RequestOptions = {}): Promise<string> {
    return request<string>({
        ...options,
        url: '/application',
        method: 'DELETE',
        data: { id },
    })
}
