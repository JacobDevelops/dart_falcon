import { createFileRoute, Link } from '@tanstack/react-router'
import { Fragment, type CSSProperties } from 'react'
import { BenchSummaryChart } from '../components/BenchChart'
import { CodeBlock } from '../components/CodeBlock'
import { CopyCommand } from '../components/CopyCommand'
import { Icon } from '../components/Icon'
import { Tabs } from '../components/Tabs'
import { scenarioSummaries, summaryText } from '../lib/benchmarks'
import { highlightDart } from '../lib/highlight'
import { REPO_URL } from '../lib/rules'
import { getHomeData } from '../lib/server'
import { seo, SOFTWARE_APPLICATION } from '../lib/seo'
import cli from '../data/cli.json'

export const Route = createFileRoute('/')({
  loader: () => getHomeData(),
  head: ({ loaderData }) =>
    seo({
      title: 'falcon: the fast Dart & Flutter linter, written in Rust',
      description: `falcon is a fast, standalone Dart and Flutter linter and language server written in Rust. ${loaderData?.coverage.total} lint rules including the official Dart lints, cross-file checks, one falcon.json. A free dart analyze, DCM and custom_lint alternative.`,
      path: '/',
      jsonLd: [{ ...SOFTWARE_APPLICATION, softwareVersion: cli.version }],
    }),
  component: Home,
})

const INSTALL = `curl -fsSL ${REPO_URL}/releases/latest/download/falcon-x86_64-linux.tar.gz | tar -xz`

const INSTALL_PUB = `# Linux and macOS; needs Dart's pub cache bin directory on PATH
dart pub global activate falcon

falcon check .`

const INSTALL_BINARY = `# Linux x86_64; see Installation for aarch64 and macOS
${INSTALL}
sudo mv falcon /usr/local/bin/

falcon check .`

const INSTALL_CARGO = `# build from source with a nightly Rust toolchain
cargo +nightly install --git ${REPO_URL} dart_falcon

falcon check .`

const INSTALL_MISE = `[tools]
"github:${REPO_URL.slice('https://github.com/'.length)}" = { version = "${cli.version}", matching = "falcon-" }

# then: mise install && mise exec -- falcon check .`

