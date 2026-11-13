import { emptyConfig, type Registration } from './type'

/** Keep generated identifiers stable for the lifetime of a form, including retries. */
export function newRegistrationDraft(administrator = false): Registration {
    const id = crypto.randomUUID()
    return {
        app_key: `app-${id}`,
        name: '',
        description: '',
        third_party: administrator,
        development: { ...emptyConfig(), access_audience: `urn:yufu:${id}:development:api` },
        production: { ...emptyConfig(), access_audience: `urn:yufu:${id}:production:api` },
    }
}
