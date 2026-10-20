import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import { ValueList } from '../../components/ValueList'
import { breadcrumbs, seo } from '../../lib/seo'
import configJson from '../../data/config-reference.json'

export const Route = createFileRoute('/reference/configuration')({
  head: () =>
    seo({
      title: 'falcon.json configuration reference: every key and default',
      description:
        'Every falcon.json key for the falcon Dart and Flutter linter, with its type, default, and accepted values, generated from the configuration schema.',
      path: '/reference/configuration',
      jsonLd: [breadcrumbs([['Reference', '/reference/cli'], ['Configuration', '/reference/configuration']])],
    }),
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

const sections = [...new Set(config.map((key) => topLevel(key.path)))]

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
        <ValueList values={entry.allowedValues} />
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
          <p className="side-heading">Sections</p>
          {sections.map((s) => (
            <a key={s} href={`#sec-${s}`}>
              {s}
            </a>
          ))}
        </nav>
      }
    >
      <div className="reference">
        <div className="page-head">
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
