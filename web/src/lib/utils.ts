export type ClassValue = string | number | null | undefined | ClassDictionary | ClassArray
interface ClassDictionary {
    [id: string]: unknown
}
type ClassArray = Array<ClassValue>

function toVal(mix: ClassValue) {
    let k,
        str = ''
    if (typeof mix === 'string' || typeof mix === 'number') {
        str += mix
    } else if (Array.isArray(mix)) {
        for (let i = 0; i < mix.length; i++) {
            if (mix[i]) {
                const x = toVal(mix[i])
                if (x) {
                    if (str) str += ' '
                    str += x
                }
            }
        }
    } else if (mix) {
        for (k in mix) {
            if (mix[k]) {
                if (str) str += ' '
                str += k
            }
        }
    }
    return str
}

export function cn(...inputs: ClassValue[]) {
    return inputs.map(toVal).filter(Boolean).join(' ')
}
