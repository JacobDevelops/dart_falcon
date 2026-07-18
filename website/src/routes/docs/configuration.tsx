import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import markdown from '../../content/configuration.md?raw'

export const Route = createFileRoute('/docs/configuration')({
  head: () => ({ meta: [{ title: 'Configuration — falcon' }] }),
  component: () => <DocsPage markdown={markdown} />,
})
