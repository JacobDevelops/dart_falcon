import type { CSSProperties } from 'react'
import {
  compareText,
  corpusLabel,
  formatMs,
  formatRange,
  speedup,
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
 * falcon's ratio against dart analyze pinned above the bars. `detailed` adds spread and run counts.
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
          {run.corpus.files.toLocaleString('en-US')} files ·{' '}
          {run.corpus.lines.toLocaleString('en-US')} lines
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
            className={`bench-row${r.tool === 'falcon' ? ' is-falcon' : ''}`}
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
    </figure>
  )
}

/**
 * One scenario across every corpus: per corpus, falcon and dart analyze bars
 * scaled to that corpus's slower tool, with the speedup range as the callout.
 */
export function BenchSummaryChart({ summary }: { summary: ScenarioSummary }) {
  const { scenario, runs, min, max, slower } = summary
  const versus = speedup(runs[0]).versus
  const falcon = runs[0].rows.find((r) => r.tool === 'falcon')
  return (
    <figure className="bench">
      <figcaption className="bench-cap">
        <h2 className="bench-title">
          falcon vs {versus.name} <span>{scenario.title.toLowerCase()}</span>
        </h2>
        <span className="bench-desc">
          <span className="key key-falcon" aria-hidden="true" /> falcon {falcon?.version}
          {'  '}
          <span className="key" aria-hidden="true" /> {versus.name} {versus.version} · mean
          wall-clock time per corpus
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
          const pair = run.rows.filter((r) => r.tool === 'falcon' || r.tool === versus.tool)
          const top = Math.max(...pair.map((r) => r.meanMs))
          return (
            <li key={run.corpus.id} className="bench-group">
              <span className="bench-tool">
                {corpusLabel(run.corpus)}
                <small>
                  {compareText(run).ratio} {compareText(run).text.split(' ')[0]}
                </small>
              </span>
              {['falcon', versus.tool].map((tool) => {
                const r = pair.find((x) => x.tool === tool)
                return r ? (
                  <span
                    key={tool}
                    className={`bench-pair${tool === 'falcon' ? ' is-falcon' : ''}`}
                    style={{ '--w': `${(r.meanMs / top) * 100}%`, '--i': i } as CSSProperties}
                  >
                    <span className="bench-track" aria-hidden="true">
                      <span className="bench-bar" />
                    </span>
                    <span className="bench-time">
                      <span className="visually-hidden">{r.name}: </span>
                      {formatMs(r.meanMs)}
                    </span>
                  </span>
                ) : null
              })}
            </li>
          )
        })}
      </ol>
    </figure>
  )
}
