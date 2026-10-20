import { createFileRoute } from '@tanstack/react-router'
import { useDeferredValue, useState } from 'react'
import { DocsLayout } from '../../../components/DocsLayout'
import { RuleRow } from '../../../components/RuleRow'
import { GROUPS, type RuleSummary } from '../../../lib/rules'
import { getRulesIndex } from '../../../lib/server'
import { breadcrumbs, seo } from '../../../lib/seo'

export const Route = createFileRoute('/linter/rules/')({
  loader: () => getRulesIndex(),
  head: ({ loaderData }) =>
    seo({
      title: `Dart & Flutter lint rules: all ${loaderData?.coverage.total ?? ''} rules | falcon`,
      description: `Every falcon lint rule for Dart and Flutter: ${loaderData?.coverage.total} rules across correctness, style, performance, security and more, each with real diagnostics, examples, and its dart analyze or DCM name.`,
      path: '/linter/rules',
      jsonLd: [breadcrumbs([['Rules', '/linter/rules']])],
    }),
  component: RulesIndex,
})

function matches(rule: RuleSummary, q: string): boolean {
  return [rule.name, rule.upstreamId ?? '', rule.description].some((s) => s.toLowerCase().includes(q))
}

function RulesIndex() {
  const { rules, coverage } = Route.useLoaderData()
  const [query, setQuery] = useState('')
  const raw = useDeferredValue(query).trim().toLowerCase()
  const q = raw.replace(/_/g, '-')
  const hits = q ? rules.filter((r) => matches(r, q) || matches(r, raw)) : []
  const sections = [
    ...GROUPS.map((g) => ({ ...g, rules: rules.filter((r) => r.group === g.id && !r.crossFile) })),
    {
      id: 'cross-file',
      label: 'Cross-file',
      blurb: "Rules that analyze relationships between files, run in falcon's whole-project pass.",
      rules: rules.filter((r) => r.crossFile),
    },
  ]

  return (
    <DocsLayout
      aside={
        <nav className="toc" aria-label="Rule groups">
          <p className="side-heading">On this page</p>
          {sections.map((s) => (
            <a href={`#${s.id}`} key={s.id}>
              {s.label} <span className="num">{s.rules.length}</span>
            </a>
          ))}
        </nav>
      }
    >
      <header className="page-head">
        <h1>Dart &amp; Flutter lint rules</h1>
        <p className="lede">
          {coverage.total} rules, {coverage.recommended} on by default.{' '}
          {coverage.mappedToOfficial} carry an official Dart lint name, shown
          beside the falcon id. Every example below is falcon's real output.
        </p>
        <div className="search">
          <label htmlFor="rule-search">Find a rule</label>
          <input
            id="rule-search"
            type="search"
            placeholder="avoid_print, unused-code, setState…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoComplete="off"
            spellCheck={false}
          />
          <p className="search-count" aria-live="polite">
            {q ? `${hits.length} ${hits.length === 1 ? 'rule matches' : 'rules match'}` : ''}
          </p>
        </div>
      </header>

      {q ? (
        <ul className="rule-list">
          {hits.map((r) => (
            <RuleRow key={r.name} rule={r} />
          ))}
        </ul>
      ) : (
        sections.map((s) => (
          <section className="group-block" id={s.id} key={s.id} aria-labelledby={`${s.id}-h`}>
            <div className="group-title">
              <h2 id={`${s.id}-h`}>{s.label}</h2>
              <span className="num">
                {s.rules.length} rules · {s.rules.filter((r) => r.recommended).length} recommended
              </span>
            </div>
            <p className="blurb">{s.blurb}</p>
            <ul className="rule-list">
              {s.rules.map((r) => (
                <RuleRow key={r.name} rule={r} />
              ))}
            </ul>
          </section>
        ))
      )}
    </DocsLayout>
  )
}
