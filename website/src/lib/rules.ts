import rulesJson from '../data/rules.json'
import domainsJson from '../data/domains.json'

export type RuleGroup =
  | 'complexity'
  | 'correctness'
  | 'performance'
  | 'style'
  | 'suspicious'

export interface RuleSource {
  kind: string
  label: string
  upstreamId: string | null
  upstreamUrl: string | null
}

export interface RuleDiagnostic {
  file: string | null
  line: number
  column: number
  message: string
}

export interface CrossFileFixture {
  path: string
  content: string
}

export interface RuleExamples {
  bad: string | null
  good: string | null
  crossFile?: CrossFileFixture[]
}

export interface Rule {
  name: string
  group: RuleGroup
  domains: string[]
  recommended: boolean
  crossFile: boolean
  source: RuleSource
  description: string
  docs: string
  examples: RuleExamples
  diagnostics: RuleDiagnostic[]
}

export interface Domain {
  name: string
  rules: string[]
}

// The JSON is generated and committed by `cargo xtask docgen`; treat it as a
// trusted boundary and assert the shape once here.
export const rules = rulesJson as unknown as Rule[]
export const domains = domainsJson as unknown as Domain[]

export interface GroupMeta {
  id: RuleGroup
  label: string
  blurb: string
}

export const GROUPS: GroupMeta[] = [
  {
    id: 'correctness',
    label: 'Correctness',
    blurb: 'Code that is outright wrong or will not behave as intended.',
  },
  {
    id: 'suspicious',
    label: 'Suspicious',
    blurb: 'Patterns that are probably a mistake or a latent bug.',
  },
  {
    id: 'complexity',
    label: 'Complexity',
    blurb: 'Needlessly complex code that is harder to read and maintain.',
  },
  {
    id: 'performance',
    label: 'Performance',
    blurb: 'Patterns with avoidable runtime or build cost.',
  },
  {
    id: 'style',
    label: 'Style',
    blurb: 'Idiomatic, consistent Dart and Flutter style.',
  },
]

const byName = new Map(rules.map((r) => [r.name, r]))

export function getRule(name: string): Rule | undefined {
  return byName.get(name)
}

export function groupLabel(id: string): string {
  return GROUPS.find((g) => g.id === id)?.label ?? id
}

export interface GroupedRules extends GroupMeta {
  rules: Rule[]
}

/** Single-file rules grouped and ordered for the index page. */
export function groupedRules(): GroupedRules[] {
  return GROUPS.map((g) => ({
    ...g,
    rules: rules
      .filter((r) => r.group === g.id && !r.crossFile)
      .sort((a, b) => a.name.localeCompare(b.name)),
  })).filter((g) => g.rules.length > 0)
}

export const crossFileRules: Rule[] = rules
  .filter((r) => r.crossFile)
  .sort((a, b) => a.name.localeCompare(b.name))

export const recommendedCount = rules.filter((r) => r.recommended).length

/** Picks a real rule with both a bad example and diagnostics for the hero. */
export function heroRule(): Rule | undefined {
  return (
    getRule('avoid-function-literals-in-foreach-calls') ??
    rules.find((r) => r.examples.bad && r.diagnostics.length > 0)
  )
}
