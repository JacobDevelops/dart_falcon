import { Link } from '@tanstack/react-router'
import type { RuleSummary } from '../lib/rules'

/** Renders a rule description's `code` spans; descriptions use no other markdown. */
export function Described({ text }: { text: string }) {
  return text.split('`').map((part, i) => (i % 2 ? <code key={i}>{part}</code> : part))
}

export function RuleBadges({ rule }: { rule: Pick<RuleSummary, 'recommended' | 'crossFile' | 'domains'> }) {
  return (
    <>
      {rule.recommended ? <span className="badge badge-rec">recommended</span> : null}
      {rule.crossFile ? <span className="badge badge-cross">cross-file</span> : null}
      {rule.domains.map((d) => (
        <span className="tag" key={d}>
          {d}
        </span>
      ))}
    </>
  )
}

export function RuleRow({ rule }: { rule: RuleSummary }) {
  return (
    <li className="rule-row">
      <Link to="/linter/rules/$rule" params={{ rule: rule.name }} className="rule-name">
        {rule.name}
      </Link>
      {rule.upstreamId ? <code className="rule-sci">{rule.upstreamId}</code> : null}
      <span className="rule-badges">
        <RuleBadges rule={rule} />
      </span>
      {rule.description ? (
        <p className="rule-desc">
          <Described text={rule.description} />
        </p>
      ) : null}
    </li>
  )
}
