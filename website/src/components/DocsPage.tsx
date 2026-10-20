import { Marked } from 'marked'
import { DocsLayout } from './DocsLayout'
import { REPO_URL } from '../lib/rules'

const slug = (text: string) =>
  text
    .toLowerCase()
    .replace(/<[^>]+>|&[a-z]+;/g, '')
    .replace(/[^\w\s-]/g, '')
    .trim()
    .replace(/\s/g, '-')

// The synced markdown links sibling docs as `./name.md`; point those at site
// routes and everything else repo-relative at GitHub.
const DOC_ROUTES: Record<string, string> = {
  'getting-started': '/docs/getting-started',
  installation: '/docs/installation',
  configuration: '/docs/configuration',
  suppressions: '/docs/suppressions',
  benchmarks: '/benchmarks',
}

function resolveHref(href: string): string {
  const doc = href.match(/^\.\/([\w-]+)\.md(#.*)?$/)
  if (doc && DOC_ROUTES[doc[1]]) return `${DOC_ROUTES[doc[1]]}${doc[2] ?? ''}`
  if (href.startsWith('../')) return `${REPO_URL}/blob/main/${href.slice(3)}`
  if (href.startsWith('./')) return `${REPO_URL}/blob/main/docs/${href.slice(2)}`
  return href
}

export const markdown = new Marked({
  renderer: {
    heading({ tokens, depth }) {
      const html = this.parser.parseInline(tokens)
      return `<h${depth} id="${slug(html)}">${html}</h${depth}>\n`
    },
    link({ href, tokens }) {
      return `<a href="${resolveHref(href)}">${this.parser.parseInline(tokens)}</a>`
    },
  },
})

export function DocsPage({ source }: { source: string }) {
  return (
    <DocsLayout>
      <article
        className="prose"
        dangerouslySetInnerHTML={{ __html: markdown.parse(source, { async: false }) }}
      />
    </DocsLayout>
  )
}
