import type { CSSProperties } from 'react'
import {
  compareText,
  formatMs,
  hasComparator,
  speedup,
  unmeasuredTools,
  type BenchRun,
} from '../lib/benchmarks'

/** Round tick step (1, 2 or 5 × 10^k) giving at most five ticks up to `max`. */
function ticks(max: number): number[] {
  const raw = max / 4
  const mag = 10 ** Math.floor(Math.log10(raw))
  const step = [1, 2, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw
  return Array.from({ length: Math.floor(max / step) + 1 }, (_, i) => i * step)
}

/** Wall-clock bars for one corpus and cache scenario, on the pair's shared scale. */
export function BenchChart({
  run,
  maxMs,
  title,
}: {
  run: BenchRun
  maxMs: number
  title: string
}) {
  const max = maxMs
  const compared =
    hasComparator(run) && run.rows.some((r) => r.tool === 'falcon') && speedup(run).versus.tool !== 'falcon'
      ? compareText(run)
      : null
  return (
    <figure className="bench">
      <figcaption className="bench-cap">
        <h4 className="bench-title">{title}</h4>
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

