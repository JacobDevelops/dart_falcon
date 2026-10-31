import { useState, type ReactNode } from 'react'
import { Link, useLoaderData, useLocation } from '@tanstack/react-router'
import { Icon } from './Icon'
import { domainLabel } from '../lib/rules'

interface NavItem {
  to: string
  label: string
}

interface NavGroup {
  heading: string
  items: NavItem[]
}

export const NAV: NavGroup[] = [
  {
    heading: 'Guides',
    items: [
      { to: '/docs/getting-started', label: 'Getting started' },
      { to: '/docs/installation', label: 'Installation' },
      { to: '/docs/configuration', label: 'Configuration' },
      { to: '/docs/suppressions', label: 'Suppressions' },
    ],
  },
  {
    heading: 'Linter',
    items: [
      { to: '/linter/rules', label: 'Rules' },
      { to: '/linter/domains', label: 'Domains' },
    ],
  },
  {
    heading: 'Why falcon',
    items: [
      { to: '/benchmarks', label: 'Benchmarks' },
      { to: '/compare', label: 'Compare linters' },
    ],
  },
  {
    heading: 'Reference',
    items: [
      { to: '/reference/cli', label: 'CLI' },
      { to: '/reference/configuration', label: 'Configuration keys' },
    ],
  },
]

function CatalogNav({ label, expanded, link, children }: { label: string; expanded: boolean; link: ReactNode; children: ReactNode }) {
  const [open, setOpen] = useState(expanded)
  const id = `docs-${label.toLowerCase()}`
  return (
    <div className="side-catalog">
      <div className="side-catalog-heading">
        {link}
        <button type="button" aria-label={`${label} navigation`} aria-expanded={open} aria-controls={id} onClick={() => setOpen((value) => !value)}>
          <Icon name="chevron" />
        </button>
      </div>
      <div id={id} hidden={!open}>{children}</div>
    </div>
  )
}

/**
 * Docs shell: sticky site navigation and an optional right-hand page outline.
 * The outline becomes an inline disclosure above the content on small screens.
 */
export function DocsLayout({
  children,
  aside,
}: {
  children: ReactNode
  aside?: ReactNode
}) {
  const [open, setOpen] = useState(false)
  const [outlineOpen, setOutlineOpen] = useState(false)
  const coverage = useLoaderData({ from: '__root__' })
  const pathname = useLocation({ select: (location) => location.pathname })
  const catalogs = {
    '/linter/rules': {
      open: pathname.startsWith('/linter/rules') || pathname.startsWith('/linter/groups'),
      items: coverage.groups.map((group) => ({
        to: '/linter/groups/$group' as const,
        params: { group: group.id },
        label: group.label,
        count: group.count,
      })),
    },
    '/linter/domains': {
      open: pathname.startsWith('/linter/domains'),
      items: coverage.domains.map((domain) => ({
        to: '/linter/domains/$domain' as const,
        params: { domain: domain.name },
        label: domainLabel(domain.name),
        count: domain.count,
      })),
    },
  }
  return (
    <div className={`wrap layout${aside ? ' layout-with-outline' : ''}`}>
      <div className="sidebar-col">
        <button
          type="button"
          className="docs-menu-toggle"
          aria-expanded={open}
          aria-controls="docs-sidebar"
          onClick={() => setOpen((v) => !v)}
        >
          <Icon name={open ? 'close' : 'menu'} /> Documentation menu
        </button>
        <aside id="docs-sidebar" className={`sidebar${open ? ' open' : ''}`}>
          <nav aria-label="Documentation">
            {NAV.map((group) => (
              <div key={group.heading} className="side-group">
                <p className="side-heading">{group.heading}</p>
                {group.items.map((item) => {
                  const catalog = catalogs[item.to as keyof typeof catalogs]
                  const link = (
                    <Link to={item.to} activeOptions={{ exact: true }} activeProps={{ 'aria-current': 'page' }} onClick={() => setOpen(false)}>
                      {item.label}
                    </Link>
                  )
                  return catalog ? (
                    <CatalogNav key={item.to} label={item.label} expanded={catalog.open} link={link}>
                      <ul className="side-nested">
                        {catalog.items.map((entry) => (
                          <li key={entry.to + entry.label}>
                            <Link
                              to={entry.to}
                              params={entry.params}
                              activeOptions={{ exact: true }}
                              activeProps={{ 'aria-current': 'page' }}
                              aria-label={`${entry.label}, ${entry.count} rules`}
                              onClick={() => setOpen(false)}
                            >
                              <span>{entry.label}</span>
                              <span className="side-count" aria-hidden="true">{entry.count}</span>
                            </Link>
                          </li>
                        ))}
                      </ul>
                    </CatalogNav>
                  ) : <div key={item.to}>{link}</div>
                })}
              </div>
            ))}
          </nav>
        </aside>
      </div>
      <div className="content">{children}</div>
      {aside ? (
        <aside className="page-outline">
          <details
            className="page-outline-disclosure"
            open={outlineOpen}
            onToggle={(event) => setOutlineOpen(event.currentTarget.open)}
          >
            <summary>On this page</summary>
            <div className="page-outline-content">{aside}</div>
          </details>
        </aside>
      ) : null}
    </div>
  )
}
