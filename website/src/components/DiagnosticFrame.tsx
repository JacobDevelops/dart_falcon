import { highlightDart } from '../lib/highlight'
import type { RuleDiagnostic } from '../lib/rules'

// How many lines of leading context to show above the flagged line.
const CONTEXT = 1

function gutterWidth(maxLine: number): number {
  return Math.max(2, String(maxLine).length)
}

/**
 * Renders a single diagnostic as a Biome-style code frame: the offending source
 * line (with a little context) in a numbered gutter, a caret line marking the
 * column, and the verbatim message. `source` is the file the diagnostic points
 * into; `line`/`column` are 1-based.
 */
export function DiagnosticFrame({
  source,
  diagnostic,
}: {
  source: string
  diagnostic: RuleDiagnostic
}) {
  const lines = source.split('\n')
  const { line, column, message, file } = diagnostic
  const idx = line - 1

  // Defensive: if the line is out of range, show just the message.
  if (idx < 0 || idx >= lines.length) {
    return (
      <div className="diag-frame">
        <div className="diag-frame-msg">
          {file ? <span className="diag-frame-file">{file}</span> : null}
          {message}
        </div>
      </div>
    )
  }

  const start = Math.max(0, idx - CONTEXT)
  const gw = gutterWidth(line)
  const caretPad = ' '.repeat(Math.max(0, column - 1))

  return (
    <div className="diag-frame">
      <div className="diag-frame-msg">
        {file ? <span className="diag-frame-file">{file}</span> : null}
        {message}
      </div>
      <div className="diag-frame-code">
        {lines.slice(start, idx + 1).map((text, i) => {
          const n = start + i + 1
          const isTarget = n === line
          return (
            <div className={`fline${isTarget ? ' target' : ''}`} key={n}>
              <span className="fgutter" style={{ width: `${gw + 1}ch` }}>
                {n}
              </span>
              <code
                className="fcode"
                dangerouslySetInnerHTML={{ __html: highlightDart(text) || ' ' }}
              />
            </div>
          )
        })}
        <div className="fline caret-line" aria-hidden="true">
          <span className="fgutter" style={{ width: `${gw + 1}ch` }} />
          <code className="fcode fcaret">
            {caretPad}
            <span className="caret">∙</span>
          </code>
        </div>
      </div>
    </div>
  )
}
