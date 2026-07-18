import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import markdown from '../../content/installation.md?raw'

export const Route = createFileRoute('/docs/installation')({
  head: () => ({ meta: [{ title: 'Installation — falcon' }] }),
  component: () => <DocsPage markdown={markdown} />,
})
