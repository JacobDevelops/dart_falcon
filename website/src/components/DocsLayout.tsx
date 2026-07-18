import { useState, type ReactNode } from 'react'
import { Link } from '@tanstack/react-router'

interface NavItem {
  to: string
  label: string
}

interface NavGroup {
  heading: string
  items: NavItem[]
}

// The single source of truth for the docs IA: Guides → Linter → Reference.
const NAV: NavGroup[] = [
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
    heading: 'Reference',
    items: [
      { to: '/reference/cli', label: 'CLI' },
      { to: '/reference/configuration', label: 'Configuration keys' },
    ],
  },
]

function DocsNav({ onNavigate }: { onNavigate?: () => void }) {
  return (
    <nav aria-label="Documentation">
      {NAV.map((group) => (
        <div key={group.heading}>
          <h4>{group.heading}</h4>
          {group.items.map((item) => (
            <Link
              key={item.to}
              to={item.to}
              activeProps={{ className: 'active' }}
              activeOptions={{ exact: false }}
              onClick={onNavigate}
            >
              {item.label}
            </Link>
          ))}
        </div>
      ))}
    </nav>
  )
}

/**
 * Consistent docs shell: a sticky left sidebar with the three-tier IA on every
 * docs/linter/reference page, plus a mobile disclosure toggle. `aside` lets a
 * page add an in-page table of contents beneath the primary nav.
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
    <div className="wrap">
      <div className="layout">
        <div className="sidebar-col">
          <button
            type="button"
            className="docs-menu-toggle"
            aria-expanded={open}
            onClick={() => setOpen((v) => !v)}
          >
            <span className="menu-ico">{open ? '✕' : '☰'}</span> Documentation menu
          </button>
          <aside className={`sidebar${open ? ' open' : ''}`}>
            <DocsNav onNavigate={() => setOpen(false)} />
            {aside}
          </aside>
        </div>
        <div className="content">{children}</div>
      </div>
    </div>
  )
}
