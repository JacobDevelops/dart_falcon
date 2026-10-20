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
import { Icon } from '../components/Icon'
import { REPO_URL } from '../lib/rules'

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { name: 'color-scheme', content: 'light dark' },
      { property: 'og:site_name', content: 'falcon' },
      { property: 'og:locale', content: 'en_US' },
    ],
    links: [
      { rel: 'icon', href: '/falcon-icon.svg', type: 'image/svg+xml' },
      { rel: 'alternate icon', href: '/favicon.ico' },
      { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' },
      { rel: 'manifest', href: '/site.webmanifest' },
      {
        rel: 'preload',
        href: '/fonts/bricolage-grotesque-800.woff2',
        as: 'font',
        type: 'font/woff2',
        crossOrigin: 'anonymous',
      },
      { rel: 'stylesheet', href: appCss },
    ],
  }),
  component: RootComponent,
  notFoundComponent: NotFound,
})

function RootComponent() {
  return (
    <RootDocument>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <SiteNav />
      <main id="main">
        <Outlet />
      </main>
      <SiteFooter />
    </RootDocument>
  )
}

const TOP_LINKS = [
  { to: '/docs/getting-started', label: 'Docs' },
  { to: '/linter/rules', label: 'Rules' },
  { to: '/benchmarks', label: 'Benchmarks' },
  { to: '/compare', label: 'Compare' },
  { to: '/reference/cli', label: 'Reference' },
] as const

function SiteNav() {
  return (
    <header className="nav">
      <div className="wrap nav-inner">
        <Link to="/" className="brand" aria-label="falcon home">
          <img src="/falcon-mark.svg" alt="" width="30" height="24" />
          <span>falcon</span>
        </Link>
        <nav className="nav-links" aria-label="Primary">
          {TOP_LINKS.map((l) => (
            <Link key={l.to} to={l.to} activeProps={{ 'aria-current': 'page' }}>
              {l.label}
            </Link>
          ))}
        </nav>
        <a className="nav-gh" href={REPO_URL} aria-label="falcon on GitHub">
          <Icon name="github" />
          <span>GitHub</span>
        </a>
      </div>
    </header>
  )
}

function SiteFooter() {
  return (
    <footer className="footer">
      <div className="wrap footer-cols">
        <div className="footer-brand">
          <img src="/falcon-mark.svg" alt="" width="30" height="24" />
          <p>
            falcon is a fast Dart &amp; Flutter linter and language server,
            written in Rust. MIT licensed.
          </p>
        </div>
        {NAV.map((group) => (
          <nav className="footer-group" key={group.heading} aria-label={group.heading}>
            <h2>{group.heading}</h2>
            {group.items.map((item) => (
              <Link to={item.to} key={item.to}>
                {item.label}
              </Link>
            ))}
          </nav>
        ))}
        <nav className="footer-group" aria-label="Project">
          <h2>Project</h2>
          <a href={REPO_URL}>GitHub</a>
          <a href={`${REPO_URL}/releases`}>Releases</a>
          <a href={`${REPO_URL}/issues`}>Issues</a>
        </nav>
      </div>
    </footer>
  )
}

function NotFound() {
  return (
    <div className="wrap notfound">
      {/* React 19 hoists these into <head>. */}
      <title>Page not found | falcon</title>
      <meta name="robots" content="noindex" />
      <p className="notfound-code" aria-hidden="true">
        404
      </p>
      <h1>Nothing at this address</h1>
      <p className="lede">
        The page or rule you followed doesn't exist. Rule ids are kebab-case,
        like <code>avoid-dynamic-calls</code>.
      </p>
      <ul className="link-list">
        <li>
          <Link to="/linter/rules">Browse all lint rules</Link>
        </li>
        <li>
          <Link to="/docs/getting-started">Getting started</Link>
        </li>
        <li>
          <Link to="/">Home</Link>
        </li>
      </ul>
    </div>
  )
}

function RootDocument({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
        {/* Outside head(): the router dedupes meta by name, keeping one theme. */}
        <meta name="theme-color" content="#edf1f3" media="(prefers-color-scheme: light)" />
        <meta name="theme-color" content="#0d1114" media="(prefers-color-scheme: dark)" />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}
