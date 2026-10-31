import { REPO_URL } from './rules'

export const DOC_PAGES = [
  { name: 'getting-started', title: 'Getting started', route: '/docs/getting-started', description: 'Run falcon on your first Dart or Flutter project.' },
  { name: 'installation', title: 'Installation', route: '/docs/installation', description: 'Install falcon and configure your editor.' },
  { name: 'configuration', title: 'Configuration', route: '/docs/configuration', description: 'Configure rules, domains, severity, and project settings.' },
  { name: 'suppressions', title: 'Suppressions', route: '/docs/suppressions', description: 'Suppress diagnostics in source code and configuration.' },
  { name: 'benchmarks', title: 'Benchmarks', route: '/benchmarks', description: 'Performance measurements, methodology, and benchmark results.' },
] as const

const DOC_ROUTES: Record<string, string> = Object.fromEntries(DOC_PAGES.map((page) => [page.name, page.route]))

export function resolveDocHref(href: string): string {
  const doc = href.match(/^\.\/([\w-]+)\.md(#.*)?$/)
  if (doc && DOC_ROUTES[doc[1]]) return `${DOC_ROUTES[doc[1]]}${doc[2] ?? ''}`
  if (href.startsWith('../')) return `${REPO_URL}/blob/main/${href.slice(3)}`
  if (href.startsWith('./')) return `${REPO_URL}/blob/main/docs/${href.slice(2)}`
  return href
}