function Home() {
  const { coverage, hero, bench } = Route.useLoaderData()
  const [lead, ...rest] = scenarioSummaries(bench)
  const maxGroup = Math.max(...coverage.groups.map((g) => g.count))
  return (
    <>
      <section className="hero" aria-labelledby="hero-h">
        <div className="wrap hero-grid">
          <div className="hero-copy">
            <h1 id="hero-h">
              The fast linter for <span className="nowrap">Dart &amp; Flutter</span>
            </h1>
            <p className="lede">
              falcon lints Dart and Flutter projects with {coverage.total} rules
              from a single Rust binary. It parses Dart itself, so it never boots
              the Dart analyzer, and it speaks LSP for your editor.
            </p>
            <CopyCommand command="dart pub global activate falcon" />
            <div className="hero-cta">
              <Link to="/docs/getting-started" className="btn btn-primary">
                Get started <Icon name="arrow" />
              </Link>
              <Link to="/linter/rules" className="btn btn-ghost">
                Browse {coverage.total} rules
              </Link>
            </div>
          </div>
          <div className="hero-figure">
            {lead ? (
              <>
                <BenchSummaryChart summary={lead} />
                <ul className="bench-notes">
                  {rest.map((s) => (
                    <li key={s.scenario.id}>{summaryText(s)}</li>
                  ))}
                  <li>
                    Each scenario enables matching rule names within its rule family.
                    Matching names do not imply identical work or diagnostic output.
                    {' '}When measured, <code>dart analyze</code> also type-checks;
                    falcon does not.
                  </li>
                </ul>
                <Link to="/benchmarks" className="more-link">
                  All {bench.runs.length} runs and methodology <Icon name="arrow" />
                </Link>
              </>
            ) : null}
          </div>
        </div>
      </section>

      {hero ? (
        <section className="section" aria-labelledby="output-h">
          <div className="wrap split">
            <div className="split-copy">
              <h2 id="output-h">Real diagnostics, pinned to the line</h2>
              <p>
                This is falcon's actual output for{' '}
                <Link to="/linter/rules/$rule" params={{ rule: hero.name }}>
                  <code>{hero.name}</code>
                </Link>
                , taken from the rule's test fixture. Every rule page shows the
                same: the code that triggers it, the exact message, and the fix.
              </p>
              <ul className="ticks">
                <li>One <code>falcon.json</code>, rules grouped by intent</li>
                <li>Inline suppressions that must state a reason</li>
                <li>
                  <code>falcon migrate</code> reads your{' '}
                  <code>analysis_options.yaml</code>
                </li>
                <li>
                  Text or JSON output for CI, and LSP for your editor
                </li>
              </ul>
            </div>
            <AnnotatedCode name={hero.name} source={hero.bad} diagnostics={hero.diagnostics} />
          </div>
        </section>
      ) : null}

      <section className="section section-band" aria-labelledby="coverage-h">
        <div className="wrap">
          <h2 id="coverage-h">{coverage.total} rules, sorted by what they catch</h2>
          <p className="section-sub">
            {coverage.mappedToOfficial} rules implement official Dart lints and
            keep their analyzer names, so <code>falcon migrate</code> can carry
            your <code>analysis_options.yaml</code> over. The rest come from Dart
            Code Metrics, Pyramid Lint, and falcon itself.
          </p>
          <div className="coverage">
            <ol className="ledger" aria-label="Rules per group">
              {coverage.groups.map((g) => (
                <li key={g.id}>
                  <Link to="/linter/rules" hash={g.id} className="ledger-name">
                    {g.label}
                  </Link>
                  <span className="ledger-bar" aria-hidden="true">
                    <span style={{ width: `${(g.count / maxGroup) * 100}%` }} />
                  </span>
                  <span className="num">{g.count}</span>
                </li>
              ))}
            </ol>
            <div className="coverage-side">
              <h3>Framework domains</h3>
              <ul className="chips">
                {coverage.domains.map((d) => (
                  <li key={d.name}>
                    <Link to="/linter/domains" hash={d.name}>
                      {d.name} <span className="num">{d.count}</span>
                    </Link>
                  </li>
                ))}
              </ul>
              <h3>Where the rules come from</h3>
              <dl className="sources">
                {coverage.sources.map((s) => (
                  <div key={s.label}>
                    <dt>{s.label}</dt>
                    <dd className="num">{s.count}</dd>
                  </div>
                ))}
              </dl>
            </div>
          </div>
        </div>
      </section>

      <section className="section" aria-labelledby="switch-h">
        <div className="wrap split">
          <div className="split-copy">
            <h2 id="switch-h">Coming from dart analyze, DCM, or custom_lint?</h2>
            <p>
              falcon runs beside the analyzer, not inside it, so it adds no
              plugin to your analysis server.
              {lead ? ` ${summaryText(lead)}` : ''}{' '}
              Every rule is free and MIT licensed.
            </p>
            <Link to="/compare" className="more-link">
              Compare falcon with other Dart linters <Icon name="arrow" />
            </Link>
          </div>
          <div className="install">
            <h3 className="install-h">Install</h3>
            <Tabs
              tabs={[
                { id: 'pub', label: 'dart pub', content: <CodeBlock code={INSTALL_PUB} lang="bash" filename="terminal" /> },
                { id: 'binary', label: 'Binary', content: <CodeBlock code={INSTALL_BINARY} lang="bash" filename="terminal" /> },
                { id: 'mise', label: 'mise', content: <CodeBlock code={INSTALL_MISE} lang="toml" filename="mise.toml" /> },
                { id: 'cargo', label: 'Cargo', content: <CodeBlock code={INSTALL_CARGO} lang="bash" filename="terminal" /> },
              ]}
            />
            <Link to="/docs/installation" className="more-link">
              All platforms and editor extensions <Icon name="arrow" />
            </Link>
          </div>
        </div>
      </section>
    </>
  )
}

function AnnotatedCode({
  name,
  source,
  diagnostics,
}: {
  name: string
  source: string
  diagnostics: { line: number; column: number; message: string }[]
}) {
  const first = diagnostics[0]
  return (
    <figure className="editor">
      <figcaption className="editor-bar">
        <span>example.dart</span>
        <span>
          {diagnostics.length} {diagnostics.length === 1 ? 'diagnostic' : 'diagnostics'}
        </span>
      </figcaption>
      <pre className="editor-code">
        {source.split('\n').map((line, i) => {
          const n = i + 1
          const flagged = diagnostics.some((d) => d.line === n)
          return (
            <Fragment key={n}>
              <span className={`eline${flagged ? ' flagged' : ''}`}>
                <span className="enum" aria-hidden="true">
                  {n}
                </span>
                <code dangerouslySetInnerHTML={{ __html: highlightDart(line) || ' ' }} />
              </span>
              {first && first.line === n ? (
                <span className="fieldmark" style={{ '--col': first.column } as CSSProperties}>
                  <b>{name}</b> {first.message}
                </span>
              ) : null}
            </Fragment>
          )
        })}
      </pre>
    </figure>
  )
}
