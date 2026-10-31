import { createFileRoute, Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'
import { DocsLayout } from '../components/DocsLayout'
import { CollapsibleSection } from '../components/CollapsibleSection'
import { Toc } from '../components/Toc'
import { corpusLabel, formatSpeedup, headlineSummary, scenarioSummaries, unmeasuredTools, type ScenarioSummary } from '../lib/benchmarks'
import { getCompareData } from '../lib/server'
import { breadcrumbs, seo } from '../lib/seo'

const SOURCES = {
  dartAnalyze: 'https://dart.dev/tools/dart-analyze',
  dartAnalysis: 'https://dart.dev/tools/analysis',
  dartLints: 'https://dart.dev/tools/linter-rules',
  dcmPricing: 'https://dcm.dev/pricing/',
  dcmConfig: 'https://dcm.dev/docs/configuration/',
  customLint: 'https://github.com/invertase/dart_custom_lint',
  customLintPub: 'https://pub.dev/packages/custom_lint',
  serverPlugin: 'https://pub.dev/packages/analysis_server_plugin',
}

const FAQ: { q: string; a: string; includeInSchema?: boolean; link?: { to: '/docs/suppressions'; label: string } }[] = [
  {
    q: 'Is falcon a replacement for dart analyze?',
    a: 'No, it complements it. dart analyze runs the full Dart analyzer, which type-checks your code and reports compile errors; keep it (or your IDE) for that. falcon runs lint rules, including the official Dart lints under their analyzer names, but it does not read // ignore: or // ignore_for_file: comments. It uses its own falcon-ignore comments, each with a required reason, so lint counts from the two tools can differ on the same code.',
    link: { to: '/docs/suppressions', label: 'How falcon suppressions work' },
  },
  {
    q: 'Does falcon need the Dart SDK?',
    a: 'falcon parses Dart itself and never starts the Dart analyzer or analysis server. Rules that need SDK or package types need the Dart SDK installed and your dependencies resolved.',
    includeInSchema: false,
  },
  {
    q: 'Can I migrate my analysis_options.yaml?',
    a: 'Yes. falcon migrate reads the analyzer linter.rules list, dart_code_linter.rules, and legacy custom_lint.rules from analysis_options.yaml and writes an equivalent falcon.json. Rules with no falcon equivalent are reported on stderr.',
  },
  {
    q: 'Is falcon free?',
    a: 'Yes. falcon and every one of its rules are MIT licensed, with no seat or lines-of-code limits.',
    includeInSchema: false,
  },
]

export const Route = createFileRoute('/compare')({
  loader: () => getCompareData(),
  head: ({ loaderData }) =>
    seo({
      title: 'Compare Dart linters | Dart Falcon',
      description: `Compare falcon with dart analyze, DCM and custom_lint: ${loaderData?.coverage.total ?? ''} free Dart lint rules, Flutter support, configuration and sourced benchmarks.`,
      path: '/compare',
      type: 'article',
      jsonLd: [
        breadcrumbs([['Compare', '/compare']]),
        {
          '@type': 'FAQPage',
          mainEntity: FAQ.filter(({ includeInSchema }) => includeInSchema !== false).map(({ q, a }) => ({
            '@type': 'Question',
            name: q,
            acceptedAnswer: { '@type': 'Answer', text: a },
          })),
        },
      ],
    }),
  component: Compare,
})

function Cite({ href, n }: { href: string; n: number }) {
  return (
    <sup className="cite">
      <a href={href} aria-label={`Source ${n}`}>
        [{n}]
      </a>
    </sup>
  )
}

const toc = [
  { id: 'performance-h', title: 'Measured performance' },
  { id: 'comparison-h', title: 'At a glance' },
  { id: 'alt-h', title: 'When to pick which' },
  { id: 'faq-h', title: 'Questions' },
  { id: 'src-h', title: 'Sources' },
]
const tocWithoutPerformance = toc.slice(1)

function Compare() {
  const { coverage, bench } = Route.useLoaderData()
  const summaries = scenarioSummaries(bench)
  const notMeasured = <span className="na">not measured</span>
  const rssRange = (s: ScenarioSummary, tool: string): ReactNode => {
    const rss = s.runs.flatMap((run) => run.rows.flatMap((r) => (r.tool === tool && r.peakRssMb !== undefined ? [r.peakRssMb] : [])))
    return rss.length > 0 ? `${Math.round(Math.min(...rss))}–${Math.round(Math.max(...rss))} MB` : notMeasured
  }
  const benchmarkTools = [
    { id: 'falcon', name: 'falcon' },
    { id: 'dart-analyze', name: 'dart analyze' },
    { id: 'dcl', name: 'Dart Code Linter' },
    { id: 'dcm', name: 'DCM' },
  ]
  const relativeSpeed = (s: ScenarioSummary, tool: string): ReactNode => {
    const ratios = s.runs.flatMap((run) => {
      const falcon = run.rows.find((r) => r.tool === 'falcon')
      const other = run.rows.find((r) => r.tool === tool)
      return falcon && other ? [other.meanMs / falcon.meanMs] : []
    })
    if (ratios.length === 0) return notMeasured
    if (tool === 'falcon') return 'Baseline'
    const factor = Math.exp(ratios.reduce((sum, ratio) => sum + Math.log(ratio), 0) / ratios.length)
    return `${formatSpeedup(factor >= 1 ? factor : 1 / factor)} ${
      factor >= 1 ? 'slower' : 'faster'
    } than falcon (${ratios.length} matched ${ratios.length === 1 ? 'corpus' : 'corpora'})`
  }
  const rows: [string, ...ReactNode[]][] = [
    [
      'What it is',
      'Standalone linter and language server, written in Rust',
      <>The Dart SDK's analysis command: the same static analysis as your IDE<Cite href={SOURCES.dartAnalyze} n={1} /></>,
      <>Commercial code-quality toolkit for Dart and Flutter<Cite href={SOURCES.dcmPricing} n={3} /></>,
      <>A framework for writing your own lint rules as an analyzer plugin<Cite href={SOURCES.customLintPub} n={5} /></>,
    ],
    [
      'Lint rules',
      <><Link to="/linter/rules">{coverage.total} built in</Link>, {coverage.recommended} on by default</>,
      <>The official Dart linter rules<Cite href={SOURCES.dartLints} n={2} /></>,
      <>100 on Free, 552 on Pro and above<Cite href={SOURCES.dcmPricing} n={3} /></>,
      <>None built in; you write or install rule packages<Cite href={SOURCES.customLint} n={6} /></>,
    ],
    [
      'Price',
      'Free, MIT',
      'Free, part of the Dart SDK',
      <>Free for 1 seat up to 50k lines; Pro $16/month per seat; Teams $80/month for 5 seats<Cite href={SOURCES.dcmPricing} n={3} /></>,
      <>Free, Apache-2.0<Cite href={SOURCES.customLint} n={6} /></>,
    ],
    [
      'Configuration',
      <><code>falcon.json</code> with a JSON Schema</>,
      <><code>analysis_options.yaml</code><Cite href={SOURCES.dartAnalysis} n={7} /></>,
      <>A <code>dcm</code> entry in <code>analysis_options.yaml</code><Cite href={SOURCES.dcmConfig} n={4} /></>,
      <><code>analysis_options.yaml</code> analyzer plugin<Cite href={SOURCES.customLint} n={6} /></>,
    ],
    [
      'Status',
      'Actively developed, pre-1.0',
      'Maintained by the Dart team',
      <>Actively released<Cite href={SOURCES.dcmPricing} n={3} /></>,
      <>Repository archived; the README recommends <a href={SOURCES.serverPlugin}>analysis_server_plugin</a> instead<Cite href={SOURCES.customLint} n={6} /></>,
    ],
  ]

  return (
    <DocsLayout aside={<Toc items={summaries.length ? toc : tocWithoutPerformance} />}>
      <header className="page-head">
        <h1>Dart linter comparison: falcon, dart analyze, DCM and custom_lint</h1>
        <p className="lede">
          Looking for a DCM alternative or a custom_lint alternative? Compare
          falcon as a Dart linter and Flutter linter, including where a{' '}
          <code>dart analyze</code> alternative for linting still needs the
          analyzer for type checking. falcon's numbers come from its rule catalog
          and <Link to="/benchmarks">benchmarks</Link>; other claims link to public sources.
          {` ${headlineSummary(bench)}`}
        </p>
      </header>

      {summaries.length > 0 ? (
        <CollapsibleSection id="performance-h" title="Measured performance" className="rule-section">
          <div className="table-scroll">
            <table className="compare">
              <caption>Measured performance; geometric-mean time ratios relative to falcon</caption>
              <thead>
                <tr>
                  <th scope="col">Measurement</th>
                  {benchmarkTools.map((tool) => <th key={tool.id} scope="col">{tool.name}</th>)}
                </tr>
              </thead>
              <tbody>
                {summaries.flatMap((s) => [
                  <tr key={`${s.scenario.id}-speed`}>
                    <th scope="row">Speed, {s.scenario.title}</th>
                    {benchmarkTools.map((tool) => (
                      <td key={tool.id}>
                        {relativeSpeed(s, tool.id)}
                        {s.runs.flatMap((run) =>
                          unmeasuredTools(run).filter((entry) => entry.tool === tool.id).map((entry) => (
                            <small key={run.corpus.id} className="scope">
                              {corpusLabel(run.corpus)}: not measured — {entry.reason}.
                            </small>
                          )),
                        )}
                      </td>
                    ))}
                  </tr>,
                  <tr key={`${s.scenario.id}-rss`}>
                    <th scope="row">Largest-process peak RSS range, {s.scenario.title}</th>
                    {benchmarkTools.map((tool) => (
                      <td key={tool.id}>{rssRange(s, tool.id)}</td>
                    ))}
                  </tr>,
                ])}
              </tbody>
            </table>
          </div>
        </CollapsibleSection>
      ) : null}

      <CollapsibleSection id="comparison-h" title="At a glance" className="rule-section">
        <div className="table-scroll">
          <table className="compare">
            <caption>Dart and Flutter linters at a glance</caption>
            <thead>
              <tr>
                <th scope="col">
                  <span className="visually-hidden">Aspect</span>
                </th>
                <th scope="col" className="is-falcon">falcon</th>
                <th scope="col">dart analyze</th>
                <th scope="col">DCM</th>
                <th scope="col">custom_lint</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(([label, ...cells]) => (
                <tr key={label}>
                  <th scope="row">{label}</th>
                  {cells.map((c, i) => (
                    <td key={i} className={i === 0 ? 'is-falcon' : undefined}>
                      {c}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {summaries.length > 0 ? (
          <p className="note">
            Each timing is a mean of repeated runs. Speed cells use the geometric
            mean of per-corpus time ratios against falcon on the matched corpora
            shown; RSS cells show the range across measured corpora only. DCL is
            the DCL-family speedup baseline in the{' '}
            <Link to="/benchmarks">per-corpus charts and methodology</Link>, which
            also list the machine, versions, and commands.
          </p>
        ) : null}
      </CollapsibleSection>

      <CollapsibleSection id="alt-h" title="When to pick which" className="rule-section">
        <h3>A dart analyze alternative for lint rules</h3>
        <p>
          Keep <code>dart analyze</code> for type checking and compile errors,
          and add falcon for linting. falcon implements {coverage.mappedToOfficial} official Dart lints
          under their analyzer names, plus {coverage.total - coverage.mappedToOfficial} rules
          the official set doesn't have. Its rules run in falcon's own process,
          never inside the Dart analysis server.
        </p>
        <h3>A DCM alternative</h3>
        <p>
          {coverage.sources.find((s) => s.label === 'Dart Code Metrics')?.count ?? 0} falcon rules
          are ports of Dart Code Metrics rules, alongside cross-file checks like{' '}
          <Link to="/linter/rules/$rule" params={{ rule: 'unused-code' }}>unused-code</Link> and{' '}
          <Link to="/linter/rules/$rule" params={{ rule: 'unused-files' }}>unused-files</Link>. falcon has
          no seat or line limits. DCM also offers code metrics, dashboards, and
          CLI fixes on its paid plans<Cite href={SOURCES.dcmPricing} n={3} />.
        </p>
        <h3>A custom_lint alternative</h3>
        <p>
          custom_lint is a framework for your own rules. If you used it for
          community rule packages, falcon's{' '}
          <Link to="/linter/domains">framework domains</Link> cover{' '}
          {coverage.domains.map((d) => d.name).join(', ')} out of the box.
        </p>
      </CollapsibleSection>

      <CollapsibleSection id="faq-h" title="Questions" className="rule-section faq">
        {FAQ.map(({ q, a, link }) => (
          <details key={q}>
            <summary>{q}</summary>
            <p>
              {a}
              {link ? (
                <>
                  {' '}
                  <Link to={link.to}>{link.label}</Link>
                </>
              ) : null}
            </p>
          </details>
        ))}
      </CollapsibleSection>

      <CollapsibleSection id="src-h" title="Sources" className="rule-section">
        <p className="note">Competitor details checked on October 21, 2026.</p>
        <ol className="sources-list">
          <li><a href={SOURCES.dartAnalyze}>dart.dev: dart analyze</a></li>
          <li><a href={SOURCES.dartLints}>dart.dev: Linter rules</a></li>
          <li><a href={SOURCES.dcmPricing}>dcm.dev: Pricing</a></li>
          <li><a href={SOURCES.dcmConfig}>dcm.dev: Configuration</a></li>
          <li><a href={SOURCES.customLintPub}>pub.dev: custom_lint</a></li>
          <li><a href={SOURCES.customLint}>GitHub: invertase/dart_custom_lint</a></li>
          <li><a href={SOURCES.dartAnalysis}>dart.dev: Customizing static analysis</a></li>
        </ol>
      </CollapsibleSection>
    </DocsLayout>
  )
}
