import { afterEach } from 'vitest'
import { cleanupConfirmations } from '@/utils/modal'
import { removeToken } from '@/common/cache/cookies'
afterEach(() => {
    removeToken()
    sessionStorage.clear()
    cleanupConfirmations()
    document.body.innerHTML = ''
    localStorage.clear()
})
if (!window.matchMedia)
    window.matchMedia = (query) => ({
        matches: false,
        media: query,
        onchange: null,
        addListener() {},
        removeListener() {},
        addEventListener() {},
        removeEventListener() {},
        dispatchEvent() {
            return true
        },
    })
if (!globalThis.ResizeObserver)
    globalThis.ResizeObserver = class {
        observe() {}
        unobserve() {}
        disconnect() {}
    }
if (!HTMLElement.prototype.scrollIntoView) HTMLElement.prototype.scrollIntoView = function () {}
