import { createFileRoute, Link, notFound } from '@tanstack/react-router'
import { DocsLayout } from '../../../components/DocsLayout'
import { RuleCatalog, catalogSearch, type CatalogSearch } from '../../../components/RuleCatalog'
import { Toc } from '../../../components/Toc'
import { getRulesIndex } from '../../../lib/server'
import { breadcrumbs, seo } from '../../../lib/seo'

export const Route = createFileRoute('/linter/groups/$group')({
  validateSearch: (search): CatalogSearch => ({ ...catalogSearch(search), group: undefined }),
  loader: async ({ params }) => {
    const { rules, coverage } = await getRulesIndex()
    const group = coverage.groups.find((item) => item.id === params.group)
    if (!group) throw notFound()
    return { group, rules: rules.filter((rule) => rule.group === group.id) }
  },
  head: ({ loaderData }) => seo({
    title: `${loaderData?.group.label ?? ''} rules | Dart Falcon`,
    description: `${loaderData?.group.blurb ?? ''} Browse ${loaderData?.group.count ?? ''} Dart and Flutter lint rules with examples, including ${loaderData?.group.recommended ?? ''} recommended rules.`,
    path: `/linter/groups/${loaderData?.group.id ?? ''}`,
    jsonLd: [breadcrumbs([['Rules', '/linter/rules'], [loaderData?.group.label ?? 'Group', `/linter/groups/${loaderData?.group.id ?? ''}`]])],
  }),
  component: RuleGroup,
})

function RuleGroup() {
  const { group, rules } = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  return (
    <DocsLayout aside={<Toc items={[{ id: 'find-rules', title: 'Find a rule' }, { id: 'matching-rules', title: 'Rules' }]} />}>
      <nav className="crumbs" aria-label="Breadcrumb"><ol><li><Link to="/linter/rules">Rules</Link></li><li aria-current="page">{group.label}</li></ol></nav>
      <header className="page-head"><h1>Dart {group.label.toLowerCase()} lint rules</h1><p className="lede">{group.blurb} {group.count} rules · {group.recommended} recommended.</p></header>
      <RuleCatalog rules={rules} search={search} group={group.id} onChange={(next, replace) => { void navigate({ search: next, replace, resetScroll: false }) }} />
    </DocsLayout>
  )
}
