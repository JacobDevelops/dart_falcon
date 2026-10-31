import { createFileRoute } from '@tanstack/react-router'
import { documentMarkdown } from '../lib/agent-docs'

export const Route = createFileRoute('/docs/{$name}.md')({
  server: { handlers: { GET: ({ params }) => documentMarkdown(params.name) } },
})
