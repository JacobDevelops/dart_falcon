import { createFileRoute, Link } from '@tanstack/react-router'
import { CodeBlock } from '../../../components/CodeBlock'
import { DocsLayout } from '../../../components/DocsLayout'
import { markdown } from '../../../components/DocsPage'
import { Tabs } from '../../../components/Tabs'
import { DiagnosticFrame } from '../../../components/DiagnosticFrame'
import { Described, RuleBadges, RuleRow } from '../../../components/RuleRow'
import { groupLabel, type Rule, type RuleDiagnostic } from '../../../lib/rules'
import { getRuleDetail } from '../../../lib/server'
import { breadcrumbs, seo, SITE_URL } from '../../../lib/seo'

export const Route = createFileRoute('/linter/rules/$rule')({
  loader: ({ params }) => getRuleDetail({ data: params.rule }),
  head: ({ loaderData, params }) => {
    if (!loaderData) return {}
    const { rule } = loaderData
    const alias = rule.source.upstreamId ? ` (${rule.source.upstreamId})` : ''
    const path = `/linter/rules/${params.rule}`
    const group = groupLabel(rule.group)
    const description = `${rule.description.replace(/`/g, '').replace(/\.?$/, '.')} A ${group.toLowerCase()} lint rule for Dart${rule.domains.includes('flutter') ? ' and Flutter' : ''} in falcon, with examples and configuration.`
    return seo({
      title: `${rule.name}${alias}: Dart lint rule | falcon`,
      description,
      path,
      type: 'article',
      jsonLd: [
        breadcrumbs([
          ['Rules', '/linter/rules'],
          [group, `/linter/rules#${rule.group}`],
          [rule.name, path],
        ]),
        {
          '@type': 'TechArticle',
          headline: `${rule.name}: Dart lint rule`,
          description,
          url: `${SITE_URL}${path}`,
          keywords: [rule.name, rule.source.upstreamId, 'dart lint', 'flutter lint', group]
            .filter(Boolean)
            .join(', '),
          about: { '@type': 'SoftwareApplication', name: 'falcon', url: SITE_URL },
        },
      ],
    })
  },
  component: RuleDetail,
})

