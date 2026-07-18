import { createFileRoute, Link } from '@tanstack/react-router'
import { SidebarLayout } from '../../../components/SidebarLayout'
import {
  groupedRules,
  crossFileRules,
  rules,
  type Rule,
} from '../../../lib/rules'

export const Route = createFileRoute('/linter/rules/')({
  head: () => ({ meta: [{ title: 'Rules — falcon' }] }),
  component: RulesIndex,
})

function RulesIndex() {
  const groups = groupedRules()
  return (
    <SidebarLayout sidebar={<RulesSidebar />}>
      <div className="rules-head">
        <h1>Rules</h1>
        <p>
          {rules.length} rules across {groups.length} groups. {crossFileRules.length}{' '}
          run across the whole project graph.
        </p>
      </div>

      {groups.map((g) => (
        <section className="group-block" id={g.id} key={g.id}>
          <div className="group-title">
            <h2>{g.label}</h2>
            <span className="count">{g.rules.length}</span>
          </div>
          <p className="blurb">{g.blurb}</p>
          {g.rules.map((r) => (
            <RuleRow key={r.name} rule={r} />
          ))}
        </section>
      ))}

      {crossFileRules.length > 0 ? (
        <section className="group-block" id="cross-file">
          <div className="group-title">
            <h2>Cross-file rules</h2>
            <span className="count">{crossFileRules.length}</span>
          </div>
          <p className="blurb">
            Rules that analyze relationships between files across the whole
            project.
          </p>
          {crossFileRules.map((r) => (
            <RuleRow key={r.name} rule={r} />
          ))}
        </section>
      ) : null}
    </SidebarLayout>
  )
}

function RuleRow({ rule }: { rule: Rule }) {
  return (
    <Link
      to="/linter/rules/$rule"
      params={{ rule: rule.name }}
      className="rule-row"
    >
      <div className="rule-row-top">
        <span className="rule-name">{rule.name}</span>
        {rule.recommended ? <span className="badge badge-rec">recommended</span> : null}
        {rule.crossFile ? <span className="badge badge-cross">cross-file</span> : null}
        {rule.domains.map((d) => (
          <span className="tag" key={d}>
            {d}
          </span>
        ))}
      </div>
      {rule.description ? <p className="rule-desc">{rule.description}</p> : null}
    </Link>
  )
}

function RulesSidebar() {
  const groups = groupedRules()
  return (
    <nav>
      <h4>Groups</h4>
      {groups.map((g) => (
        <a href={`#${g.id}`} key={g.id}>
          {g.label}
        </a>
      ))}
      {crossFileRules.length > 0 ? <a href="#cross-file">Cross-file</a> : null}
      <h4>Docs</h4>
      <Link to="/docs/installation">Installation</Link>
      <Link to="/docs/configuration">Configuration</Link>
      <Link to="/linter/domains">Domains</Link>
    </nav>
  )
}
