/**
 * 主题切换组件 Props 定义
 * - 默认根据 localStorage 或系统偏好设置
 */
export interface ThemeSwitcherProps {
  /** 初始主题，优先级低于 localStorage 的持久化值 */
  initialTheme?: 'light' | 'dark'
  /** 是否显示标签文本 */
  showLabel?: boolean
  /** 额外类名（通过 Tailwind 原子类扩展） */
  class?: string
}

export type Theme = 'light' | 'dark'