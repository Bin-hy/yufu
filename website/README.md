# cobotusr website

Official website and documentation for cobotusr, built with
[Astro](https://astro.build) and [Starlight](https://starlight.astro.build).

- Chinese (default): `/`
- English: `/en/`
- Static output: `dist/`

## Local development

```bash
pnpm install
pnpm --filter @cobotrix/website dev
```

The dev server runs at `http://localhost:4321`.

## Build

```bash
pnpm --filter @cobotrix/website build
pnpm --filter @cobotrix/website preview
```

## Content

```
src/content/docs/          Chinese pages (served at /)
src/content/docs/en/       English pages (served at /en/)
src/styles/custom.css      Brand palette and landing page styles
```

Sidebar labels live in `astro.config.mjs` and use Starlight `translations`.
Every page needs a `title` frontmatter field; add the English counterpart under
`src/content/docs/en/` with the same relative path.

## Cloudflare Pages

### Git integration

Create a Pages project from the repository and use:

| Setting | Value |
| --- | --- |
| Framework preset | Astro |
| Root directory | `/` (repository root) |
| Build command | `pnpm --filter @cobotrix/website build` |
| Build output directory | `website/dist` |
| Environment variable | `SITE_URL=https://your-domain.example` |
| Environment variable | `NODE_VERSION=22.23.4` (or newer) |

`SITE_URL` drives canonical URLs, `hreflang` alternates, and the sitemap.
Without it the build falls back to `https://cobotusr.pages.dev`.

### Wrangler CLI

```bash
pnpm --filter @cobotrix/website build
pnpm --filter @cobotrix/website exec wrangler pages deploy dist \
  --project-name cobotusr-website
```

Set `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` first. The project name
is also declared in `wrangler.jsonc`.

### Headers and redirects

- `public/_headers` sets security headers and long-lived caching for `/_astro/*`.
- `public/_redirects` maps the legacy `/docs/*` prefix to the quick start page.

## Checks

```bash
pnpm --filter @cobotrix/website check
pnpm --filter @cobotrix/website build
```
