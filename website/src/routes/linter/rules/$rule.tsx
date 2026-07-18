import { createFileRoute, Link } from '@tanstack/react-router'
import { marked } from 'marked'
import { CodeBlock } from '../../../components/CodeBlock'
import { DocsLayout } from '../../../components/DocsLayout'
import { Tabs } from '../../../components/Tabs'
import { DiagnosticFrame } from '../../../components/DiagnosticFrame'
import {
  getRule,
  groupLabel,
  type Rule,
  type RuleDiagnostic,
} from '../../../lib/rules'

marked.setOptions({ gfm: true })

export const Route = createFileRoute('/linter/rules/$rule')({
  head: ({ params }) => ({
    meta: [{ title: `${params.rule} — falcon rule` }],
  }),
  component: RuleDetail,
})

function categoryPath(rule: Rule): string {
  const section = rule.crossFile ? 'cross-file' : 'lint'
  return `${section}/${rule.group}/${rule.name}`
}

function configSnippet(rule: Rule): string {
  const section = rule.crossFile ? 'cross-file' : 'linter'
  const config = {
    [section]: { rules: { [rule.group]: { [rule.name]: 'error' } } },
  }
  return JSON.stringify(config, null, 2)
}

function SourceCrosswalk({ rule }: { rule: Rule }) {
  const { source } = rule
  if (!source.upstreamId) {
    return (
      <span className="rule-source">
        Source: <span className="src-origin">{source.label}</span>
      </span>
    )
  }
  const same = (
    <>
      Same as <code>{source.upstreamId}</code>
    </>
  )
  return (
    <span className="rule-source">
      {source.upstreamUrl ? (
        <a href={source.upstreamUrl} target="_blank" rel="noreferrer">
          {same}
        </a>
      ) : (
        same
      )}{' '}
      <span className="src-origin">· {source.label}</span>
    </span>
  )
}

function InvalidSingle({ rule }: { rule: Rule }) {
  const bad = rule.examples.bad
  if (!bad) return null
  return (
    <>
      <CodeBlock code={bad} filename="example.dart" />
      {rule.diagnostics.length > 0 ? (
        <div className="diag-frames">
          {rule.diagnostics.map((d, i) => (
            <DiagnosticFrame key={i} source={bad} diagnostic={d} />
          ))}
        </div>
      ) : null}
    </>
  )
}

function InvalidCrossFile({ rule }: { rule: Rule }) {
  const fixtures = rule.examples.crossFile ?? []
  if (fixtures.length === 0) return null
  const byFile = new Map<string, RuleDiagnostic[]>()
  for (const d of rule.diagnostics) {
    const key = d.file ?? ''
    const list = byFile.get(key) ?? []
    list.push(d)
    byFile.set(key, list)
  }
  return (
    <Tabs
      tabs={fixtures.map((f) => {
        const diags = byFile.get(f.path) ?? []
        return {
          id: f.path,
          label: diags.length > 0 ? `${f.path} ·${diags.length}` : f.path,
          content: (
            <>
              <CodeBlock code={f.content} filename={f.path} />
              {diags.length > 0 ? (
                <div className="diag-frames">
                  {diags.map((d, i) => (
                    <DiagnosticFrame key={i} source={f.content} diagnostic={d} />
                  ))}
                </div>
              ) : (
                <p className="fixture-clean">No diagnostics in this file.</p>
              )}
            </>
          ),
        }
      })}
    />
  )
}

function RuleDetail() {
  const { rule: name } = Route.useParams()
  const rule = getRule(name)

  if (!rule) {
    return (
      <DocsLayout>
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
      </DocsLayout>
    )
  }

  const docsHtml = rule.docs
    ? (marked.parse(rule.docs, { async: false }) as string)
    : ''
  const hasInvalid =
    !!rule.examples.bad || (rule.examples.crossFile?.length ?? 0) > 0

  return (
    <DocsLayout>
      <div className="rule-detail">
        <p className="crumbs">
          <Link to="/linter/rules">Rules</Link> /{' '}
          <a href={`/linter/rules#${rule.group}`}>{groupLabel(rule.group)}</a> /{' '}
          {rule.name}
        </p>
        <h1 className="rule-title">{rule.name}</h1>

        {/* Metadata summary block */}
        <div className="rule-summary">
          <code className="category-path">{categoryPath(rule)}</code>
          <div className="rule-summary-badges">
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
          <SourceCrosswalk rule={rule} />
        </div>

        {/* Cross-file callout */}
        {rule.crossFile ? (
          <div className="callout callout-cross">
            <strong>Cross-file rule.</strong> This rule runs in falcon's
            whole-project pass, analyzing relationships across every file rather
            than one file at a time. Configure it under the top-level{' '}
            <code>cross-file</code> key, and expect it to do more work on large
            projects than a single-file rule.
          </div>
        ) : null}

        {/* Full rule docs */}
        {docsHtml ? (
          <article
            className="prose rule-docs"
            dangerouslySetInnerHTML={{ __html: docsHtml }}
          />
        ) : null}

        {/* Invalid */}
        {hasInvalid ? (
          <section className="rule-section example-invalid">
            <h2>
              <span className="dot-bad" /> Invalid
            </h2>
            {rule.crossFile ? (
              <InvalidCrossFile rule={rule} />
            ) : (
              <InvalidSingle rule={rule} />
            )}
          </section>
        ) : null}

        {/* Valid */}
        {rule.examples.good ? (
          <section className="rule-section example-valid">
            <h2>
              <span className="dot-good" /> Valid
            </h2>
            <CodeBlock code={rule.examples.good} filename="example.dart" />
          </section>
        ) : null}

        {/* How to configure */}
        <section className="rule-section">
          <h2>How to configure</h2>
          <p style={{ color: 'var(--muted)', marginTop: 0 }}>
            Set the severity of <code>{rule.name}</code> in your{' '}
            <code>falcon.json</code>
            {rule.crossFile ? (
              <>
                {' '}
                under the <code>cross-file</code> section
              </>
            ) : null}
            :
          </p>
          <CodeBlock
            code={configSnippet(rule)}
            lang="json"
            filename="falcon.json"
          />
        </section>

        {/* Related */}
        <section className="rule-section related">
          <h2>Related</h2>
          <ul>
            <li>
              <Link to="/linter/rules">All rules</Link>
            </li>
            <li>
              <a href={`/linter/rules#${rule.group}`}>
                {groupLabel(rule.group)} rules
              </a>
            </li>
            {rule.domains.map((d) => (
              <li key={d}>
                <Link to="/linter/domains">{d} domain</Link>
              </li>
            ))}
            <li>
              <Link to="/docs/suppressions">Suppress this rule inline</Link>
            </li>
            {rule.source.upstreamUrl ? (
              <li>
                <a href={rule.source.upstreamUrl} target="_blank" rel="noreferrer">
                  {rule.source.label} source
                </a>
              </li>
            ) : null}
          </ul>
        </section>
      </div>
    </DocsLayout>
  )
}
