import type { CSSProperties } from 'react'
import {
  compareText,
  corpusLabel,
  formatMs,
  formatRange,
  speedup,
  unmeasuredTools,
  type BenchRun,
  type ScenarioSummary,
} from '../lib/benchmarks'

/** Round tick step (1, 2 or 5 × 10^k) giving at most five ticks up to `max`. */
function ticks(max: number): number[] {
  const raw = max / 4
  const mag = 10 ** Math.floor(Math.log10(raw))
  const step = [1, 2, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw
  return Array.from({ length: Math.floor(max / step) + 1 }, (_, i) => i * step)
}

/**
 * Horizontal wall-clock bars for one corpus × scenario, fastest first, with
 * falcon's ratio against the scenario comparator above the bars. `detailed` adds spread and run counts.
 */
export function BenchChart({
  run,
  detailed = false,
}: {
  run: BenchRun
  detailed?: boolean
}) {
  const max = run.rows[run.rows.length - 1].meanMs
  const compared =
    run.rows.some((r) => r.tool === 'falcon') && speedup(run).versus.tool !== 'falcon'
      ? compareText(run)
      : null
  const dcl = run.scenario.comparator === 'dcl'
  const files = dcl ? run.corpus.dclFiles : run.corpus.files
  const lines = dcl ? run.corpus.dclLines : run.corpus.lines
  return (
    <figure className="bench">
      <figcaption className="bench-cap">
        <h2 className="bench-title">
          {run.scenario.title} <span>on {corpusLabel(run.corpus)}</span>
        </h2>
        <span className="bench-corpus">
          <a href={run.corpus.url}>{run.corpus.name}</a>
          {run.corpus.scope.length > 0
            ? ` (${run.corpus.scope.map((d) => d.split('/').pop()).join(', ')} only)`
            : ''}
          {' · '}
          {files?.toLocaleString('en-US') ?? 'unknown'} files ·{' '}
          {lines?.toLocaleString('en-US') ?? 'unknown'} lines
        </span>
        {detailed ? <span className="bench-desc">{run.scenario.description}</span> : null}
      </figcaption>
      {compared ? (
        <p className="bench-callout">
          <strong>{compared.ratio}</strong> {compared.text}
        </p>
      ) : null}
      <ol className="bench-rows">
        {run.rows.map((r, i) => (
          <li
            key={r.tool}
            className={`bench-row is-${r.tool}`}
            style={{ '--w': `${(r.meanMs / max) * 100}%`, '--i': i } as CSSProperties}
          >
            <span className="bench-tool">
              {r.name}
              {r.version ? <small> {r.version}</small> : null}
            </span>
            <span className="bench-track" aria-hidden="true">
              <span className="bench-bar" />
            </span>
            <span className="bench-time">{formatMs(r.meanMs)}</span>
            {detailed ? (
              <small className="bench-detail">
                ± {formatMs(r.stddevMs)} over {r.runs} runs
                {r.peakRssMb !== undefined ? ` · ${Math.round(r.peakRssMb)} MB peak RSS` : ''}
              </small>
            ) : null}
          </li>
        ))}
      </ol>
      <div className="bench-scale" aria-hidden="true">
        {ticks(max).map((t) => (
          <span key={t} style={{ '--x': `${(t / max) * 100}%` } as CSSProperties}>
            {formatMs(t)}
          </span>
        ))}
      </div>
      {unmeasuredTools(run).map(({ tool, reason }) => (
        <p key={tool} className="bench-desc">
          {tool.toUpperCase()}: not measured — {reason}.
        </p>
      ))}
    </figure>
  )
}

/**
 * One scenario across every corpus: all measured tools scaled to that corpus's
 * slowest tool, with falcon's speedup over the scenario comparator as the callout.
 */
export function BenchSummaryChart({ summary }: { summary: ScenarioSummary }) {
  const { scenario, runs, min, max, slower } = summary
  const versus = speedup(runs[0]).versus
  const tools = [...new Map(runs.flatMap((run) => run.rows.map((row) => [row.tool, row] as const))).values()]
    .sort((a, b) => Number(b.tool === 'falcon') - Number(a.tool === 'falcon'))
  return (
    <figure className="bench">
      <figcaption className="bench-cap">
        <h2 className="bench-title">
          {tools.map((tool) => tool.name).join(' vs ')} <span>{scenario.title}</span>
        </h2>
        <span className="bench-desc">
          {tools.map((tool) => (
            <span key={tool.tool}>
              <span className={`key key-${tool.tool}`} aria-hidden="true" />{' '}
              {tool.name} {tool.version}{' · '}
            </span>
          ))}
          mean wall-clock time per corpus; speedups vs {versus.name}
        </span>
      </figcaption>
      {slower.length === 0 ? (
        <p className="bench-callout">
          <strong>
            {formatRange(min, max)}
          </strong>{' '}
          faster than {versus.name} on {runs.length === 1 ? 'this corpus' : `all ${runs.length} corpora`}
        </p>
      ) : null}
      <ol className="bench-rows">
        {runs.map((run, i) => {
          const top = Math.max(...run.rows.map((r) => r.meanMs))
          const comparison = compareText(run)
          return (
            <li key={run.corpus.id} className="bench-group">
              <span className="bench-tool">
                {corpusLabel(run.corpus)}
                <small>
                  {comparison.ratio} {comparison.text.split(' ')[0]}
                </small>
              </span>
              {tools.map((tool) => {
                const r = run.rows.find((x) => x.tool === tool.tool)
                return r ? (
                  <span
                    key={tool.tool}
                    className={`bench-pair is-${tool.tool}`}
                    style={{ '--w': `${(r.meanMs / top) * 100}%`, '--i': i } as CSSProperties}
                  >
                    <span className="bench-tool">{r.name}</span>
                    <span className="bench-track" aria-hidden="true">
                      <span className="bench-bar" />
                    </span>
                    <span className="bench-time">
                      {formatMs(r.meanMs)}
                    </span>
                  </span>
                ) : null
              })}
              {unmeasuredTools(run).map(({ tool, reason }) => (
                <span key={tool} className="bench-desc">
                  {tool.toUpperCase()}: not measured — {reason}.
                </span>
              ))}
            </li>
          )
        })}
      </ol>
    </figure>
  )
}
