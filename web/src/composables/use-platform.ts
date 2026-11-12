export function useShortcutLabel() {
    const isMac =
        typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform)
    return { isMac, commandKey: isMac ? '⌘K' : 'Ctrl K' }
}
