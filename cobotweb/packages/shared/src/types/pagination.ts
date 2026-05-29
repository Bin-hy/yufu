export interface PageQuery {
  page_number: number
  page_size: number
}

export interface PageResult<T> {
  page_number: number
  page_size: number
  total_pages: number
  data_list: T[]
}
