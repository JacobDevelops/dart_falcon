import { GROUPS, REPO_URL, ruleGuidance, type Rule } from './rules'
import { domains, rules } from './rules.data'
import { SITE_URL } from './seo'
import { DOC_PAGES, resolveDocHref } from './docs'

const sources = import.meta.glob<string>('../content/*.md', { query: '?raw', import: 'default', eager: true })

const documents = DOC_PAGES.map((page) => {
  const source = sources[`../content/${page.name}.md`]
  if (source === undefined) throw new Error(`Missing synced documentation: ${page.name}.md`)
  return { ...page, source, path: page.route }
})

export function textResponse(body: string, markdown = false, status = 200) {
  return new Response(body, {
    status,
    headers: {
      'Content-Type': `${markdown ? 'text/markdown' : 'text/plain'}; charset=utf-8`,
      'Cache-Control': 'public, max-age=3600',
      'X-Content-Type-Options': 'nosniff',
    },
  })
}

function documentSource(doc: (typeof documents)[number], combined = false): string {
  let fence = ''
  return doc.source.split('\n').map((line) => {
    const marker = line.match(/^\s*(`{3,}|~{3,})/)?.[1]
    if (marker) {
      fence = fence ? (marker[0] === fence[0] && marker.length >= fence.length ? '' : fence) : marker
      return line
    }
    if (fence) return line
    const linked = line.replace(/(\]\()(\.\.?\/[^)\s]+|\/(?!\/)[^)\s]+|#[^)\s]+)(\))/g, (_, start: string, href: string, end: string) => {
      if (href.startsWith('#')) return `${start}${combined ? `${SITE_URL}${doc.path}` : ''}${href}${end}`
      const resolved = new URL(resolveDocHref(href), SITE_URL)
      const target = DOC_PAGES.find((page) => page.route === resolved.pathname)
      return `${start}${target ? `${SITE_URL}/docs/${target.name}.md${resolved.hash}` : resolved.href}${end}`
    })
    return combined ? linked.replace(/^(#{1,5}) /, '#$1 ') : linked
  }).join('\n')
}

export function documentMarkdown(name: string) {
  const doc = documents.find((d) => d.name === name)
  if (!doc) return textResponse('Document not found\n', false, 404)
  return textResponse(`${documentSource(doc)}\n\nCanonical URL: ${SITE_URL}${doc.path}\n`, true)
}

export function ruleMarkdown(rule: Rule): string {
  const code = (source: string, lang = 'dart') => `\n\`\`\`${lang}\n${source}\n\`\`\`\n`
  const { configSnippet, suppression } = ruleGuidance(rule)
  return [
    `# ${rule.name}`,
    `Canonical URL: ${SITE_URL}/linter/rules/${rule.name}`,
    `Group: ${rule.group}\nOfficial Dart lint name (not a falcon configuration key): ${rule.source.kind === 'Lints' ? rule.source.upstreamId ?? 'none' : 'none'}\nDefault: ${rule.recommended ? 'on (warning)' : 'off'}\nDomains: ${rule.domains.join(', ') || 'general Dart'}\nSource: ${rule.source.label}`,
    rule.docs.startsWith(rule.description) ? rule.docs : `${rule.description}\n\n${rule.docs}`,
    ...(rule.examples.bad ? ['## Invalid example', code(rule.examples.bad)] : []),
    ...(rule.examples.crossFile ?? []).flatMap((f) => [`## Fixture: ${f.path}`, code(f.content, f.path.endsWith('.yaml') ? 'yaml' : 'dart')]),
    ...(rule.examples.good ? ['## Valid example', code(rule.examples.good)] : []),
    '## Configuration',
    code(configSnippet, 'json'),
    '## Suppression',
    suppression.text,
    code(suppression.code, suppression.lang),
    `See [suppression documentation](${SITE_URL}/docs/suppressions.md).`,
  ].join('\n\n') + '\n'
}

const link = (label: string, path: string, note?: string) => `- [${label}](${SITE_URL}${path})${note ? `: ${note}` : ''}`
const section = (heading: string, ...lines: string[]) => `## ${heading}\n\n${lines.join('\n')}`

export function llmsIndex(): string {
  return [
    '# Dart Falcon',
    `> falcon is a free, MIT-licensed Dart linter and Flutter linter written in Rust: ${rules.length} lint rules (including the official Dart lints under their analyzer names), cross-file checks, one falcon.json, and an LSP server for editors. It complements dart analyze, which still does type checking; SDK-dependent rules need the Dart SDK and resolved packages.`,
    section(
      'Quick start',
      '```sh',
      'dart pub global activate falcon   # Linux and macOS',
      'falcon check .                    # lint the current project',
      'falcon migrate                    # convert analysis_options.yaml lint config to falcon.json',
      '```',
    ),
    section(
      'Documentation',
      ...documents.map((doc) => link(doc.title, doc.path, `Markdown at ${SITE_URL}/docs/${doc.name}.md`)),
      link('Configuration reference', '/reference/configuration', 'every falcon.json key and default'),
      link('CLI reference', '/reference/cli', 'commands and flags'),
    ),
    section(
      'Rule catalog',
      link(`All ${rules.length} Dart lint rules`, '/linter/rules', 'searchable; append .md to any rule URL for Markdown, e.g. /linter/rules/avoid-print.md'),
      ...GROUPS.map((g) => link(`${g.label} rules`, `/linter/groups/${g.id}`, g.blurb)),
    ),
    section('Framework domains', ...domains.map((d) => link(`${d.name} rules`, `/linter/domains/${d.name}`, `${d.rules.length} rules`))),
    section(
      'Comparisons and evidence',
      link('falcon vs dart analyze, DCM and custom_lint', '/compare'),
      link('Benchmarks and methodology', '/benchmarks'),
      `- [Source and releases](${REPO_URL})`,
    ),
    section('Optional', link('All documentation and a compact rule catalog in one file', '/llms-full.txt')),
  ].join('\n\n') + '\n'
}

export function llmsFull(): string {
  return [
    '# falcon documentation and rule catalog',
    '> falcon complements dart analyze; it does not replace Dart type checking. This file contains synced documentation and a compact catalog. Follow each rule URL or its .md variant for full examples.',
    ...documents.map((d) => `${documentSource(d, true)}\n\nCanonical URL: ${SITE_URL}${d.path}`),
    '## Compact rule catalog',
    'Fields: id | official Dart lint name (not a falcon configuration key) | group | default | domains | description | URL',
    ...rules.map((r) => [
      r.name,
      r.source.kind === 'Lints' ? r.source.upstreamId ?? '-' : '-',
      r.group,
      r.recommended ? 'on' : 'off',
      r.domains.join(', ') || '-',
      r.description.replace(/\s+/g, ' ').replace(/\|/g, '\\|'),
      `${SITE_URL}/linter/rules/${r.name}`,
    ].join(' | ')),
  ].join('\n\n') + '\n'
}
