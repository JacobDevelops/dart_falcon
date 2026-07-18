import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import markdown from '../../content/getting-started.md?raw'

export const Route = createFileRoute('/docs/getting-started')({
  head: () => ({ meta: [{ title: 'Getting Started — falcon' }] }),
  component: () => <DocsPage markdown={markdown} />,
})
