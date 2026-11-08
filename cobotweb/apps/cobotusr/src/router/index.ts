import { createRouter } from 'vue-router'
import { routerConfig } from './config'
import { constantRoutes } from './routes'
export const router = createRouter({ history: routerConfig.history, routes: constantRoutes })
