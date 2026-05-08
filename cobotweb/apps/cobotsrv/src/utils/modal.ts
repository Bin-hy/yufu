import { createVNode, render } from 'vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'

interface ModalOptions {
  title: string
  content: string
  confirmText?: string
  cancelText?: string
  confirmCallback?: Function
  cancleCallback?: Function
}

/**
 * 弹出弹窗
 * 使用: showModal({title:"" , content: "", confirmCallback:()=>{}, cancleCallback: ()=>{} })
 * */
export function showModal(options: ModalOptions): Promise<boolean> {
  return new Promise((resolve) => {
    const container = document.createElement('div')
    document.body.appendChild(container)
    handle_default_value(options)
    // 通过 UiDialog 的默认插槽渲染内容，避免 content 作为 props 被忽略
    const vnode = createVNode(
      UiDialog as any,
      {
        title: options.title,
        open: true,
        onConfirm: () => {
          options.confirmCallback ? options.confirmCallback() : ''
          resolve(true)
          cleanup()
        },
        onCancel: () => {
          options.cancleCallback ? options.cancleCallback() : ''
          resolve(false)
          cleanup()
        },
        'onUpdate:open': (v: boolean) => { if (!v) cleanup() },
      },
      {
        default: () => options.content,
      }
    )

    function cleanup() {
      render(null, container)
      container.remove()
    }

    render(vnode, container)
    // 弹出 modal
    // UiDialog 通过 open: true 初始化显示
  })
}
function handle_default_value(options: ModalOptions){
    if(!options.cancelText){
        options.cancelText = "取消"
    }
    if(!options.confirmText){
        options.confirmText = "确认"
    }
}
