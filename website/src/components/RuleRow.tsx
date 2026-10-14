import { Link } from '@tanstack/react-router'
import type { Rule } from '../lib/rules'

export function RuleBadges({ rule }: { rule: Rule }) {
  return (
    <>
      {rule.recommended ? <span className="badge badge-rec">recommended</span> : null}
      {rule.crossFile ? <span className="badge badge-cross">cross-file</span> : null}
      {rule.domains.map((d) => (
        <span className="tag" key={d}>{d}</span>
      ))}
    </>
  )
}

export function RuleRow({ name, rule, domain = false }: {
  name: string
  rule?: Rule
  domain?: boolean
}) {
  return (
    <Link to="/linter/rules/$rule" params={{ rule: name }} className="rule-row">
      <div className="rule-row-top">
        <span className="rule-name">{name}</span>
        {domain && rule?.recommended ? <span className="badge badge-rec">rec</span> : null}
        {!domain && rule ? <RuleBadges rule={rule} /> : null}
      </div>
      {rule?.description ? <p className="rule-desc">{rule.description}</p> : null}
    </Link>
  )
}
