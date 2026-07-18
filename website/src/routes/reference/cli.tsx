import { createFileRoute } from '@tanstack/react-router'
import { DocsLayout } from '../../components/DocsLayout'
import { CodeBlock } from '../../components/CodeBlock'
import cliJson from '../../data/cli.json'

export const Route = createFileRoute('/reference/cli')({
  head: () => ({ meta: [{ title: 'CLI Reference — falcon' }] }),
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
          {arg.possibleValues.length > 0 ? (
            <span className="flag-tag">
              values:{' '}
              {arg.possibleValues.map((v, i) => (
                <span key={v}>
                  {i > 0 ? ' | ' : ''}
                  <code>{v}</code>
                </span>
              ))}
            </span>
          ) : null}
        </div>
      </div>
    </div>
  )
}

function CliReference() {
  return (
    <DocsLayout
      aside={
        <nav className="toc" aria-label="Commands">
          <h4>Commands</h4>
          {cli.subcommands.map((c) => (
            <a key={c.name} href={`#cmd-${c.name}`}>
              {cli.name} {c.name}
            </a>
          ))}
        </nav>
      }
    >
      <div className="reference">
        <div className="rules-head">
          <h1>CLI Reference</h1>
          <p>
            Generated from falcon's argument parser (v{cli.version}). {cli.about}.
          </p>
        </div>

        <CodeBlock code={cli.usage} lang="bash" filename="usage" />

        <section className="ref-section">
          <h2>Global options</h2>
          <p className="blurb">Accepted by every subcommand.</p>
          {cli.globalArgs.map((a) => (
            <FlagEntry key={a.name} arg={a} />
          ))}
        </section>

        {cli.subcommands.map((cmd) => (
          <section className="ref-section" id={`cmd-${cmd.name}`} key={cmd.name}>
            <h2>
              <code>
                {cli.name} {cmd.name}
              </code>
            </h2>
            <p className="blurb">{cmd.about}</p>
            {cmd.usage ? (
              <CodeBlock code={cmd.usage} lang="bash" filename="usage" />
            ) : null}
            {cmd.args.length > 0 ? (
              cmd.args.map((a) => <FlagEntry key={a.name} arg={a} />)
            ) : (
              <p className="blurb">No options.</p>
            )}
          </section>
        ))}
      </div>
    </DocsLayout>
  )
}
