import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import { breadcrumbs, seo } from '../../lib/seo'
import source from '../../content/suppressions.md?raw'

export const Route = createFileRoute('/docs/suppressions')({
  head: () =>
    seo({
      title: 'Suppress Dart lint warnings with falcon-ignore | falcon',
      description:
        'Silence a single falcon lint diagnostic or a whole file with falcon-ignore and falcon-ignore-all comments. Every suppression names one rule and requires a reason.',
      path: '/docs/suppressions',
      type: 'article',
      jsonLd: [breadcrumbs([['Docs', '/docs/getting-started'], ['Suppressions', '/docs/suppressions']])],
    }),
  component: () => <DocsPage source={source} />,
})
