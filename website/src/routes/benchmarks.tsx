import { createFileRoute } from '@tanstack/react-router'
import { BenchChart, BenchSummaryChart } from '../components/BenchChart'
import { DocsLayout } from '../components/DocsLayout'
import { markdown } from '../components/DocsPage'
import { corpusLabel, formatMs, scenarioSummaries, summaryText, unmeasuredTools, type BenchView } from '../lib/benchmarks'
import { getBenchmarks } from '../lib/server'
import { breadcrumbs, seo } from '../lib/seo'
import methodology from '../content/benchmarks.md?raw'

function summarize(view: BenchView | undefined): string {
  const [cold] = view ? scenarioSummaries(view) : []
  return cold ? summaryText(cold) : 'Reproducible Dart linter benchmarks.'
}

const count = (n: number | undefined) => (n === undefined ? '–' : n.toLocaleString('en-US'))

export const Route = createFileRoute('/benchmarks')({
  loader: () => getBenchmarks(),
  head: ({ loaderData }) =>
    seo({
      title: 'Dart and Flutter linter benchmarks | falcon',
      description: `${summarize(loaderData)} Wall-clock benchmarks of Dart and Flutter linters on real open-source codebases, with machine, versions, and commit SHAs.`,
      path: '/benchmarks',
      type: 'article',
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
    <DocsLayout>
      <header className="page-head">
        <h1>Dart linter benchmarks</h1>
        <p className="lede">
          {summaries.map(summaryText).join(' ')} Each bar is the mean of
          repeated fresh-process runs on one machine, measured {measured}.{' '}
          {hasAnalyzer ? (
            <>
              The dart analyze scenarios enable the same official lint names,
              but <code>dart analyze</code> also type-checks the code and honors
              Dart's own suppressions.{' '}
            </>
          ) : null}
          {hasDcl ? (
            <>
              The Dart Code Linter (DCL) scenarios compare falcon, DCL and DCM with
              the selected DCL rule names enabled; their speedup baseline is DCL,
              not dart analyze. DCM is not measured where the source subset exceeds
              the DCM Free 50k-line limit; those omissions are labeled, not timed.{' '}
            </>
          ) : null}
          Matching rule names do not imply identical work or output. The{' '}
          <a href="#methodology">methodology</a> says exactly what is timed.
        </p>
      </header>

      <div className="bench-list">
        {summaries.map((s) => (
          <BenchSummaryChart key={s.scenario.id} summary={s} />
        ))}
      </div>

      <section className="rule-section" aria-labelledby="runs-h">
        <h2 id="runs-h">Every run</h2>
        <div className="bench-list">
          {runs.map((run) => (
            <BenchChart key={`${run.corpus.id}-${run.scenario.id}`} run={run} detailed />
          ))}
        </div>
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
      </section>

      <section className="rule-section" aria-labelledby="output-h">
        <h2 id="output-h">What each tool reported</h2>
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
          <a href="#methodology">methodology</a> explains differences in output.
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
      </section>

      <section className="rule-section" aria-labelledby="setup-h">
        <h2 id="setup-h">Test setup</h2>
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
      </section>

      <section className="rule-section" aria-labelledby="methodology">
        <h2 id="methodology">Methodology</h2>
        <div
          className="prose"
          dangerouslySetInnerHTML={{
            // The synced doc's own h1 is dropped; its sections nest under this h2.
            __html: markdown.parse(methodology.replace(/^# .*\n/, '').replace(/^## /gm, '### '), {
              async: false,
            }),
          }}
        />
      </section>
    </DocsLayout>
  )
}
