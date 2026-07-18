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
          <Link
            to="/docs/installation"
            activeProps={{ className: 'active' }}
            className="hide-sm"
          >
            Docs
          </Link>
          <Link to="/linter/rules" activeProps={{ className: 'active' }}>
            Rules
          </Link>
          <Link
            to="/linter/domains"
            activeProps={{ className: 'active' }}
            className="hide-sm"
          >
            Domains
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
      <div className="wrap footer-inner">
        <div>
          falcon — a fast Dart &amp; Flutter linter, written in Rust.
        </div>
        <div style={{ display: 'flex', gap: '18px' }}>
          <Link to="/docs/installation">Install</Link>
          <Link to="/docs/configuration">Configure</Link>
          <Link to="/linter/rules">Rules</Link>
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
