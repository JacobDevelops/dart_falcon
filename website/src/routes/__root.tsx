import type { ReactNode } from 'react'
import {
  Outlet,
  createRootRoute,
  HeadContent,
  Scripts,
  Link,
} from '@tanstack/react-router'
import appCss from '../styles/app.css?url'
import { NAV } from '../components/DocsLayout'
import { rules, REPO_URL } from '../lib/rules'

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'falcon — a fast Dart & Flutter linter' },
      {
        name: 'description',
        content:
          `falcon is a fast, standalone Dart & Flutter linter written in Rust. No Dart SDK required. ${rules.length} rules, Biome-inspired config.`,
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
            href={REPO_URL}
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
        {NAV.map((group) => (
          <div className="footer-group" key={group.heading}>
            <h5>{group.heading}</h5>
            {group.items.map((item) => (
              <Link to={item.to} key={item.to}>{item.label}</Link>
            ))}
            {group.heading === 'Reference' ? (
              <a href={REPO_URL} target="_blank" rel="noreferrer">
                GitHub
              </a>
            ) : null}
          </div>
        ))}
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
