import { marked } from 'marked'
import { DocsLayout } from './DocsLayout'

marked.setOptions({ gfm: true })

export function DocsPage({ markdown }: { markdown: string }) {
  const html = marked.parse(markdown, { async: false }) as string
  return (
    <DocsLayout>
      <article className="prose" dangerouslySetInnerHTML={{ __html: html }} />
    </DocsLayout>
  )
}
