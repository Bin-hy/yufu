import { request } from '@/http/axios'
import type { ApplicationPage, ApplicationInsertReq, ApplicationPatchReq } from './type'
import type { ApplicationVO } from '@/types/models'

export async function getApplicationsByPage(
  page_number: number,
  page_size: number
): Promise<ApplicationPage> {
  return request<ApplicationPage>({
    url: '/application',
    method: 'GET',
    params: { page_number, page_size }
  })
}

export async function createApplication(payload: ApplicationInsertReq): Promise<ApplicationVO> {
  return request<ApplicationVO>({
    url: '/application',
    method: 'POST',
    data: payload
  })
}

export async function patchApplication(payload: ApplicationPatchReq): Promise<ApplicationVO> {
  return request<ApplicationVO>({
    url: '/application',
    method: 'PATCH',
    data: payload
  })
}

export async function deleteApplication(id: number): Promise<string> {
  return request<string>({
    url: '/application',
    method: 'DELETE',
    data: { id }
  })
}
