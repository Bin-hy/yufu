import { createVNode, render } from 'vue'
import DaisyModal from '@/components/DaisyModal.vue'

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
    const vnode = createVNode(DaisyModal, {
      ...options,
      onConfirm: () => {
        options.confirmCallback? options.confirmCallback():""
        resolve(true)
        cleanup()
      },
      onCancel: () => {
        options.cancleCallback? options.cancleCallback():""
        resolve(false)
        cleanup()
      },
      onClose: cleanup,
    })

    function cleanup() {
      render(null, container)
      container.remove()
    }

    render(vnode, container)
    // 弹出 modal
    const instance = vnode.component?.exposed
    instance?.open?.()
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
