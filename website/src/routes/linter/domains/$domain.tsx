import { createFileRoute, Link, notFound } from '@tanstack/react-router'
import { DocsLayout } from '../../../components/DocsLayout'
import { RuleCatalog, catalogSearch, type CatalogSearch } from '../../../components/RuleCatalog'
import { Toc } from '../../../components/Toc'
import { DOMAIN_BLURBS, domainLabel } from '../../../lib/rules'
import { getDomains } from '../../../lib/server'
import { breadcrumbs, seo } from '../../../lib/seo'

export const Route = createFileRoute('/linter/domains/$domain')({
  validateSearch: (search): CatalogSearch => ({ ...catalogSearch(search), domain: undefined }),
  loader: async ({ params }) => {
    const domains = await getDomains()
    const domain = domains.find((item) => item.name === params.domain)
    if (!domain) throw notFound()
    return domain
  },
  head: ({ loaderData }) => seo({
    title: `${domainLabel(loaderData?.name ?? '')} lint rules | Dart Falcon`,
    description: `${DOMAIN_BLURBS[loaderData?.name ?? ''] ?? ''} Explore ${loaderData?.rules.length ?? ''} framework-specific falcon rules for Dart with examples and configuration.`,
    path: `/linter/domains/${loaderData?.name ?? ''}`,
    jsonLd: [breadcrumbs([['Domains', '/linter/domains'], [domainLabel(loaderData?.name ?? ''), `/linter/domains/${loaderData?.name ?? ''}`]])],
  }),
  component: DomainRules,
})

function DomainRules() {
  const domain = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  return (
    <DocsLayout aside={<Toc items={[{ id: 'find-rules', title: 'Find a rule' }, { id: 'matching-rules', title: 'Rules' }]} />}>
      <nav className="crumbs" aria-label="Breadcrumb"><ol><li><Link to="/linter/domains">Domains</Link></li><li aria-current="page">{domainLabel(domain.name)}</li></ol></nav>
      <header className="page-head">
        <h1>{domainLabel(domain.name)} lint rules for Dart</h1>
        <p className="lede">{DOMAIN_BLURBS[domain.name]} {domain.rules.length} rules.</p>
        <details className="catalog-disclosure">
          <summary>Configure {domainLabel(domain.name)} rules</summary>
          <p>Recommended domain rules run by default. Set <code>linter.domains.{domain.name}</code> to <code>"all"</code> to include opt-in rules, or <code>"none"</code> to disable this domain. <Link to="/reference/configuration">Configuration reference</Link></p>
        </details>
      </header>
      <RuleCatalog rules={domain.rules} search={search} domain={domain.name} onChange={(next, replace) => { void navigate({ search: next, replace, resetScroll: false }) }} />
    </DocsLayout>
  )
}
