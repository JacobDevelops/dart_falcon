import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import { ValueList } from '../../components/ValueList'
import { Toc } from '../../components/Toc'
import { CollapsibleSection } from '../../components/CollapsibleSection'
import { breadcrumbs, seo } from '../../lib/seo'
import { configKeyId } from '../../lib/headings'
import configJson from '../../data/config-reference.json'

export const Route = createFileRoute('/reference/configuration')({
  head: () =>
    seo({
      title: 'Config reference | Dart Falcon',
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
    <div className="config-key" id={configKeyId(entry.path)}>
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
        <Toc
          items={sections.map((section) => ({
            id: `sec-${section}`,
            title: section,
          }))}
        />
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
          <CollapsibleSection id={`sec-${sec}`} title={sec} key={sec} className="ref-section">
            {config
              .filter((k) => topLevel(k.path) === sec)
              .map((k) => (
                <KeyEntry key={k.path} entry={k} />
              ))}
          </CollapsibleSection>
        ))}
      </div>
    </DocsLayout>
  )
}
