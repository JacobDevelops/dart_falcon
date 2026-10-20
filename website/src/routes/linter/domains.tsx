import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import { RuleRow } from '../../components/RuleRow'
import { getDomains } from '../../lib/server'
import { breadcrumbs, seo } from '../../lib/seo'

export const Route = createFileRoute('/linter/domains')({
  loader: () => getDomains(),
  head: ({ loaderData }) =>
    seo({
      title: 'Flutter, Bloc, Riverpod & Provider lint rules by domain | falcon',
      description: `Framework lint rules for Flutter, Bloc, Riverpod, Provider, flutter_hooks, Equatable and tests, grouped into ${loaderData?.length ?? ''} domains you can switch on together in falcon.json.`,
      path: '/linter/domains',
      jsonLd: [breadcrumbs([['Rules', '/linter/rules'], ['Domains', '/linter/domains']])],
    }),
  component: Domains,
})

const DOMAIN_BLURBS: Record<string, string> = {
  flutter: 'Widget construction, lifecycle, and Flutter framework APIs.',
  test: 'Dart and Flutter test suites: assertions, matchers, and test structure.',
  bloc: 'Blocs and Cubits from package:bloc: events, states, and emit discipline.',
  riverpod: 'Providers and refs from package:riverpod and flutter_riverpod.',
  provider: 'package:provider: reading, watching, and disposing providers.',
  flutter_hooks: 'Hook rules for package:flutter_hooks.',
  equatable: 'Value equality with package:equatable.',
}

function Domains() {
  const domains = Route.useLoaderData()
  return (
    <DocsLayout
      aside={
        <nav className="toc" aria-label="Domains">
          <p className="side-heading">On this page</p>
          {domains.map((d) => (
            <a href={`#${d.name}`} key={d.name}>
              {d.name} <span className="num">{d.rules.length}</span>
            </a>
          ))}
        </nav>
      }
    >
      <header className="page-head">
        <h1>Lint rules by framework domain</h1>
        <p className="lede">
          Domains group rules by the package they apply to. Turn a whole domain
          on in <code>falcon.json</code> under <code>linter.domains</code>; see
          the <Link to="/reference/configuration">configuration reference</Link>.
        </p>
      </header>

      {domains.map((domain) => (
        <section className="group-block" id={domain.name} key={domain.name} aria-labelledby={`${domain.name}-h`}>
          <div className="group-title">
            <h2 id={`${domain.name}-h`}>
              <code>{domain.name}</code>
            </h2>
            <span className="num">{domain.rules.length} rules</span>
          </div>
          {DOMAIN_BLURBS[domain.name] ? <p className="blurb">{DOMAIN_BLURBS[domain.name]}</p> : null}
          <ul className="rule-list">
            {domain.rules.map((r) => (
              <RuleRow key={r.name} rule={r} />
            ))}
          </ul>
        </section>
      ))}
    </DocsLayout>
  )
}
