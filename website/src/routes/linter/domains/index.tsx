import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../../components/DocsLayout'
import { DOMAIN_BLURBS, domainLabel } from '../../../lib/rules'
import { getDomains } from '../../../lib/server'
import { breadcrumbs, seo } from '../../../lib/seo'

export const Route = createFileRoute('/linter/domains/')({
  loader: () => getDomains(),
  head: ({ loaderData }) => seo({
    title: 'Rules by domain | Dart Falcon',
    description: `Framework lint rules for Flutter, Bloc, Riverpod, Provider, Flutter Hooks, Equatable and tests, grouped into ${loaderData?.length ?? ''} domains you can switch on together in falcon.json.`,
    path: '/linter/domains',
    jsonLd: [breadcrumbs([['Rules', '/linter/rules'], ['Domains', '/linter/domains']])],
  }),
  component: Domains,
})

function Domains() {
  const domains = Route.useLoaderData()
  return (
    <DocsLayout>
      <header className="page-head">
        <h1>Lint rules by framework domain</h1>
        <p className="lede">Choose the frameworks your project uses. Each domain groups relevant rules you can enable together in <code>falcon.json</code>.</p>
        <p>Configure <code>linter.domains</code> with the <Link to="/reference/configuration">configuration reference</Link>.</p>
      </header>
      <div className="catalog-cards">
        {domains.map((domain) => (
          <Link key={domain.name} className="catalog-card" to="/linter/domains/$domain" params={{ domain: domain.name }}>
            <h2>{domainLabel(domain.name)}</h2>
            <p>{DOMAIN_BLURBS[domain.name]}</p>
            <p className="catalog-card-meta">{domain.rules.length} rules · {domain.rules.filter((rule) => rule.recommended).length} recommended</p>
            <span>Browse {domainLabel(domain.name)} rules →</span>
          </Link>
        ))}
      </div>
    </DocsLayout>
  )
}
