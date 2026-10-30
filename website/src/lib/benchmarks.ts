// Types and formatting only. The raw benchmarks.json (with every sample and
// lockfile snapshot) is read on the server by `getBenchmarks` in server.ts.

export interface BenchTool {
  id: string
  name: string
  version: string
}

export interface BenchCorpus {
  id: string
  name: string
  url: string
  rev: string
  files: number
  lines: number
  /** Source subset used by the DCL-family scenarios; absent in older reports. */
  dclFiles?: number
  dclLines?: number
  notMeasured?: { tool: string; reason: string }[]
  /** Plugin directories measured when only part of the repository is in scope. */
  scope: string[]
}

export interface BenchScenario {
  id: string
  title: string
  description: string
  /** Tool used as the speedup baseline; older snapshots compare with dart analyze. */
  comparator?: string
}

export interface BenchResult {
  corpus: string
  scenario: string
  tool: string
  meanMs: number
  stddevMs: number
  minMs: number
  maxMs: number
  runs: number
  /** Total diagnostics, including the analyzer's type-checking output. */
  diagnostics?: number
  lintDiagnostics?: number
  nonLintDiagnostics?: number
  errorDiagnostics?: number
  /** Peak resident memory of one run, in MB. */
  peakRssMb?: number
}

export interface Benchmarks {
  generatedAt: string
  machine: { cpu: string; cores: number; memoryGb: number; os: string }
  tools: BenchTool[]
  corpora: BenchCorpus[]
  scenarios: BenchScenario[]
  results: BenchResult[]
}

export interface BenchRow extends BenchResult {
  name: string
  version: string
}

export interface BenchRun {
  corpus: BenchCorpus
  scenario: BenchScenario
  /** Fastest first. */
  rows: BenchRow[]
}

/** What pages receive: the setup facts plus every corpus × scenario with results. */
export interface BenchView {
  generatedAt: string
  machine: Benchmarks['machine']
  tools: BenchTool[]
  corpora: BenchCorpus[]
  runs: BenchRun[]
}


/** falcon's speedup over the scenario's comparator, never an unrelated tool. */
export function speedup(run: BenchRun) {
  const falcon = run.rows.find((r) => r.tool === 'falcon')
  const versus =
    run.rows.find((r) => r.tool === (run.scenario.comparator ?? 'dart-analyze')) ??
    falcon ??
    run.rows[0]
  return { factor: falcon ? versus.meanMs / falcon.meanMs : 1, versus }
}

export function formatMs(ms: number): string {
  if (ms >= 10_000) return `${(ms / 1000).toFixed(1)} s`
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)} s`
  return `${Math.round(ms)} ms`
}

export function formatSpeedup(x: number): string {
  return x >= 10 ? `${Math.round(x)}×` : `${x.toFixed(1)}×`
}

/** "6.0× faster than dart analyze" or, when falcon lost, "1.7× slower than …". */
export function compareText(run: BenchRun): { ratio: string; text: string } {
  const { factor, versus } = speedup(run)
  return factor >= 1
    ? { ratio: formatSpeedup(factor), text: `faster than ${versus.name}` }
    : { ratio: formatSpeedup(1 / factor), text: `slower than ${versus.name}` }
}

/** Corpus label that names a partial checkout, e.g. "3 flutter/packages plugin families". */
export function corpusLabel(corpus: BenchCorpus): string {
  const n = corpus.scope.length
  return n > 0 ? `${['one', 'two', 'three', 'four', 'five'][n - 1] ?? n} ${corpus.name} plugin families` : corpus.name
}
/** Explicit omissions apply to the DCL family, not the official-lint scenarios. */
export function unmeasuredTools(run: BenchRun) {
  return run.scenario.comparator === 'dcl'
    ? (run.corpus.notMeasured ?? []).filter((entry) => !run.rows.some((row) => row.tool === entry.tool))
    : []
}


export interface ScenarioSummary {
  scenario: BenchScenario
  /** Runs comparing falcon with the scenario's comparator, one per corpus. */
  runs: BenchRun[]
  /** Lowest and highest falcon speedup across corpora. */
  min: number
  max: number
  slower: BenchRun[]
}

/** One summary per scenario across every corpus, in scenario order (cold first). */
export function scenarioSummaries(view: BenchView): ScenarioSummary[] {
  const scenarios = [...new Map(view.runs.map((r) => [r.scenario.id, r.scenario])).values()]
  return scenarios.flatMap((scenario) => {
    const runs = view.runs.filter(
      (r) => r.scenario.id === scenario.id && r.rows.some((x) => x.tool === 'falcon') && speedup(r).versus.tool !== 'falcon',
    )
    if (runs.length === 0) return []
    const factors = runs.map((r) => speedup(r).factor)
    return [{ scenario, runs, min: Math.min(...factors), max: Math.max(...factors), slower: runs.filter((r) => speedup(r).factor < 1) }]
  })
}

/** "3.9–6.0×", or one ratio when both ends agree. */
export function formatRange(min: number, max: number): string {
  return formatSpeedup(min) === formatSpeedup(max)
    ? formatSpeedup(max)
    : `${formatSpeedup(min).slice(0, -1)}–${formatSpeedup(max)}`
}

const list = (names: string[]) =>
  names.length < 2 ? names.join('') : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`

/** "falcon 3.9–6.0× faster than dart analyze on all 4 corpora." */
export function summaryBody(s: ScenarioSummary): string {
  const versus = speedup(s.runs[0]).versus.name
  if (s.slower.length === 0) {
    return `falcon ${formatRange(s.min, s.max)} faster than ${versus} on ${s.runs.length === 1 ? corpusLabel(s.runs[0].corpus) : `all ${s.runs.length} corpora`}.`
  }
  const faster = s.runs.filter((r) => !s.slower.includes(r))
  return `${versus} was faster on ${list(s.slower.map((r) => corpusLabel(r.corpus)))}${
    faster.length > 0
      ? `; falcon was faster on ${list(faster.map((r) => `${corpusLabel(r.corpus)} (${formatSpeedup(speedup(r).factor)})`))}`
      : ''
  }.`
}

export function summaryText(s: ScenarioSummary): string {
  return `${s.scenario.title}: ${summaryBody(s)}`
}

/** Every scenario's summary as one paragraph; the cold scenario leads. */
export function headlineSummary(view: BenchView): string {
  return scenarioSummaries(view).map(summaryText).join(' ')
}
