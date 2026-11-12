import sitemap from '@astrojs/sitemap';
import starlight from '@astrojs/starlight';
import { defineConfig } from 'astro/config';

// Set SITE_URL in Cloudflare Pages (or locally) to the public origin.
const site = process.env.SITE_URL ?? 'https://cobotusr.pages.dev';

export default defineConfig({
  site,
  integrations: [
    starlight({
      title: 'yufu',
      description:
        '单企业统一身份与 SSO 平台：标准 OIDC、细粒度授权、自托管部署。',
      logo: {
        src: './src/assets/logo.svg',
        alt: 'yufu 鱼符',
      },
      favicon: '/favicon.svg',
      defaultLocale: 'root',
      locales: {
        root: { label: '简体中文', lang: 'zh-CN' },
        en: { label: 'English', lang: 'en' },
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/Bin-hy/cobot-for',
        },
      ],
      editLink: {
        baseUrl: 'https://github.com/Bin-hy/cobot-for/edit/main/',
      },
      customCss: ['./src/styles/custom.css'],
      lastUpdated: true,
      sidebar: [
        {
          label: '开始使用',
          translations: { en: 'Getting started' },
          items: [
            {
              label: '快速开始',
              translations: { en: 'Quick start' },
              slug: 'getting-started',
            },
          ],
        },
        {
          label: '接入指南',
          translations: { en: 'Guides' },
          items: [
            {
              label: '控制台使用',
              translations: { en: 'Using the consoles' },
              slug: 'guides/console',
            },
            {
              label: 'OIDC 接入',
              translations: { en: 'OIDC integration' },
              slug: 'guides/oidc-integration',
            },
            {
              label: '机密 Web 应用',
              translations: { en: 'Confidential web app' },
              slug: 'guides/web-app',
            },
            {
              label: '公共 SPA',
              translations: { en: 'Public SPA' },
              slug: 'guides/spa',
            },
            {
              label: 'Go SDK',
              translations: { en: 'Go SDK' },
              slug: 'guides/go-sdk',
            },
          ],
        },
        {
          label: '参考',
          translations: { en: 'Reference' },
          items: [
            {
              label: '配置项',
              translations: { en: 'Configuration' },
              slug: 'reference/configuration',
            },
            {
              label: '端点与令牌',
              translations: { en: 'Endpoints and tokens' },
              slug: 'reference/endpoints',
            },
          ],
        },
        {
          label: '运维',
          translations: { en: 'Operations' },
          items: [
            {
              label: '生产部署',
              translations: { en: 'Deployment' },
              slug: 'operations/deployment',
            },
            {
              label: '容器部署',
              translations: { en: 'Container deployment' },
              slug: 'operations/docker',
            },
            {
              label: '安全与备份',
              translations: { en: 'Security and backup' },
              slug: 'operations/security',
            },
          ],
        },
        {
          label: '常见问题',
          translations: { en: 'FAQ' },
          slug: 'faq',
        },
      ],
      head: [
        {
          tag: 'meta',
          attrs: { property: 'og:type', content: 'website' },
        },
        {
          tag: 'meta',
          attrs: { name: 'theme-color', content: '#0f766e' },
        },
      ],
    }),
    sitemap(),
  ],
});
