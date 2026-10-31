import { createFileRoute, Link } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import { ValueList } from '../../components/ValueList'
import { Toc } from '../../components/Toc'
import { CollapsibleSection } from '../../components/CollapsibleSection'
import { breadcrumbs, seo } from '../../lib/seo'
import { configKeyId } from '../../lib/headings'
import configJson from '../../data/config-reference.json'
import cli from '../../data/cli.json'
import { CodeBlock } from '../../components/CodeBlock'
import { CopyCommand } from '../../components/CopyCommand'

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

const schemaUrl = `https://raw.githubusercontent.com/JacobDevelops/dart_falcon/v${cli.version}/schema/falcon.schema.json`

function keyExample(entry: ConfigKey): string | undefined {
  const crossFile = entry.path.includes('cross-file')
  const path = entry.path
    .replace('<group>', crossFile ? 'correctness' : 'suspicious')
    .replace('<rule>', crossFile ? 'unused-files' : 'avoid-dynamic')
    .split('.')
  const values: Record<string, unknown> = {
    baseline: 'falcon-baseline.json',
    'max-errors': 100,
    'error-on-warnings': true,
    includes: ['test/**', '!test/fixtures/**'],
    enabled: false,
    recommended: false,
  }
  const last = path[path.length - 1]
  let example: unknown = entry.allowedValues?.includes('warn')
    ? 'warn'
    : entry.allowedValues?.includes('recommended')
      ? 'all'
      : values[last]
  if (entry.path === 'files.includes') example = ['**', '!build/**', '!**/*.g.dart']
  if (example === undefined) return undefined
  for (const key of path.reverse()) {
    if (key === 'overrides[]') {
      const override: Record<string, unknown> = {
        includes: values.includes,
        ...(example as Record<string, unknown>),
      }
      if (entry.path === 'overrides[].includes') {
        override.linter = { enabled: false }
      }
      example = { overrides: [override] }
    } else {
      example = { [key]: example }
    }
  }
  return JSON.stringify(example, null, 2)
}

function KeyEntry({ entry }: { entry: ConfigKey }) {
  const isRulePath = entry.path.includes('<rule>')
  const example = entry.path === '$schema' ? undefined : keyExample(entry)
  return (
    <div className="config-key" id={configKeyId(entry.path)}>
      {entry.path !== topLevel(entry.path) ? (
        <h3 className="config-key-head">
          <code className="config-path">{entry.path}</code>
        </h3>
      ) : null}
      <p className="config-signature">
        Type: <code>{entry.type}</code>
        {'default' in entry ? (
          <> · Default: <code>{JSON.stringify(entry.default)}</code></>
        ) : null}
      </p>
      <p className="config-desc">
        {entry.path === '$schema'
          ? `Use this exact URL for falcon ${cli.version}. It enables rule-name autocomplete and configuration validation in your editor.`
          : entry.description}
      </p>
      {entry.allowedValues?.length || isRulePath ? (
        <div className="config-meta">
          <ValueList values={entry.allowedValues} />
          {isRulePath ? (
            <span className="flag-tag">
              <Link to="/linter/rules">browse rule ids →</Link>
            </span>
          ) : null}
        </div>
      ) : null}
      {entry.path === '$schema' ? (
        <CopyCommand
          command={`"$schema": "${schemaUrl}"`}
          prompt={false}
          label="Copy schema setting"
        />
      ) : example ? (
        <details className="config-example">
          <summary>Example<span className="visually-hidden"> for {entry.path}</span></summary>
          <CodeBlock code={example} filename="falcon.json" lang="json" />
        </details>
      ) : null}
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
