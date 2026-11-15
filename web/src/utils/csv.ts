/** RFC 4180 style CSV with a UTF-8 BOM so Excel opens Chinese text correctly. */
export function buildCsv(header: string[], rows: (string | number | null | undefined)[][]) {
    const escape = (value: string | number | null | undefined) => {
        const text = value == null ? '' : String(value)
        return /[",\r\n]/.test(text) ? '"' + text.replace(/"/g, '""') + '"' : text
    }
    return (
        '\uFEFF' +
        [header, ...rows].map((row) => row.map(escape).join(',')).join('\r\n') +
        '\r\n'
    )
}

export function downloadFile(name: string, content: string, type = 'text/csv;charset=utf-8') {
    const url = URL.createObjectURL(new Blob([content], { type }))
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = name
    document.body.append(anchor)
    anchor.click()
    anchor.remove()
    URL.revokeObjectURL(url)
}
