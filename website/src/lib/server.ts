import { createServerFn } from '@tanstack/react-start'
import { notFound, redirect } from '@tanstack/react-router'
import benchmarksJson from '../data/benchmarks.json'
import type { BenchCorpus, BenchResult, Benchmarks, BenchView } from './benchmarks'
import { coverage, domains, getRule, resolveRule, rules, summaries } from './rules.data'

// Route loaders call these; the Start compiler replaces each handler with an
// RPC stub in the client build, which keeps rules.json out of the bundle.

interface RawCorpus extends Omit<BenchCorpus, 'scope'> {
  includedDirectories: string[]
}

// Written by the benchmark harness from real runs; typed so a renamed field fails the type check.
const raw: Omit<Benchmarks, 'corpora'> & { corpora: RawCorpus[] } = benchmarksJson

const corpusView = ({ id, name, url, rev, files, lines, dclFiles, dclLines, notMeasured, includedDirectories }: RawCorpus): BenchCorpus => ({
  id,
  name,
  url,
  rev,
  files,
  lines,
  dclFiles,
  dclLines,
  notMeasured,
  scope: includedDirectories,
})

// Only contract fields cross the wire; the JSON also carries raw samples.
const pick = (r: BenchResult): BenchResult => ({
  corpus: r.corpus,
  scenario: r.scenario,
  tool: r.tool,
  meanMs: r.meanMs,
  stddevMs: r.stddevMs,
  minMs: r.minMs,
  maxMs: r.maxMs,
  runs: r.runs,
  diagnostics: r.diagnostics,
  lintDiagnostics: r.lintDiagnostics,
  nonLintDiagnostics: r.nonLintDiagnostics,
  errorDiagnostics: r.errorDiagnostics,
  peakRssMb: r.peakRssMb,
})

const benchView: BenchView = {
  generatedAt: raw.generatedAt,
  machine: raw.machine,
  tools: raw.tools.map(({ id, name, version }) => ({ id, name, version })),
  corpora: raw.corpora.map(corpusView),
  runs: raw.corpora.flatMap((corpus) =>
    raw.scenarios.flatMap((scenario) => {
      const rows = raw.results
        .filter((r) => r.corpus === corpus.id && r.scenario === scenario.id)
        .map((r) => {
          const tool = raw.tools.find((t) => t.id === r.tool)
          return { ...pick(r), name: tool?.name ?? r.tool, version: tool?.version ?? '' }
        })
        .sort((a, b) => a.meanMs - b.meanMs)
      return rows.length > 0
        ? [{ corpus: corpusView(corpus), scenario: { id: scenario.id, title: scenario.title, description: scenario.description, comparator: scenario.comparator }, rows }]
        : []
    }),
  ),
}

export const getBenchmarks = createServerFn({ method: 'GET' }).handler(() => benchView)

export const getCompareData = createServerFn({ method: 'GET' }).handler(() => ({
  coverage: coverage(),
  bench: benchView,
}))

export const getHomeData = createServerFn({ method: 'GET' }).handler(() => {
  const hero =
    getRule('avoid-function-literals-in-foreach-calls') ??
    rules.find((r) => r.examples.bad && r.diagnostics.length > 0)
  return {
    coverage: coverage(),
    bench: benchView,
    hero: hero
      ? {
          name: hero.name,
          description: hero.description,
          bad: hero.examples.bad ?? '',
          diagnostics: hero.diagnostics,
        }
      : null,
  }
})

export const getCatalogCoverage = createServerFn({ method: 'GET' }).handler(() => coverage())

export const getRulesIndex = createServerFn({ method: 'GET' }).handler(() => ({
  rules: summaries,
  coverage: coverage(),
}))

export const getDomains = createServerFn({ method: 'GET' }).handler(() =>
  domains.map((d) => ({
    name: d.name,
    rules: summaries.filter((s) => d.rules.includes(s.name)),
  })),
)

export const getRuleDetail = createServerFn({ method: 'GET' })
  .validator((name: string) => name)
  .handler(({ data: name }) => {
    const rule = resolveRule(name)
    if (!rule) throw notFound()
    if (rule.name !== name) {
      throw redirect({ to: '/linter/rules/$rule', params: { rule: rule.name }, statusCode: 301 })
    }
    const index = rules.indexOf(rule)
    const group = summaries.filter((s) => s.group === rule.group)
    const at = group.findIndex((s) => s.name === rule.name)
    const { sourcePath, fixturePath, ...detail } = rule
    return {
      rule: detail,
      prev: summaries[index - 1] ?? null,
      next: summaries[index + 1] ?? null,
      related: [...group.slice(at + 1), ...group.slice(0, at)].slice(0, 8),
    }
  })
