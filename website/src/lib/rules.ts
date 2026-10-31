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
  sourcePath: string
  fixturePath: string
  description: string
  docs: string
  examples: RuleExamples
  diagnostics: RuleDiagnostic[]
}

export type RuleDetail = Omit<Rule, 'sourcePath' | 'fixturePath'>

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
  sourceKind: string
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

export const DOMAIN_BLURBS: Record<string, string> = {
  flutter: 'Widget construction, lifecycle, and Flutter framework APIs.',
  test: 'Dart and Flutter test suites: assertions, matchers, and test structure.',
  bloc: 'Blocs and Cubits from package:bloc: events, states, and emit discipline.',
  riverpod: 'Providers and refs from package:riverpod and flutter_riverpod.',
  provider: 'package:provider: reading, watching, and disposing providers.',
  flutter_hooks: 'Hook rules for package:flutter_hooks.',
  equatable: 'Value equality with package:equatable.',
}

export function domainLabel(name: string): string {
  if (name === 'flutter_hooks') return 'Flutter Hooks'
  return name.charAt(0).toUpperCase() + name.slice(1)
}

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
    sourceKind: rule.source.kind,
  }
}

export function ruleGuidance(rule: RuleDetail) {
  const section = rule.crossFile ? 'cross-file' : 'linter'
  const config = (severity: 'error' | 'off') =>
    JSON.stringify({ [section]: { rules: { [rule.group]: { [rule.name]: severity } } } }, null, 2)
  const pubspec = rule.diagnostics.length > 0 &&
    rule.diagnostics.every(({ file }) => /(^|\/)pubspec\.yaml$/.test(file ?? ''))
  return {
    configSnippet: config('error'),
    suppression: pubspec ? {
      text: 'These diagnostics are reported on pubspec.yaml and cannot be suppressed with source comments. Disable this rule in falcon.json when necessary:',
      code: config('off'),
      lang: 'json',
      filename: 'falcon.json',
    } : {
      text: rule.name === 'unused-files'
        ? 'Use a file-wide suppression in the reported Dart file. A reason is required:'
        : 'Place this comment above the affected line. A reason is required:',
      code: `// falcon-ignore${rule.name === 'unused-files' ? '-all' : ''} ${rule.crossFile ? 'cross-file' : 'lint'}/${rule.group}/${rule.name}: explain why this is intentional`,
      lang: 'dart',
      filename: 'example.dart',
    },
  }
}
