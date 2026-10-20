// Types and static metadata only: the 2 MB rule catalog stays on the server
// (see rules.data.ts) so it never ships in the client bundle.
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
  crossFile?: CrossFileFixture[] | null
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

/** The slice of a rule that lists and links need. */
export interface RuleSummary {
  name: string
  group: RuleGroup
  description: string
  recommended: boolean
  crossFile: boolean
  domains: string[]
  upstreamId: string | null
}

export interface GroupMeta {
  id: RuleGroup
  label: string
  blurb: string
}

/** Rule-catalog counts derived from rules.json/domains.json. */
export interface Coverage {
  total: number
  recommended: number
  crossFile: number
  /** Rules that map one-to-one to an official Dart lint. */
  mappedToOfficial: number
  groups: (GroupMeta & { count: number; recommended: number })[]
  domains: { name: string; count: number }[]
  sources: { label: string; count: number }[]
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

export function groupLabel(id: string): string {
  return GROUPS.find((g) => g.id === id)?.label ?? id
}

export function summarize(rule: Rule): RuleSummary {
  return {
    name: rule.name,
    group: rule.group,
    description: rule.description,
    recommended: rule.recommended,
    crossFile: rule.crossFile,
    domains: rule.domains,
    upstreamId: rule.source.upstreamId,
  }
}
