import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import { breadcrumbs, seo } from '../../lib/seo'
import source from '../../content/getting-started.md?raw'

export const Route = createFileRoute('/docs/getting-started')({
  head: () =>
    seo({
      title: 'Getting started | Dart Falcon',
      description:
        'Install falcon, run your first Dart and Flutter lint check, configure falcon.json, wire it into CI, and connect the language server to your editor.',
      path: '/docs/getting-started',
      markdownPath: '/docs/getting-started.md',
      type: 'article',
      jsonLd: [breadcrumbs([['Docs', '/docs/getting-started'], ['Getting started', '/docs/getting-started']])],
    }),
  component: () => <DocsPage source={source} />,
})
