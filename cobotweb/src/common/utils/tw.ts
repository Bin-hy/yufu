/**
 * 合并 Tailwind 类名的工具函数
 * - 避免直接字符串拼接
 */
export function cn(...classes: Array<string | false | null | undefined>) {
  return classes.filter(Boolean).join(' ')
}