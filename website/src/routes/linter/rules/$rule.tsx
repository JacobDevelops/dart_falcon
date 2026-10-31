import { createFileRoute, Link } from '@tanstack/react-router'
import { CodeBlock } from '../../../components/CodeBlock'
import { DocsLayout } from '../../../components/DocsLayout'
import { MarkdownContent } from '../../../components/DocsPage'
import { CollapsibleSection } from '../../../components/CollapsibleSection'
import { Toc } from '../../../components/Toc'
import { extractHeadings } from '../../../lib/headings'
import { Tabs } from '../../../components/Tabs'
import { DiagnosticFrame } from '../../../components/DiagnosticFrame'
import { Described, RuleBadges, RuleRow } from '../../../components/RuleRow'
import { groupLabel, ruleGuidance, type Rule, type RuleDiagnostic } from '../../../lib/rules'
import { getRuleDetail } from '../../../lib/server'
import { breadcrumbs, seo, SITE_URL } from '../../../lib/seo'

export const Route = createFileRoute('/linter/rules/$rule')({
  loader: ({ params }) => getRuleDetail({ data: params.rule }),
  head: ({ loaderData, params }) => {
    if (!loaderData) return {}
    const { rule } = loaderData
    const path = `/linter/rules/${params.rule}`
    const group = groupLabel(rule.group)
    const description = `${rule.name}: ${rule.description.replace(/`/g, '').replace(/\.?$/, '.')} ${group} lint rule for Dart${rule.domains.includes('flutter') ? ' and Flutter' : ''} in falcon.`
    return seo({
      title: `${rule.name} | Dart Falcon`,
      description,
      path,
      markdownPath: `${path}.md`,
      type: 'article',
      jsonLd: [
        breadcrumbs([
          ['Rules', '/linter/rules'],
          [group, `/linter/groups/${rule.group}`],
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
  const { rule, prev, next, related } = Route.useLoaderData()
  const { configSnippet, suppression } = ruleGuidance(rule)
  const hasInvalid = !!rule.examples.bad || (rule.examples.crossFile?.length ?? 0) > 0
  const group = groupLabel(rule.group)
  const toc = [
    ...extractHeadings(rule.docs ?? ''),
    ...(hasInvalid ? [{ id: 'invalid', title: 'Invalid' }] : []),
    ...(rule.examples.good ? [{ id: 'valid', title: 'Valid' }] : []),
    { id: 'configure', title: 'Configuration' },
    ...(related.length ? [{ id: 'related', title: `More ${group.toLowerCase()} rules` }] : []),
  ]

  return (
    <DocsLayout aside={<Toc items={toc} />}>
      <article className="rule-detail">
        <nav aria-label="Breadcrumb" className="crumbs">
          <ol>
            <li>
              <Link to="/linter/rules">Rules</Link>
            </li>
            <li>
              <Link to="/linter/groups/$group" params={{ group: rule.group }}>
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
                <Link to="/linter/groups/$group" params={{ group: rule.group }}>
                  {group}
                </Link>
              </dd>
            </div>
            <div>
              <dt>Path</dt>
              <dd>
                <code>
                  {rule.crossFile ? 'cross-file' : 'lint'}/{rule.group}/{rule.name}
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
                      <Link to="/linter/domains/$domain" params={{ domain: d }}>
                        {d}
                      </Link>
                    </span>
                  ))}
                </dd>
              </div>
            ) : null}
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
          <div className="prose rule-docs">
            <MarkdownContent source={rule.docs} />
          </div>
        ) : null}

        {hasInvalid ? (
          <CollapsibleSection id="invalid" title="Invalid" className="rule-section example-invalid">
            {(rule.examples.crossFile?.length ?? 0) > 0 ? <InvalidCrossFile rule={rule} /> : <InvalidSingle rule={rule} />}
          </CollapsibleSection>
        ) : null}

        {rule.examples.good ? (
          <CollapsibleSection id="valid" title="Valid" className="rule-section example-valid">
            <CodeBlock code={rule.examples.good} filename="example.dart" />
          </CollapsibleSection>
        ) : null}

        <CollapsibleSection id="configure" title={`Configure ${rule.name}`} className="rule-section">
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
          <CodeBlock code={configSnippet} lang="json" filename="falcon.json" />
          <p>{suppression.text}</p>
          <CodeBlock
            code={suppression.code}
            lang={suppression.lang}
            filename={suppression.filename}
          />
          <p>See the <Link to="/docs/suppressions">suppression guide</Link>.</p>
        </CollapsibleSection>

        {related.length > 0 ? (
          <CollapsibleSection id="related" title={`More ${group.toLowerCase()} rules`} className="rule-section">
            <ul className="rule-list">
              {related.map((r) => (
                <RuleRow key={r.name} rule={r} />
              ))}
            </ul>
          </CollapsibleSection>
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
