// 为 .vue 单文件组件提供类型声明，避免隐式 any 报错
declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<Record<string, unknown>, Record<string, unknown>, any>
  export default component
}