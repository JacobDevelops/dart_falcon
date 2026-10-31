import { createFileRoute } from '@tanstack/react-router'
import { domains, rules } from '../lib/rules.data'
import { GROUPS } from '../lib/rules'
import lastmodJson from '../generated/lastmod.json'
import { SITE_URL } from '../lib/seo'
import { DOC_PAGES } from '../lib/docs'

const lastmod: Record<string, string> = lastmodJson

const PAGES = [
  '/',
  '/compare',
  ...DOC_PAGES.map((page) => page.route),
  '/linter/rules',
  '/linter/domains',
  '/reference/cli',
  '/reference/configuration',
]

export const Route = createFileRoute('/sitemap.xml')({
  server: {
    handlers: {
      GET: () => {
        const urls = [
          ...PAGES,
          ...GROUPS.map((g) => `/linter/groups/${g.id}`),
          ...domains.map((d) => `/linter/domains/${d.name}`),
          ...rules.map((r) => `/linter/rules/${r.name}`),
        ]
        const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls.map((u) => `  <url><loc>${SITE_URL}${u}</loc>${lastmod[u] ? `<lastmod>${lastmod[u]}</lastmod>` : ''}</url>`).join('\n')}
</urlset>
`
        return new Response(body, {
          headers: {
            'Content-Type': 'application/xml; charset=utf-8',
            'Cache-Control': 'public, max-age=3600',
          },
        })
      },
    },
  },
})
