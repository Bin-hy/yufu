import source from '../../../brand/site-branding.defaults.json'
import type { SiteSettingsDocument } from './types'
export const defaultDocument = (): SiteSettingsDocument => JSON.parse(JSON.stringify(source))
