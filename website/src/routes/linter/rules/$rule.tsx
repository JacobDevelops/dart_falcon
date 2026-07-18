import { createFileRoute, Link } from '@tanstack/react-router'
import { CodeBlock } from '../../../components/CodeBlock'
import { getRule, groupLabel, type Rule } from '../../../lib/rules'

export const Route = createFileRoute('/linter/rules/$rule')({
  head: ({ params }) => ({
    meta: [{ title: `${params.rule} — falcon rule` }],
  }),
  component: RuleDetail,
})

function configSnippet(rule: Rule): string {
  const section = rule.crossFile ? 'cross-file' : 'linter'
  const config = {
    [section]: {
      rules: {
        [rule.group]: {
          [rule.name]: 'error',
        },
      },
    },
  }
  return JSON.stringify(config, null, 2)
}

function RuleDetail() {
  const { rule: name } = Route.useParams()
  const rule = getRule(name)

  if (!rule) {
    return (
      <div className="wrap">
        <div style={{ padding: '60px 0', maxWidth: 820 }}>
          <p className="crumbs">
            <Link to="/linter/rules">Rules</Link> / {name}
          </p>
          <h1>Rule not found</h1>
          <p style={{ color: 'var(--muted)' }}>
            No rule named <code>{name}</code> exists.{' '}
            <Link to="/linter/rules" style={{ color: 'var(--amber-bright)' }}>
              Browse all rules
            </Link>
            .
          </p>
        </div>
      </div>
    )
  }

  const fixtures = rule.examples.crossFile ?? []

  return (
    <div className="wrap">
      <div style={{ padding: '36px 0 72px', maxWidth: 840 }}>
        <div className="detail-head">
          <p className="crumbs">
            <Link to="/linter/rules">Rules</Link> /{' '}
            <a href={`/linter/rules#${rule.group}`}>{groupLabel(rule.group)}</a> /{' '}
            {rule.name}
          </p>
          <h1>{rule.name}</h1>
          <div className="detail-badges">
            {rule.recommended ? (
              <span className="badge badge-rec">recommended</span>
            ) : null}
            {rule.crossFile ? (
              <span className="badge badge-cross">cross-file</span>
            ) : null}
            {rule.domains.map((d) => (
              <span className="tag" key={d}>
                {d}
              </span>
            ))}
          </div>
          {rule.description ? (
            <p style={{ color: 'var(--muted)', margin: 0, fontSize: '1.05rem' }}>
              {rule.description}
            </p>
          ) : null}

          <dl className="meta-table">
            <dt>Group</dt>
            <dd>{groupLabel(rule.group)}</dd>
            <dt>Recommended</dt>
            <dd>{rule.recommended ? 'Yes' : 'No'}</dd>
            <dt>Scope</dt>
            <dd>{rule.crossFile ? 'Cross-file (whole project)' : 'Single file'}</dd>
            <dt>Source</dt>
            <dd>
              {rule.source.upstreamUrl ? (
                <a href={rule.source.upstreamUrl} target="_blank" rel="noreferrer">
                  {rule.source.label}
                </a>
              ) : (
                rule.source.label
              )}
              {rule.source.upstreamId ? (
                <span style={{ color: 'var(--faint)' }}>
                  {' '}
                  · <code>{rule.source.upstreamId}</code>
                </span>
              ) : null}
            </dd>
            {rule.domains.length > 0 ? (
              <>
                <dt>Domains</dt>
                <dd>{rule.domains.join(', ')}</dd>
              </>
            ) : null}
          </dl>
        </div>

        {/* Invalid example */}
        {rule.examples.bad || fixtures.length > 0 ? (
          <section className="detail-section example-invalid">
            <h2>
              <span className="dot-bad" /> Invalid
            </h2>
            {rule.examples.bad ? (
              <CodeBlock code={rule.examples.bad} filename="example.dart" />
            ) : (
              fixtures.map((f) => (
                <div key={f.path}>
                  <div className="fixture-name">{f.path}</div>
                  <CodeBlock code={f.content} filename={f.path} />
                </div>
              ))
            )}
            {rule.diagnostics.length > 0 ? (
              <div className="diag-list">
                {rule.diagnostics.map((d, i) => (
                  <div className="diag" key={i}>
                    <span className="loc">
                      {d.file ? `${d.file}:` : ''}
                      {d.line}:{d.column}
                    </span>
                    <span className="msg">{d.message}</span>
                  </div>
                ))}
              </div>
            ) : null}
          </section>
        ) : null}

        {/* Valid example */}
        {rule.examples.good ? (
          <section className="detail-section example-valid">
            <h2>
              <span className="dot-good" /> Valid
            </h2>
            <CodeBlock code={rule.examples.good} filename="example.dart" />
          </section>
        ) : null}

        {/* Configure */}
        <section className="detail-section">
          <h2>Configure</h2>
          <p style={{ color: 'var(--muted)', marginTop: 0 }}>
            Enable or set the severity of <code>{rule.name}</code> in your{' '}
            <code>falcon.json</code>:
          </p>
          <CodeBlock
            code={configSnippet(rule)}
            lang="json"
            filename="falcon.json"
          />
        </section>
      </div>
    </div>
  )
}
