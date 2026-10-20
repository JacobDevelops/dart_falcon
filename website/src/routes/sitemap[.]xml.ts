import { createFileRoute } from '@tanstack/react-router'
import { rules } from '../lib/rules.data'
import { SITE_URL } from '../lib/seo'

const PAGES = [
  '/',
  '/benchmarks',
  '/compare',
  '/docs/getting-started',
  '/docs/installation',
  '/docs/configuration',
  '/docs/suppressions',
  '/linter/rules',
  '/linter/domains',
  '/reference/cli',
  '/reference/configuration',
]

export const Route = createFileRoute('/sitemap.xml')({
  server: {
    handlers: {
      GET: () => {
        const urls = [...PAGES, ...rules.map((r) => `/linter/rules/${r.name}`)]
        const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls.map((u) => `  <url><loc>${SITE_URL}${u}</loc></url>`).join('\n')}
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
