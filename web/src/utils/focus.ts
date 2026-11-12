let pointerOrigin: HTMLElement | null = null
let pointerTime = 0

// macOS does not necessarily focus a button when it is clicked. Remember the
// actual pointer trigger as well as the keyboard focus for dialog return.
export function trackFocusOrigins() {
    function pointer(event: MouseEvent) {
        pointerOrigin =
            event.detail > 0 && event.target instanceof Element
                ? event.target.closest<HTMLElement>('button, a[href], input, select, textarea, [tabindex]')
                : null
        pointerTime = Date.now()
    }
    function keyboard() {
        pointerOrigin = null
    }
    document.addEventListener('click', pointer, true)
    document.addEventListener('keydown', keyboard, true)
    return () => {
        document.removeEventListener('click', pointer, true)
        document.removeEventListener('keydown', keyboard, true)
        pointerOrigin = null
    }
}

export function captureFocusOrigin(): HTMLElement | null {
    if (pointerOrigin?.isConnected && Date.now() - pointerTime < 500) return pointerOrigin
    return document.activeElement instanceof HTMLElement ? document.activeElement : null
}

export function returnFocus(origin: HTMLElement | null) {
    if (origin?.isConnected && origin.getClientRects().length && origin !== document.body) origin.focus()
    else document.querySelector<HTMLElement>('#workspace-content')?.focus()
}