function configSnippet(rule: Rule): string {
  const section = rule.crossFile ? 'cross-file' : 'linter'
  return JSON.stringify({ [section]: { rules: { [rule.group]: { [rule.name]: 'error' } } } }, null, 2)
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
    byFile.set(key, [...(byFile.get(key) ?? []), d])
  }
  return (
    <Tabs
      tabs={fixtures.map((f) => {
        const diags = byFile.get(f.path) ?? []
        return {
          id: f.path,
          label: diags.length > 0 ? `${f.path} · ${diags.length}` : f.path,
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
  const { rule, number, total, prev, next, related } = Route.useLoaderData()
  const section = rule.crossFile ? 'cross-file' : 'lint'
  const hasInvalid = !!rule.examples.bad || (rule.examples.crossFile?.length ?? 0) > 0
  const group = groupLabel(rule.group)

  return (
    <DocsLayout>
      <article className="rule-detail">
        <nav aria-label="Breadcrumb" className="crumbs">
          <ol>
            <li>
              <Link to="/linter/rules">Rules</Link>
            </li>
            <li>
              <Link to="/linter/rules" hash={rule.group}>
                {group}
              </Link>
            </li>
            <li aria-current="page">{rule.name}</li>
          </ol>
        </nav>

        <header className="rule-head">
          <h1 className="rule-title">{rule.name}</h1>
          {rule.source.upstreamId ? (
            <p className="rule-alias">
              Same as{' '}
              {rule.source.upstreamUrl ? (
                <a href={rule.source.upstreamUrl}>
                  <code>{rule.source.upstreamId}</code>
                </a>
              ) : (
                <code>{rule.source.upstreamId}</code>
              )}{' '}
              in {rule.source.label}
            </p>
          ) : null}
          {rule.docs ? null : (
            <p className="rule-lede">
              <Described text={rule.description} />
            </p>
          )}
          <dl className="rule-facts">
            <div>
              <dt>Group</dt>
              <dd>
                <Link to="/linter/rules" hash={rule.group}>
                  {group}
                </Link>
              </dd>
            </div>
            <div>
              <dt>Path</dt>
              <dd>
                <code>
                  {section}/{rule.group}/{rule.name}
                </code>
              </dd>
            </div>
            <div>
              <dt>Default</dt>
              <dd>{rule.recommended ? 'On (recommended)' : 'Off, opt in'}</dd>
            </div>
            <div>
              <dt>Origin</dt>
              <dd>{rule.source.label}</dd>
            </div>
            {rule.domains.length > 0 ? (
              <div>
                <dt>Domain</dt>
                <dd>
                  {rule.domains.map((d, i) => (
                    <span key={d}>
                      {i > 0 ? ', ' : ''}
                      <Link to="/linter/domains" hash={d}>
                        {d}
                      </Link>
                    </span>
                  ))}
                </dd>
              </div>
            ) : null}
            <div>
              <dt>Rule</dt>
              <dd className="num">
                {number} of {total}
              </dd>
            </div>
          </dl>
          <div className="rule-badges">
            <RuleBadges rule={rule} />
          </div>
        </header>

        {rule.crossFile ? (
          <p className="callout">
            <strong>Cross-file rule.</strong> It runs in falcon's whole-project
            pass, analyzing relationships across every file rather than one file
            at a time. Configure it under the top-level <code>cross-file</code> key.
          </p>
        ) : null}

        {rule.docs ? (
          <div
            className="prose rule-docs"
            dangerouslySetInnerHTML={{ __html: markdown.parse(rule.docs, { async: false }) }}
          />
        ) : null}

        {hasInvalid ? (
          <section className="rule-section example-invalid" aria-labelledby="invalid">
            <h2 id="invalid">
              <span className="mark-bad" aria-hidden="true" /> Invalid
            </h2>
            {rule.crossFile ? <InvalidCrossFile rule={rule} /> : <InvalidSingle rule={rule} />}
          </section>
        ) : null}

        {rule.examples.good ? (
          <section className="rule-section example-valid" aria-labelledby="valid">
            <h2 id="valid">
              <span className="mark-good" aria-hidden="true" /> Valid
            </h2>
            <CodeBlock code={rule.examples.good} filename="example.dart" />
          </section>
        ) : null}

        <section className="rule-section" aria-labelledby="configure">
          <h2 id="configure">Configure {rule.name}</h2>
          <p>
            Set its severity in <code>falcon.json</code>
            {rule.crossFile ? (
              <>
                {' '}
                under the <code>cross-file</code> section
              </>
            ) : null}
            . See the <Link to="/docs/configuration">configuration guide</Link>.
          </p>
          <CodeBlock code={configSnippet(rule)} lang="json" filename="falcon.json" />
          <p>
            Or silence one occurrence with a{' '}
            <Link to="/docs/suppressions">suppression comment</Link>; the reason
            is required.
          </p>
          <CodeBlock
            code={`// falcon-ignore ${section}/${rule.group}/${rule.name}: <reason>`}
            filename="example.dart"
          />
        </section>

        {related.length > 0 ? (
          <section className="rule-section" aria-labelledby="related">
            <h2 id="related">More {group.toLowerCase()} rules</h2>
            <ul className="rule-list">
              {related.map((r) => (
                <RuleRow key={r.name} rule={r} />
              ))}
            </ul>
          </section>
        ) : null}

        <nav className="pager" aria-label="Adjacent rules">
          {prev ? (
            <Link to="/linter/rules/$rule" params={{ rule: prev.name }} rel="prev">
              <span>Previous rule</span>
              {prev.name}
            </Link>
          ) : (
            <span />
          )}
          {next ? (
            <Link to="/linter/rules/$rule" params={{ rule: next.name }} rel="next" className="pager-next">
              <span>Next rule</span>
              {next.name}
            </Link>
          ) : null}
        </nav>
      </article>
    </DocsLayout>
  )
}
