import rulesJson from '../data/rules.json'
import domainsJson from '../data/domains.json'

export const REPO_URL = 'https://github.com/JacobDevelops/dart_falcon'

export type RuleGroup =
  | 'a11y'
  | 'complexity'
  | 'correctness'
  | 'performance'
  | 'security'
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
    id: 'a11y',
    label: 'Accessibility',
    blurb: 'Patterns that make applications harder to use with assistive technology.',
  },
  {
    id: 'complexity',
    label: 'Complexity',
    blurb: 'Needlessly complex code that is harder to read and maintain.',
  },
  {
    id: 'correctness',
    label: 'Correctness',
    blurb: 'Code that is outright wrong or will not behave as intended.',
  },
  {
    id: 'performance',
    label: 'Performance',
    blurb: 'Patterns with avoidable runtime or build cost.',
  },
  {
    id: 'security',
    label: 'Security',
    blurb: 'Patterns that expose applications or their users to security risks.',
  },
  {
    id: 'style',
    label: 'Style',
    blurb: 'Idiomatic, consistent Dart and Flutter style.',
  },
  {
    id: 'suspicious',
    label: 'Suspicious',
    blurb: 'Patterns that are probably a mistake or a latent bug.',
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

export const groupedRules: GroupedRules[] = GROUPS.map((g) => ({
  ...g,
  rules: rules
    .filter((r) => r.group === g.id && !r.crossFile)
    .sort((a, b) => a.name.localeCompare(b.name)),
}))

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
