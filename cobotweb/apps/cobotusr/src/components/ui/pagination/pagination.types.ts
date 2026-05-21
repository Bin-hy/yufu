// 为兼容旧版与新版命名，所有字段为可选，运行时做归一化
export interface PaginationProps {
  // 新版命名（推荐）
  page?: number
  pageCount?: number
  size?: number
  sizeOptions?: number[]
  total?: number

  // 旧版命名（兼容）
  pageNumber?: number
  totalPages?: number
  pageSize?: number
  pageSizeOptions?: number[]
  totalItems?: number

  disabled?: boolean
}

export interface PaginationEmits {
  // 旧版事件（兼容）
  (e: 'update:pageNumber', value: number): void
  (e: 'change', value: number): void
  (e: 'update:pageSize', value: number): void
  (e: 'size-change', value: number): void

  // 新版事件（推荐）
  (e: 'update:page', value: number): void
  (e: 'page-change', value: number): void
  (e: 'update:size', value: number): void
}