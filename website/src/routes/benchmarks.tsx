import { createFileRoute } from '@tanstack/react-router'
import { BenchChart } from '../components/BenchChart'
import { DocsLayout } from '../components/DocsLayout'
import { MarkdownContent } from '../components/DocsPage'
import { CollapsibleSection } from '../components/CollapsibleSection'
import { expandTarget, Toc } from '../components/Toc'
import { extractHeadings } from '../lib/headings'
import { cacheLabel, compareText, corpusLabel, formatMs, formatRange, scenarioSummaries, speedup, summaryText, unmeasuredTools, type BenchView } from '../lib/benchmarks'
import { getBenchmarks } from '../lib/server'
import { breadcrumbs, seo } from '../lib/seo'
import methodology from '../content/benchmarks.md?raw'

function summarize(view: BenchView | undefined): string {
  const [cold] = view ? scenarioSummaries(view) : []
  return cold ? summaryText(cold) : 'Reproducible Dart linter benchmarks.'
}

const count = (n: number | undefined) => (n === undefined ? '–' : n.toLocaleString('en-US'))

const methodologySource = methodology.replace(/^# .*\n/, '').replace(/^(#{2,5}) /gm, '#$1 ')
const toc = [
  { id: 'summary-h', title: 'Summary' },
  { id: 'runs-h', title: 'Every run' },
  { id: 'output-h', title: 'What each tool reported' },
  { id: 'setup-h', title: 'Test setup' },
  { id: 'methodology', title: 'Methodology' },
  ...extractHeadings(methodologySource),
]

export const Route = createFileRoute('/benchmarks')({
  loader: () => getBenchmarks(),
  head: ({ loaderData }) =>
    seo({
      title: 'Benchmarks | Dart Falcon',
      description: `${summarize(loaderData)} Measured Dart and Flutter linter timings, versions and methodology.`,
      path: '/benchmarks',
      type: 'article',
      markdownPath: '/docs/benchmarks.md',
      jsonLd: [breadcrumbs([['Benchmarks', '/benchmarks']])],
    }),
  component: Benchmarks,
})

function Benchmarks() {
  const view = Route.useLoaderData()
  const { machine, tools, corpora, generatedAt, runs } = view
  const summaries = scenarioSummaries(view)
  const measured = new Date(generatedAt).toLocaleDateString('en-US', { dateStyle: 'long', timeZone: 'UTC' })
  const hasAnalyzer = runs.some((run) => (run.scenario.comparator ?? 'dart-analyze') === 'dart-analyze')
  const hasDcl = runs.some((run) => run.scenario.comparator === 'dcl')
  // Diagnostic captures differ by rule family, but cold and warm share the same output.
  const outputs = runs.filter((run, i) => runs.findIndex((other) =>
    other.corpus.id === run.corpus.id &&
    other.scenario.comparator === run.scenario.comparator,
  ) === i)

  return (
    <DocsLayout aside={<Toc items={toc} />}>
      <header className="page-head">
        <h1>Dart linter benchmarks</h1>
        <p className="lede">
          Repeated fresh-process timings on one machine. Measured {measured}.
        </p>
      </header>

      <CollapsibleSection id="summary-h" title="Summary" className="rule-section">
        <div className="table-scroll">
          <table>
            <caption>falcon relative to each baseline. dart analyze also type-checks; falcon does not.</caption>
            <thead>
              <tr>
                <th scope="col">Baseline</th>
                <th scope="col">Cache</th>
                <th scope="col">Result</th>
              </tr>
            </thead>
            <tbody>
              {summaries.map((s) => (
                <tr key={s.scenario.id}>
                  <th scope="row">vs {speedup(s.comparableRuns[0]).versus.name}</th>
                  <td>{cacheLabel(s.scenario)}</td>
                  <td>
                    {s.slower.length === 0 ? (
                      <><strong>{formatRange(s.min, s.max)} faster</strong> · {s.comparableRuns.length} of {corpora.length} {corpora.length === 1 ? 'corpus' : 'corpora'}</>
                    ) : (
                      <ul className="bench-results">
                        {s.comparableRuns.map((run) => (
                          <li key={run.corpus.id}>{corpusLabel(run.corpus)}: <strong>{compareText(run).ratio} {speedup(run).factor >= 1 ? 'faster' : 'slower'}</strong></li>
                        ))}
                      </ul>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="note">dart analyze honors its own ignore comments; falcon does not read analyzer suppressions.</p>
        <details className="bench-disclosure">
          <summary>How to read these results</summary>
          <ul>
            <li>Matching rule names do not mean identical work or diagnostic output.</li>
            <li>Cold and warm describe the analyzer cache, not a reused process. Every timed run starts a fresh process.</li>
            <li>The OS file cache stays warm in both scenarios.</li>
            {hasDcl ? <li>DCL-family speedups use Dart Code Linter as the baseline. DCM is omitted above its Free 50k-line limit; DCL is omitted when a corpus cannot fit one DCL root. Each omission is labeled below, never timed.</li> : null}
            <li>Ranges cover matched corpora only. See <a href="#methodology" onClick={() => expandTarget('methodology')}>methodology</a> for commands, scope and limitations.</li>
          </ul>
        </details>
      </CollapsibleSection>

      <CollapsibleSection id="runs-h" title="Every run" className="rule-section">
        <p className="note">Mean wall-clock time. Each cold/warm pair shares one time scale.</p>
        <div className="bench-list">
          {corpora.flatMap((corpus) => {
            const corpusRuns = runs.filter((run) => run.corpus.id === corpus.id)
            const families = [...new Set(corpusRuns.map((run) => run.scenario.comparator ?? 'dart-analyze'))]
            return families.map((family) => {
              const paired = corpusRuns.filter((run) => (run.scenario.comparator ?? 'dart-analyze') === family)
              const maxMs = Math.max(...paired.flatMap((run) => run.rows.map((row) => row.meanMs)))
              return (
                <section key={`${corpus.id}-${family}`} className="bench-run-group">
                  <h3>{corpusLabel(corpus)} <span>· {family === 'dcl' ? 'DCL rules' : 'Official Dart lints'}</span></h3>
                  <div className="bench-paired">
                    {paired.map((run) => (
                      <BenchChart key={run.scenario.id} run={run} maxMs={maxMs} title={`${cacheLabel(run.scenario)} analyzer cache`} />
                    ))}
                  </div>
                </section>
              )
            })
          })}
        </div>
        <details className="bench-disclosure">
          <summary>All timings, spread and memory measurements</summary>
        <div className="table-scroll">
          <table>
            <caption>
              Wall-clock time in ms (hyperfine) and largest-process peak RSS
              (GNU time <code>%M</code>, one separate untimed run)
            </caption>
            <thead>
              <tr>
                <th scope="col">Corpus</th>
                <th scope="col">Scenario</th>
                <th scope="col">Tool</th>
                <th scope="col" className="num">Mean</th>
                <th scope="col" className="num">± σ</th>
                <th scope="col" className="num">Min</th>
                <th scope="col" className="num">Max</th>
                <th scope="col" className="num">Runs</th>
                <th scope="col" className="num">Peak RSS</th>
              </tr>
            </thead>
            <tbody>
              {runs.flatMap((run) =>
                [...run.rows.map((r) => (
                  <tr key={`${run.corpus.id}-${run.scenario.id}-${r.tool}`}>
                    <th scope="row">{corpusLabel(run.corpus)}</th>
                    <td>{run.scenario.title}</td>
                    <td>{r.name}</td>
                    <td className="num">{formatMs(r.meanMs)}</td>
                    <td className="num">{formatMs(r.stddevMs)}</td>
                    <td className="num">{formatMs(r.minMs)}</td>
                    <td className="num">{formatMs(r.maxMs)}</td>
                    <td className="num">{r.runs}</td>
                    <td className="num">{r.peakRssMb === undefined ? '–' : `${Math.round(r.peakRssMb)} MB`}</td>
                  </tr>
                )), ...unmeasuredTools(run).map(({ tool, reason }) => (
                  <tr key={`${run.corpus.id}-${run.scenario.id}-${tool}`}>
                    <th scope="row">{corpusLabel(run.corpus)}</th>
                    <td>{run.scenario.title}</td>
                    <td>{tool.toUpperCase()}</td>
                    <td colSpan={6}>Not measured — {reason}</td>
                  </tr>
                ))],
              )}
            </tbody>
          </table>
        </div>
        </details>
      </CollapsibleSection>

      <CollapsibleSection id="output-h" title="What each tool reported" className="rule-section" defaultExpanded={false}>
        <p>
          Diagnostics are grouped by corpus and rule family; cold and warm runs
          use the same rule configuration.{' '}
          {hasAnalyzer ? (
            <>
              For <code>dart analyze</code>, “Other” and “Error severity” include
              compilation and type errors.{' '}
            </>
          ) : null}
          {hasDcl ? (
            <>
              DCL scenarios report the selected DCL rules for falcon, DCL and DCM
              where measured. “Error severity” counts rule issues marked as errors
              for DCL and DCM, so these can overlap “Lint”.{' '}
            </>
          ) : null}
          For falcon, “Error severity” counts syntax errors, not lint severity.{' '}
          Matching rule names are not evidence of diagnostic parity; the{' '}
          <a href="#methodology" onClick={() => expandTarget('methodology')}>methodology</a> explains differences in output.
        </p>
        <div className="table-scroll">
          <table>
            <caption>Diagnostics per corpus and rule family</caption>
            <thead>
              <tr>
                <th scope="col">Corpus</th>
                <th scope="col">Rule family</th>
                <th scope="col">Tool</th>
                <th scope="col" className="num">Lint</th>
                <th scope="col" className="num">Other</th>
                <th scope="col" className="num">Error severity</th>
                <th scope="col" className="num">Total</th>
              </tr>
            </thead>
            <tbody>
              {outputs.flatMap((run) =>
                [...run.rows.map((r) => (
                  <tr key={`${run.corpus.id}-${run.scenario.id}-${r.tool}`}>
                    <th scope="row">{corpusLabel(run.corpus)}</th>
                    <td>{run.scenario.comparator === 'dcl' ? 'DCL rules' : 'Official Dart lints'}</td>
                    <td>{r.name}</td>
                    <td className="num">{count(r.lintDiagnostics)}</td>
                    <td className="num">{count(r.nonLintDiagnostics)}</td>
                    <td className="num">{count(r.errorDiagnostics)}</td>
                    <td className="num">{count(r.diagnostics)}</td>
                  </tr>
                )), ...unmeasuredTools(run).map(({ tool, reason }) => (
                  <tr key={`${run.corpus.id}-${run.scenario.id}-${tool}`}>
                    <th scope="row">{corpusLabel(run.corpus)}</th>
                    <td>DCL rules</td>
                    <td>{tool.toUpperCase()}</td>
                    <td colSpan={4}>Not measured — {reason}</td>
                  </tr>
                ))],
              )}
            </tbody>
          </table>
        </div>
      </CollapsibleSection>

      <CollapsibleSection id="setup-h" title="Test setup" className="rule-section">
        <dl className="facts">
          <div>
            <dt>CPU</dt>
            <dd>
              {machine.cpu}, {machine.cores} threads
            </dd>
          </div>
          <div>
            <dt>Memory</dt>
            <dd>{machine.memoryGb.toFixed(1)} GB</dd>
          </div>
          <div>
            <dt>OS</dt>
            <dd>{machine.os}</dd>
          </div>
          {tools.map((t) => (
            <div key={t.id}>
              <dt>{t.name}</dt>
              <dd>
                <code>{t.version}</code>
              </dd>
            </div>
          ))}
        </dl>
        <div className="table-scroll">
          <table>
            <caption>
              Corpora, pinned to exact commits
              {hasDcl ? '; DCL-family counts exclude generated files' : ''}
            </caption>
            <thead>
              <tr>
                <th scope="col">Corpus</th>
                <th scope="col">Commit</th>
                <th scope="col" className="num">Scoped Dart files</th>
                <th scope="col" className="num">Scoped lines</th>
                {hasDcl ? <th scope="col" className="num">DCL-family files</th> : null}
                {hasDcl ? <th scope="col" className="num">DCL-family lines</th> : null}
              </tr>
            </thead>
            <tbody>
              {corpora.map((c) => (
                <tr key={c.id}>
                  <th scope="row">
                    <a href={c.url}>{c.name}</a>
                    {c.scope.length > 0 ? (
                      <small className="scope"> {c.scope.join(', ')}</small>
                    ) : null}
                  </th>
                  <td>
                    <a href={`${c.url}/tree/${c.rev}`} title={c.rev}>
                      <code>{c.rev.slice(0, 12)}</code>
                    </a>
                  </td>
                  <td className="num">{c.files.toLocaleString('en-US')}</td>
                  <td className="num">{c.lines.toLocaleString('en-US')}</td>
                  {hasDcl ? <td className="num">{count(c.dclFiles)}</td> : null}
                  {hasDcl ? <td className="num">{count(c.dclLines)}</td> : null}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CollapsibleSection>

      <CollapsibleSection id="methodology" title="Methodology" className="rule-section" defaultExpanded={false}>
        <div className="prose">
          <MarkdownContent source={methodologySource} />
        </div>
      </CollapsibleSection>
    </DocsLayout>
  )
}
