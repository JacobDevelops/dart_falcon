import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import { domains, getRule } from '../../lib/rules'

export const Route = createFileRoute('/linter/domains')({
  head: () => ({ meta: [{ title: 'Domains — falcon' }] }),
  component: Domains,
})

const DOMAIN_BLURBS: Record<string, string> = {
  flutter:
    'Rules that only make sense in a Flutter project — widget construction, lifecycle, and framework APIs. Enable the domain to turn them all on at once.',
}

function Domains() {
  return (
    <DocsLayout>
      <div className="rules-head">
        <h1>Domains</h1>
        <p>
          Domains group rules by the framework they apply to. Toggle a whole
          domain in <code>falcon.json</code> under{' '}
          <code>linter.domains</code> — see the{' '}
          <Link to="/reference/configuration" className="inline-link">
            configuration reference
          </Link>
          .
        </p>
      </div>

      {domains.map((domain) => (
        <div className="domain-card" key={domain.name}>
          <h2>
            <span className="dot-good" style={{ background: 'var(--amber)' }} />
            {domain.name}
          </h2>
          <p className="desc">
            {DOMAIN_BLURBS[domain.name] ??
              `Rules in the ${domain.name} domain.`}{' '}
            {domain.rules.length} rules.
          </p>
          <div className="domain-rules">
            {domain.rules.map((name) => {
              const rule = getRule(name)
              return (
                <Link
                  to="/linter/rules/$rule"
                  params={{ rule: name }}
                  className="rule-row"
                  key={name}
                >
                  <div className="rule-row-top">
                    <span className="rule-name">{name}</span>
                    {rule?.recommended ? (
                      <span className="badge badge-rec">rec</span>
                    ) : null}
                  </div>
                  {rule?.description ? (
                    <p className="rule-desc">{rule.description}</p>
                  ) : null}
                </Link>
              )
            })}
          </div>
        </div>
      ))}
    </DocsLayout>
  )
}
