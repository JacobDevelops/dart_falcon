import { useState, type ReactNode } from 'react'
import { Link } from '@tanstack/react-router'
import { Icon } from './Icon'

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

/**
 * Docs shell: a sticky sidebar with the site IA, collapsible on small screens.
 * `aside` adds an in-page table of contents beneath the primary nav.
 */
export function DocsLayout({
  children,
  aside,
}: {
  children: ReactNode
  aside?: ReactNode
}) {
  const [open, setOpen] = useState(false)
  return (
    <div className="wrap layout">
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
                {group.items.map((item) => (
                  <Link
                    key={item.to}
                    to={item.to}
                    activeProps={{ 'aria-current': 'page' }}
                    onClick={() => setOpen(false)}
                  >
                    {item.label}
                  </Link>
                ))}
              </div>
            ))}
          </nav>
          {aside}
        </aside>
      </div>
      <div className="content">{children}</div>
    </div>
  )
}
