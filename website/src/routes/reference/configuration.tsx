import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import configJson from '../../data/config-reference.json'

export const Route = createFileRoute('/reference/configuration')({
  head: () => ({ meta: [{ title: 'Configuration Reference — falcon' }] }),
  component: ConfigReference,
})

interface ConfigKey {
  path: string
  type: string
  description: string
  default?: unknown
  allowedValues?: string[]
}

const config = configJson as unknown as ConfigKey[]

function topLevel(path: string): string {
  return path.split('.')[0].replace(/\[\]$/, '')
}

// Preserve first-seen order of top-level sections.
const sections: string[] = []
for (const key of config) {
  const top = topLevel(key.path)
  if (!sections.includes(top)) sections.push(top)
}

function KeyEntry({ entry }: { entry: ConfigKey }) {
  const isRulePath = entry.path.includes('<rule>')
  return (
    <div className="config-key" id={`key-${entry.path.replace(/[^a-z0-9]+/gi, '-')}`}>
      <div className="config-key-head">
        <code className="config-path">{entry.path}</code>
        <code className="config-type">{entry.type}</code>
      </div>
      <p className="config-desc">{entry.description}</p>
      <div className="config-meta">
        {'default' in entry ? (
          <span className="flag-tag">
            default: <code>{JSON.stringify(entry.default)}</code>
          </span>
        ) : null}
        {entry.allowedValues && entry.allowedValues.length > 0 ? (
          <span className="flag-tag">
            values:{' '}
            {entry.allowedValues.map((v, i) => (
              <span key={v}>
                {i > 0 ? ' | ' : ''}
                <code>{v}</code>
              </span>
            ))}
          </span>
        ) : null}
        {isRulePath ? (
          <span className="flag-tag">
            <Link to="/linter/rules">browse rule ids →</Link>
          </span>
        ) : null}
      </div>
    </div>
  )
}

function ConfigReference() {
  return (
    <DocsLayout
      aside={
        <nav className="toc" aria-label="Config sections">
          <h4>Sections</h4>
          {sections.map((s) => (
            <a key={s} href={`#sec-${s}`}>
              {s}
            </a>
          ))}
        </nav>
      }
    >
      <div className="reference">
        <div className="rules-head">
          <h1>Configuration Reference</h1>
          <p>
            Every <code>falcon.json</code> key, its type, default, and accepted
            values — generated from the config schema. For task-oriented setup,
            see the{' '}
            <Link to="/docs/configuration" className="inline-link">
              Configuration guide
            </Link>
            .
          </p>
        </div>

        {sections.map((sec) => (
          <section className="ref-section" id={`sec-${sec}`} key={sec}>
            <h2>
              <code>{sec}</code>
            </h2>
            {config
              .filter((k) => topLevel(k.path) === sec)
              .map((k) => (
                <KeyEntry key={k.path} entry={k} />
              ))}
          </section>
        ))}
      </div>
    </DocsLayout>
  )
}
