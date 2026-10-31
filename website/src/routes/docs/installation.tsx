import { createFileRoute } from '@tanstack/react-router'
import { DocsPage } from '../../components/DocsPage'
import { breadcrumbs, seo } from '../../lib/seo'
import source from '../../content/installation.md?raw'

export const Route = createFileRoute('/docs/installation')({
  head: () =>
    seo({
      title: 'Installation | Dart Falcon',
      description:
        'Install the falcon Dart and Flutter linter from prebuilt static binaries, mise, or source with cargo, plus VS Code and Neovim editor setup.',
      path: '/docs/installation',
      markdownPath: '/docs/installation.md',
      type: 'article',
      jsonLd: [breadcrumbs([['Docs', '/docs/getting-started'], ['Installation', '/docs/installation']])],
    }),
  component: () => <DocsPage source={source} />,
})
