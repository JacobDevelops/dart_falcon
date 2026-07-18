import type { ReactNode } from 'react'
import {
  Outlet,
  createRootRoute,
  HeadContent,
  Scripts,
  Link,
} from '@tanstack/react-router'
import appCss from '../styles/app.css?url'

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'falcon — a fast Dart & Flutter linter' },
      {
        name: 'description',
        content:
          'falcon is a fast, standalone Dart & Flutter linter written in Rust. No Dart SDK required. 148 rules, Biome-inspired config.',
      },
      { name: 'theme-color', content: '#101828' },
    ],
    links: [
      { rel: 'icon', href: '/falcon-icon.svg', type: 'image/svg+xml' },
      { rel: 'alternate icon', href: '/favicon.ico' },
      { rel: 'stylesheet', href: appCss },
    ],
  }),
  component: RootComponent,
})

function RootComponent() {
  return (
    <RootDocument>
      <div className="shell">
        <SiteNav />
        <Outlet />
        <SiteFooter />
      </div>
    </RootDocument>
  )
}

function SiteNav() {
  return (
    <header className="nav">
      <div className="wrap nav-inner">
        <Link to="/" className="brand">
          <img src="/falcon-mark.svg" alt="" />
          <span>
            falcon
          </span>
        </Link>
        <nav className="nav-links">
          <Link to="/docs/getting-started" activeProps={{ className: 'active' }}>
            Docs
          </Link>
          <Link to="/linter/rules" activeProps={{ className: 'active' }}>
            Rules
          </Link>
          <Link
            to="/reference/cli"
            activeProps={{ className: 'active' }}
            className="hide-sm"
          >
            Reference
          </Link>
          <a
            className="nav-gh"
            href="https://github.com/JacobDevelops/dart_falcon"
            target="_blank"
            rel="noreferrer"
          >
            GitHub
          </a>
        </nav>
      </div>
    </header>
  )
}

function SiteFooter() {
  return (
    <footer className="footer">
      <div className="wrap footer-cols">
        <div className="footer-brand">
          falcon — a fast Dart &amp; Flutter linter, written in Rust.
        </div>
        <div className="footer-group">
          <h5>Guides</h5>
          <Link to="/docs/getting-started">Getting started</Link>
          <Link to="/docs/installation">Installation</Link>
          <Link to="/docs/configuration">Configuration</Link>
          <Link to="/docs/suppressions">Suppressions</Link>
        </div>
        <div className="footer-group">
          <h5>Linter</h5>
          <Link to="/linter/rules">Rules</Link>
          <Link to="/linter/domains">Domains</Link>
        </div>
        <div className="footer-group">
          <h5>Reference</h5>
          <Link to="/reference/cli">CLI</Link>
          <Link to="/reference/configuration">Configuration keys</Link>
          <a
            href="https://github.com/JacobDevelops/dart_falcon"
            target="_blank"
            rel="noreferrer"
          >
            GitHub
          </a>
        </div>
      </div>
    </footer>
  )
}

function RootDocument({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}
