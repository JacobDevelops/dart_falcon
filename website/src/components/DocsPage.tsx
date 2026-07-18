import { marked } from 'marked'
import { Link } from '@tanstack/react-router'
import { SidebarLayout } from './SidebarLayout'

marked.setOptions({ gfm: true })

function DocsSidebar() {
  return (
    <nav>
      <h4>Documentation</h4>
      <Link to="/docs/installation" activeProps={{ className: 'active' }}>
        Installation
      </Link>
      <Link to="/docs/configuration" activeProps={{ className: 'active' }}>
        Configuration
      </Link>
      <h4>Linter</h4>
      <Link to="/linter/rules" activeProps={{ className: 'active' }}>
        Rules
      </Link>
      <Link to="/linter/domains" activeProps={{ className: 'active' }}>
        Domains
      </Link>
    </nav>
  )
}

export function DocsPage({ markdown }: { markdown: string }) {
  const html = marked.parse(markdown, { async: false }) as string
  return (
    <SidebarLayout sidebar={<DocsSidebar />}>
      <article
        className="prose"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </SidebarLayout>
  )
}
