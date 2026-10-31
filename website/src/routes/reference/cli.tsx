import { createFileRoute } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import { CodeBlock } from '../../components/CodeBlock'
import { ValueList } from '../../components/ValueList'
import { Toc } from '../../components/Toc'
import { CollapsibleSection } from '../../components/CollapsibleSection'
import { breadcrumbs, seo } from '../../lib/seo'
import cliJson from '../../data/cli.json'

export const Route = createFileRoute('/reference/cli')({
  head: () =>
    seo({
      title: 'CLI reference | Dart Falcon',
      description: `Every falcon command and flag for linting Dart and Flutter from the terminal and CI: falcon check, falcon lsp, falcon migrate, output formats, and exit codes. Generated from falcon v${cli.version}.`,
      path: '/reference/cli',
      jsonLd: [breadcrumbs([['Reference', '/reference/cli'], ['CLI', '/reference/cli']])],
    }),
  component: CliReference,
})

interface CliArg {
  name: string
  long: string | null
  short: string | null
  valueName: string | null
  default: string | null
  help: string | null
  possibleValues: string[]
  positional: boolean
}

interface CliCommand {
  name: string
  about: string
  usage?: string
  args: CliArg[]
}

interface Cli {
  name: string
  about: string
  version: string
  usage: string
  globalArgs: CliArg[]
  subcommands: CliCommand[]
}

const cli = cliJson as unknown as Cli

function argSignature(arg: CliArg): string {
  if (arg.positional) {
    return arg.valueName ? `<${arg.valueName}>` : arg.name.toUpperCase()
  }
  const flag = arg.long ?? `--${arg.name}`
  return arg.valueName ? `${flag} <${arg.valueName}>` : flag
}

function FlagEntry({ arg }: { arg: CliArg }) {
  const isBoolean = !arg.valueName && !arg.positional
  return (
    <div className="flag">
      <div className="flag-sig">
        <code className="flag-name">{argSignature(arg)}</code>
        {arg.short ? <code className="flag-alias">{arg.short}</code> : null}
        {arg.positional ? <span className="flag-kind">positional</span> : null}
      </div>
      <div className="flag-body">
        {arg.help ? <p className="flag-help">{arg.help}</p> : null}
        <div className="flag-meta">
          {isBoolean ? <span className="flag-tag">flag</span> : null}
          {arg.default && arg.default !== 'false' ? (
            <span className="flag-tag">
              default: <code>{arg.default}</code>
            </span>
          ) : null}
          <ValueList values={arg.possibleValues} />
        </div>
      </div>
    </div>
  )
}

function CliReference() {
  return (
    <DocsLayout
      aside={
        <Toc
          items={[
            { id: 'global-options', title: 'Global options' },
            ...cli.subcommands.map((cmd) => ({
              id: `cmd-${cmd.name}`,
              title: `${cli.name} ${cmd.name}`,
            })),
          ]}
        />
      }
    >
      <div className="reference">
        <div className="page-head">
          <h1>CLI Reference</h1>
          <p>
            Generated from falcon's argument parser (v{cli.version}). {cli.about}.
          </p>
        </div>

        <CodeBlock code={cli.usage} lang="bash" filename="usage" />

        <CollapsibleSection id="global-options" title="Global options" className="ref-section">
          <p className="blurb">Accepted by every subcommand.</p>
          {cli.globalArgs.map((a) => (
            <FlagEntry key={a.name} arg={a} />
          ))}
        </CollapsibleSection>

        {cli.subcommands.map((cmd) => (
          <CollapsibleSection
            id={`cmd-${cmd.name}`}
            title={`${cli.name} ${cmd.name}`}
            key={cmd.name}
            className="ref-section"
          >
            <p className="blurb">{cmd.about}</p>
            {cmd.usage ? (
              <CodeBlock code={cmd.usage} lang="bash" filename="usage" />
            ) : null}
            {cmd.args.length > 0 ? (
              cmd.args.map((a) => <FlagEntry key={a.name} arg={a} />)
            ) : (
              <p className="blurb">No options.</p>
            )}
          </CollapsibleSection>
        ))}
      </div>
    </DocsLayout>
  )
}
