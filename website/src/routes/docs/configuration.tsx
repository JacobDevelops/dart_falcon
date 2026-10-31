import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import { breadcrumbs, seo } from '../../lib/seo'
import source from '../../content/configuration.md?raw'

export const Route = createFileRoute('/docs/configuration')({
  head: () =>
    seo({
      title: 'Configuration | Dart Falcon',
      description:
        'Configure falcon with falcon.json: turn Dart and Flutter lint rules on by group, set severities and options, enable domains, override per path, and migrate from analysis_options.yaml.',
      path: '/docs/configuration',
      markdownPath: '/docs/configuration.md',
      type: 'article',
      jsonLd: [breadcrumbs([['Docs', '/docs/getting-started'], ['Configuration', '/docs/configuration']])],
    }),
  component: () => <DocsPage source={source} />,
})
