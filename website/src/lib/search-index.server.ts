import { DOC_PAGES } from './docs'
import { configKeyId, extractHeadings } from './headings'
import { DOMAIN_BLURBS, domainLabel } from './rules'
import { coverage, summaries } from './rules.data'
import type { SearchEntry } from './search'
import configReference from '../data/config-reference.json'

const sources = import.meta.glob<string>('../content/*.md', { query: '?raw', import: 'default', eager: true })
const referencePages = [
  { path: '/reference/cli', title: 'CLI reference', description: 'Commands and flags for linting, fixes, migration, and the language server.', source: '' },
  { path: '/reference/configuration', title: 'Configuration reference', description: 'All falcon configuration keys, types, and defaults.', source: '' },
]
const pages = DOC_PAGES.flatMap((page) => {
  const summary = { path: page.route, title: page.title, description: page.description, source: sources[`../content/${page.name}.md`] }
  return page.name === 'configuration' ? [summary, ...referencePages] : [summary]
})

const catalog = coverage()

export const SEARCH_INDEX: SearchEntry[] = [
  ...summaries.map((rule) => ({
    id: rule.name,
    title: rule.name,
    description: rule.description,
    path: `/linter/rules/${rule.name}`,
    hash: '',
    kind: 'Rule' as const,
    aliases: [rule.name.replace(/-/g, '_'), ...(rule.upstreamId ? [rule.upstreamId] : [])],
  })),
  ...catalog.groups.map((group) => ({
    id: `group:${group.id}`,
    title: `${group.label} rules`,
    description: `${group.blurb} ${group.count} rules · ${group.recommended} recommended.`,
    path: `/linter/groups/${group.id}`,
    hash: '',
    kind: 'Group' as const,
    aliases: [group.id, group.label],
  })),
  ...catalog.domains.map((domain) => ({
    id: `domain:${domain.name}`,
    title: `${domainLabel(domain.name)} lint rules`,
    description: `${DOMAIN_BLURBS[domain.name]} ${domain.count} rules.`,
    path: `/linter/domains/${domain.name}`,
    hash: '',
    kind: 'Domain' as const,
    aliases: [domain.name, domainLabel(domain.name)],
  })),
  ...configReference.map((entry) => ({
    id: `config:${entry.path}`,
    title: entry.path,
    description: `${entry.type} · ${entry.description}`,
    path: '/reference/configuration',
    hash: configKeyId(entry.path),
    kind: 'Config key' as const,
    aliases: [entry.path.replace(/-/g, '_')],
  })),
  ...pages.flatMap((page) => [
    { id: page.path, title: page.title, description: page.description, path: page.path, hash: '', kind: 'Guide' as const, aliases: [] },
    ...extractHeadings(page.source).map((heading) => ({
      id: `${page.path}#${heading.id}`,
      title: heading.title,
      description: page.title,
      path: page.path,
      hash: heading.id,
      kind: 'Section' as const,
      aliases: [heading.id],
    })),
  ]),
]
