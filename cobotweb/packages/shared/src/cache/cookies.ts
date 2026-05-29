import Cookies from 'js-cookie'

const TOKEN_KEY = 'cobotrix_token'

export function getToken(): string | undefined {
  return Cookies.get(TOKEN_KEY)
}

export function setToken(token: string, opts?: Cookies.CookieAttributes) {
  Cookies.set(TOKEN_KEY, token, opts)
}

export function clearToken() {
  Cookies.remove(TOKEN_KEY)
}
