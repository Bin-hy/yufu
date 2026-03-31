// 用户模块本地表单与视图类型
// 说明：与后端请求/响应类型区分，专用于页面表单与选项的类型约束

export interface CreateUserForm {
  username: string
  nickname: string
  password: string
  confirmPassword: string
  role_id: number
}

export interface RoleOption {
  label: string
  value: number
}

export interface FormErrors {
  username: string
  nickname: string
  password: string
  confirmPassword: string
}