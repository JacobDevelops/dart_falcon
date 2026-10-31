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
import { seo, SOFTWARE_APPLICATION, WEBSITE } from '../lib/seo'
import cli from '../data/cli.json'

const FAQ = [
  { q: 'What is falcon?', a: 'falcon is a free Dart linter and Flutter linter written in Rust. It runs built-in Dart lint rules and cross-file checks from a standalone binary, with a language server for editors.' },
  { q: 'How is falcon different from dart analyze, DCM, and custom_lint?', a: 'falcon runs outside the Dart analyzer. It is a dart analyze alternative for linting, not type checking: keep dart analyze for compile errors. As a DCM alternative it offers MIT-licensed lint rules without seat or line limits. As a custom_lint alternative it provides built-in framework rules rather than an analyzer plugin framework.' },
  { q: 'Does falcon need the Dart SDK?', a: 'falcon parses Dart itself without starting the Dart analyzer. Rules that need SDK or package types require the Dart SDK and resolved dependencies. Installing through dart pub also requires Dart; release binaries do not.' },
  { q: 'Is falcon free?', a: 'Yes. falcon and all its rules are MIT licensed, with no seat or lines-of-code limits.' },
  { q: 'How do I install falcon?', a: 'On Linux x86_64 or ARM64 and macOS Apple Silicon, run dart pub global activate falcon, add the pub cache bin directory to PATH, then run falcon check . in your project. Release binaries and source installation are also available; Windows and Intel Mac binaries are not currently distributed.' },
  { q: 'Which editors work with falcon?', a: 'falcon provides a Language Server Protocol server, VS Code and Zed extensions, and documented Neovim configuration. Install the falcon binary on PATH before connecting your editor.' },
]

export const Route = createFileRoute('/')({
  loader: () => getHomeData(),
  head: ({ loaderData }) =>
    seo({
      title: 'Dart Falcon: the fast Dart linter',
      description: `A free Dart linter and Flutter linter with ${loaderData?.coverage.total ?? ''} Dart lint rules, cross-file checks and editor support. Compare falcon with dart analyze, DCM and custom_lint.`,
      path: '/',
      jsonLd: [
        { ...SOFTWARE_APPLICATION, softwareVersion: cli.version },
        WEBSITE,
        {
          '@type': 'FAQPage',
          mainEntity: FAQ.map(({ q, a }) => ({
            '@type': 'Question',
            name: q,
            acceptedAnswer: { '@type': 'Answer', text: a },
          })),
        },
      ],
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
              The fast Dart linter <span className="nowrap">&amp; Flutter linter</span>
            </h1>
            <p className="lede">
              falcon runs {coverage.total} Dart lint rules for Dart and Flutter
              projects from a single Rust binary. It parses Dart itself, so it never
              boots the Dart analyzer, and it speaks LSP for your editor.
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
                  <Link to="/linter/groups/$group" params={{ group: g.id }} className="ledger-name">
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
                    <Link to="/linter/domains/$domain" params={{ domain: d.name }}>
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

      <section className="section section-band" aria-labelledby="home-faq-h">
        <div className="wrap">
          <div className="rule-section faq">
            <h2 id="home-faq-h">Dart and Flutter linting questions</h2>
            {FAQ.map(({ q, a }) => (
              <details key={q}>
                <summary>{q}</summary>
                <p>{a}</p>
              </details>
            ))}
            <p>
              Explore <Link to="/linter/domains/$domain" params={{ domain: 'flutter' }}>Flutter lint rules</Link>,{' '}
              <Link to="/linter/domains/$domain" params={{ domain: 'riverpod' }}>Riverpod rules</Link>,{' '}
              <Link to="/linter/domains/$domain" params={{ domain: 'bloc' }}>Bloc rules</Link>, or the{' '}
              <Link to="/linter/rules">complete Dart lint rules catalog</Link>.{' '}
              <Link to="/compare">Compare Dart linters</Link> before migrating.
            </p>
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
