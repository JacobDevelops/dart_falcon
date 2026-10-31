import { execFileSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import rules from '../src/data/rules.json'
import benchmarks from '../src/data/benchmarks.json'
import { DOC_PAGES } from '../src/lib/docs'

const websiteRoot = join(dirname(fileURLToPath(import.meta.url)), '..')
const repoRoot = join(websiteRoot, '..')
const outDir = join(websiteRoot, 'src', 'generated')
const lastmod: Record<string, string> = {}

function historyDates(): Map<string, string> | null {
  const git = (args: string[]) => execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  try {
    if (git(['rev-parse', '--is-shallow-repository']).trim() !== 'false') {
      throw new Error('shallow Git history')
    }
    const history = git([
      'log', '--format=%cI', '--name-only', '--no-renames', '--',
      'docs', 'website/src', 'crates/falcon_rules/src', 'crates/falcon_rules/tests/corpus',
    ])
    const dates = new Map<string, string>()
    let date = ''
    for (const line of history.split('\n')) {
      if (/^\d{4}-\d{2}-\d{2}T/.test(line)) {
        date = new Date(line).toISOString()
      } else if (line && date && !dates.has(line)) {
        dates.set(line, date)
      }
    }
    return dates
  } catch (error) {
    console.warn('warning: sitemap lastmod omitted; full Git history is required:', error instanceof Error ? error.message : error)
    return null
  }
}

function latest(path: string, date: string | undefined) {
  if (date && (!lastmod[path] || date > lastmod[path])) lastmod[path] = date
}

const dates = historyDates()
if (dates) {
  const fixtureDates = new Map<string, string>()
  const corpus = 'crates/falcon_rules/tests/corpus/'
  for (const [file, date] of dates) {
    if (!file.startsWith(corpus)) continue
    const directory = corpus + file.slice(corpus.length).split('/')[0]
    if (date > (fixtureDates.get(directory) ?? '')) fixtureDates.set(directory, date)
  }

  for (const rule of rules) {
    const path = `/linter/rules/${rule.name}`
    latest(path, dates.get(rule.sourcePath))
    latest(path, fixtureDates.get(rule.fixturePath))
    const date = lastmod[path]
    latest('/linter/rules', date)
    latest(`/linter/groups/${rule.group}`, date)
    for (const domain of rule.domains) {
      latest(`/linter/domains/${domain}`, date)
      latest('/linter/domains', date)
    }
  }
  for (const page of DOC_PAGES) latest(page.route, dates.get(`docs/${page.name}.md`))
  for (const [path, route, data] of [
    ['/', 'index.tsx'],
    ['/compare', 'compare.tsx'],
    ['/reference/cli', 'reference/cli.tsx', 'cli.json'],
    ['/reference/configuration', 'reference/configuration.tsx', 'config-reference.json'],
  ]) {
    latest(path, dates.get(`website/src/routes/${route}`))
    if (data) latest(path, dates.get(`website/src/data/${data}`))
  }
  latest('/benchmarks', dates.get('website/src/data/benchmarks.json'))
  latest('/benchmarks', new Date(benchmarks.generatedAt).toISOString())
}

mkdirSync(outDir, { recursive: true })
writeFileSync(join(outDir, 'lastmod.json'), JSON.stringify(lastmod, null, 2) + '\n')
console.log(`generated lastmod for ${Object.keys(lastmod).length} URLs`)
