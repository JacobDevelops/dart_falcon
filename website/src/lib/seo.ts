import { REPO_URL } from './rules'

export const SITE_URL = 'https://dart-falcon.jacobdevelops.com'
const OG_IMAGE = `${SITE_URL}/og.png`

interface PageSeo {
  title: string
  description: string
  path: string
  type?: 'website' | 'article'
  jsonLd?: object[]
  noindex?: boolean
  markdownPath?: string
}

/** Per-route `head()` payload: title, description, canonical, OG/Twitter, JSON-LD. */
export function seo({ title, description, path, type = 'website', jsonLd = [], noindex, markdownPath }: PageSeo) {
  const url = `${SITE_URL}${path}`
  return {
    meta: [
      { title },
      { name: 'description', content: description },
      ...(noindex ? [{ name: 'robots', content: 'noindex' }] : []),
      { property: 'og:type', content: type },
      { property: 'og:title', content: title },
      { property: 'og:description', content: description },
      { property: 'og:url', content: url },
      { property: 'og:image', content: OG_IMAGE },
      { property: 'og:image:width', content: '1200' },
      { property: 'og:image:height', content: '630' },
      { property: 'og:image:alt', content: 'falcon, the fast Dart and Flutter linter' },
      { name: 'twitter:card', content: 'summary_large_image' },
      { name: 'twitter:title', content: title },
      { name: 'twitter:description', content: description },
      { name: 'twitter:image', content: OG_IMAGE },
    ],
    links: [
      ...(noindex ? [] : [{ rel: 'canonical', href: url }]),
      ...(markdownPath ? [{ rel: 'alternate', type: 'text/markdown', href: `${SITE_URL}${markdownPath}` }] : []),
    ],
    scripts: jsonLd.map((data) => ({
      type: 'application/ld+json',
      // `<` escaped so rule text can never close the script element.
      children: JSON.stringify({ '@context': 'https://schema.org', ...data }).replace(/</g, '\\u003c'),
    })),
  }
}

export function breadcrumbs(items: [name: string, path: string][]) {
  return {
    '@type': 'BreadcrumbList',
    itemListElement: items.map(([name, path], i) => ({
      '@type': 'ListItem',
      position: i + 1,
      name,
      item: `${SITE_URL}${path}`,
    })),
  }
}

export const SOFTWARE_APPLICATION = {
  '@type': 'SoftwareApplication',
  name: 'falcon',
  alternateName: 'dart_falcon',
  description:
    'A fast, standalone linter and language server for Dart and Flutter, written in Rust.',
  url: SITE_URL,
  sameAs: [REPO_URL, 'https://pub.dev/packages/falcon'],
  author: { '@type': 'Person', name: 'Jacob Sanderson', url: 'https://github.com/JacobDevelops' },
  publisher: { '@type': 'Person', name: 'Jacob Sanderson', url: 'https://github.com/JacobDevelops' },
  featureList: [
    'Dart and Flutter lint rules',
    'Cross-file analysis',
    'Language Server Protocol support',
    'JSON configuration and analyzer configuration migration',
    'Reason-required inline suppressions',
  ],
  applicationCategory: 'DeveloperApplication',
  applicationSubCategory: 'Linter',
  operatingSystem: 'Linux, macOS',
  license: 'https://opensource.org/licenses/MIT',
  codeRepository: REPO_URL,
  downloadUrl: `${REPO_URL}/releases/latest`,
  programmingLanguage: 'Rust',
  offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
}

export const WEBSITE = {
  '@type': 'WebSite',
  name: 'Dart Falcon',
  alternateName: ['falcon', 'dart_falcon'],
  url: SITE_URL,
  description: 'Documentation, lint rules, and benchmarks for the falcon Dart and Flutter linter.',
  inLanguage: 'en',
}
