import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import markdown from '../../content/suppressions.md?raw'

export const Route = createFileRoute('/docs/suppressions')({
  head: () => ({ meta: [{ title: 'Suppressions — falcon' }] }),
  component: () => <DocsPage markdown={markdown} />,
})
