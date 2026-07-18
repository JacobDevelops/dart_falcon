import { createFileRoute, Link } from '@tanstack/react-router'
import { Fragment } from 'react'
import { CodeBlock } from '../components/CodeBlock'
import { highlightDart } from '../lib/highlight'
import { rules, recommendedCount, heroRule } from '../lib/rules'

export const Route = createFileRoute('/')({
  component: Home,
})

const INSTALL = `# install with nix
nix profile install github:JacobDevelops/dart_falcon

# lint your project — no Dart SDK required
falcon check lib/`

function Home() {
  return (
    <main>
      <Hero />
      <Features />
      <QuickStart />
    </main>
  )
}

function Hero() {
  return (
    <section className="hero">
      <div className="wrap hero-grid">
        <div>
          <span className="eyebrow">Rust-powered · standalone</span>
          <h1>
            Lint Dart &amp; Flutter <span className="squiggle">without</span> the
            Dart SDK.
          </h1>
          <p className="lede">
            falcon is a fast, self-contained linter written in Rust. Point it at
            your code and get {rules.length} rules of Dart &amp; Flutter analysis
            — no toolchain, no waiting.
          </p>
          <div className="hero-cta">
            <Link to="/docs/installation" className="btn btn-primary">
              Get started
            </Link>
            <Link to="/linter/rules" className="btn btn-ghost">
              Browse rules
            </Link>
          </div>
          <div className="stat-row">
            <div className="stat">
              <b>{rules.length}</b>
              <span>lint rules</span>
            </div>
            <div className="stat">
              <b>{recommendedCount}</b>
              <span>recommended</span>
            </div>
            <div className="stat">
              <b>0</b>
              <span>Dart SDK needed</span>
            </div>
          </div>
        </div>
        <HeroEditor />
      </div>
    </section>
  )
}

function HeroEditor() {
  const rule = heroRule()
  const bad = rule?.examples.bad ?? ''
  const lines = bad.split('\n')
  const flagged = new Set(rule?.diagnostics.map((d) => d.line) ?? [])
  const firstDiag = rule?.diagnostics[0]

  return (
    <div className="editor" aria-hidden="true">
      <div className="editor-bar">
        <span className="dot" />
        <span className="dot" />
        <span className="dot" />
        <span className="fname">example.dart</span>
      </div>
      <div className="ed-rows">
        {lines.map((line, i) => {
          const n = i + 1
          const isFlagged = flagged.has(n)
          return (
            <Fragment key={i}>
              <div className={`erow${isFlagged ? ' flagged' : ''}`}>
                <span className="enum">{n}</span>
                <code
                  className="ecode"
                  dangerouslySetInnerHTML={{
                    __html: highlightDart(line) || ' ',
                  }}
                />
              </div>
              {firstDiag && firstDiag.line === n ? (
                <div className="diag-callout">
                  <b>{rule?.name}</b>
                  {firstDiag.message}
                </div>
              ) : null}
            </Fragment>
          )
        })}
      </div>
    </div>
  )
}

function Features() {
  const items = [
    {
      ic: '⚡',
      title: 'Fast by default',
      body: 'A Rust engine analyzes whole projects in a fraction of the time a Dart-based analyzer takes.',
    },
    {
      ic: '📦',
      title: 'Zero dependencies',
      body: 'A single binary. No Dart or Flutter SDK, no pub, no analysis server to boot.',
    },
    {
      ic: '🎯',
      title: 'Real diagnostics',
      body: 'Every rule ships tested against golden fixtures — the examples in these docs are its actual output.',
    },
    {
      ic: '🔗',
      title: 'Cross-file analysis',
      body: 'Finds unused code, unused files, and unnecessary nullability across your entire project graph.',
    },
    {
      ic: '🛠️',
      title: 'Biome-inspired config',
      body: 'One falcon.json, rules grouped by intent, domain toggles for Flutter. Familiar and predictable.',
    },
    {
      ic: '🪶',
      title: 'Curated rule set',
      body: `${recommendedCount} recommended rules distilled from Dart lints, DCM, and Pyramid Lint — plus falcon originals.`,
    },
  ]
  return (
    <section className="section section-alt">
      <div className="wrap">
        <h2>Built for speed and precision</h2>
        <p className="sub">
          Everything a Dart or Flutter team needs from a linter, packed into a
          binary that runs anywhere.
        </p>
        <div className="feature-grid">
          {items.map((f) => (
            <div className="feature" key={f.title}>
              <div className="ic">{f.ic}</div>
              <h3>{f.title}</h3>
              <p>{f.body}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

function QuickStart() {
  return (
    <section className="section">
      <div className="wrap">
        <h2>Up and running in one command</h2>
        <p className="sub">
          Install the binary, then check your code. falcon reads an optional
          falcon.json for configuration.
        </p>
        <div className="install">
          <CodeBlock code={INSTALL} lang="bash" filename="terminal" />
          <div style={{ marginTop: '18px', display: 'flex', gap: '12px' }}>
            <Link to="/docs/installation" className="btn btn-ghost">
              Installation guide
            </Link>
            <Link to="/docs/configuration" className="btn btn-ghost">
              Configuration
            </Link>
          </div>
        </div>
      </div>
    </section>
  )
}
