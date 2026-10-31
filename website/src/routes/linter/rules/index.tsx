import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../../components/DocsLayout'
import { RuleCatalog, catalogSearch } from '../../../components/RuleCatalog'
import { CollapsibleSection } from '../../../components/CollapsibleSection'
import { Toc } from '../../../components/Toc'
import { domainLabel } from '../../../lib/rules'
import { getRulesIndex } from '../../../lib/server'
import { breadcrumbs, seo } from '../../../lib/seo'

export const Route = createFileRoute('/linter/rules/')({
  validateSearch: catalogSearch,
  loader: () => getRulesIndex(),
  head: ({ loaderData }) => seo({
    title: 'Dart lint rules | Dart Falcon',
    description: `Explore ${loaderData?.coverage.total ?? ''} falcon lint rules for Dart and Flutter by group, framework, and recommended defaults, with diagnostics and examples.`,
    path: '/linter/rules',
    jsonLd: [breadcrumbs([['Rules', '/linter/rules']])],
  }),
  component: RulesIndex,
})

function RulesIndex() {
  const { rules, coverage } = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  return (
    <DocsLayout aside={<Toc items={[{ id: 'rule-groups-heading', title: 'Browse by group' }, { id: 'frameworks', title: 'Browse by domain' }, { id: 'find-rules', title: 'Find a rule' }, { id: 'matching-rules', title: 'Rules' }]} />}>
      <header className="page-head">
        <h1>Dart &amp; Flutter lint rules</h1>
        <p className="lede">{coverage.total} rules, {coverage.recommended} recommended by default. Start with a group, browse your framework or package, or find the exact rule you need.</p>
        <p><a href="#find-rules">Search and filter all rules →</a></p>
      </header>
      <CollapsibleSection id="rule-groups-heading" title="Browse by group" className="catalog-overview">
        <div className="catalog-cards">
          {coverage.groups.map((group) => (
            <Link className="catalog-card" key={group.id} to="/linter/groups/$group" params={{ group: group.id }}>
              <h3>{group.label}<span className="num">{group.count}</span></h3>
              <p>{group.blurb}</p>
              <span className="catalog-card-meta">{group.recommended} recommended</span>
            </Link>
          ))}
        </div>
      </CollapsibleSection>
      <CollapsibleSection id="frameworks" title="Browse by domain" className="catalog-domains">
        <div>{coverage.domains.map((domain) => <Link key={domain.name} to="/linter/domains/$domain" params={{ domain: domain.name }}>{domainLabel(domain.name)} <span className="num">{domain.count}</span></Link>)}</div>
      </CollapsibleSection>
      <RuleCatalog rules={rules} search={search} onChange={(next, replace) => { void navigate({ search: next, replace, resetScroll: false }) }} overview />
    </DocsLayout>
  )
}
