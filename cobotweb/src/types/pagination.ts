export interface PageResult<T> {
  page_size: number
  page_number: number
  total_pages: number
  data_list: T[]
}