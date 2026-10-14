import { marked } from 'marked'
import { DocsLayout } from './DocsLayout'

export function DocsPage({ markdown }: { markdown: string }) {
  const html = marked.parse(markdown, { async: false }) as string
  return (
    <DocsLayout>
      <article className="prose" dangerouslySetInnerHTML={{ __html: html }} />
    </DocsLayout>
  )
}
