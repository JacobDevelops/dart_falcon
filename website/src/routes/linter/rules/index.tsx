import { createFileRoute } from '@tanstack/react-router'
import { DocsLayout } from '../../../components/DocsLayout'
import { RuleRow } from '../../../components/RuleRow'
import {
  groupedRules,
  crossFileRules,
  rules,
  recommendedCount,
} from '../../../lib/rules'

export const Route = createFileRoute('/linter/rules/')({
  head: () => ({ meta: [{ title: 'Rules — falcon' }] }),
  component: RulesIndex,
})

function RulesIndex() {
  const groups = groupedRules
  return (
    <DocsLayout aside={<GroupNav />}>
      <div className="rules-head">
        <h1>Rules</h1>
        <p>
          {rules.length} rules across {groups.length} groups, {recommendedCount}{' '}
          recommended. {crossFileRules.length} run across the whole{' '}
          <a href="#cross-file" className="inline-link">
            project graph
          </a>
          .
        </p>
      </div>

      {groups.map((g) => (
        <section className="group-block" id={g.id} key={g.id}>
          <div className="group-title">
            <h2>{g.label}</h2>
            <span className="count">
              {g.rules.length} rules ·{' '}
              {g.rules.filter((r) => r.recommended).length} recommended
            </span>
          </div>
          <p className="blurb">{g.blurb}</p>
          {g.rules.map((r) => (
            <RuleRow key={r.name} name={r.name} rule={r} />
          ))}
        </section>
      ))}

      {crossFileRules.length > 0 ? (
        <section className="group-block" id="cross-file">
          <div className="group-title">
            <h2>Cross-file rules</h2>
            <span className="count">
              {crossFileRules.length} rules ·{' '}
              {crossFileRules.filter((r) => r.recommended).length} recommended
            </span>
          </div>
          <p className="blurb">
            Rules that analyze relationships between files across the whole
            project, run in falcon's whole-project pass.
          </p>
          {crossFileRules.map((r) => (
            <RuleRow key={r.name} name={r.name} rule={r} />
          ))}
        </section>
      ) : null}
    </DocsLayout>
  )
}

function GroupNav() {
  const groups = groupedRules
  return (
    <nav className="toc" aria-label="Rule groups">
      <h4>On this page</h4>
      {groups.map((g) => (
        <a href={`#${g.id}`} key={g.id}>
          {g.label} ({g.rules.length})
        </a>
      ))}
      {crossFileRules.length > 0 ? (
        <a href="#cross-file">Cross-file ({crossFileRules.length})</a>
      ) : null}
    </nav>
  )
}
