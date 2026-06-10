# Deep Interview Spec: falcon — Rust-Based Dart Linter

## Metadata
- Interview ID: falcon-2026-06-09
- Rounds: 7
- Final Ambiguity Score: 17.8%
- Type: greenfield
- Generated: 2026-06-09
- Threshold: 0.2
- Threshold Source: default
- Initial Context Summarized: no
- Status: PASSED

## Clarity Breakdown
| Dimension | Score | Weight | Weighted |
|-----------|-------|--------|----------|
| Goal Clarity | 0.90 | 0.40 | 0.36 |
| Constraint Clarity | 0.82 | 0.30 | 0.246 |
| Success Criteria | 0.72 | 0.30 | 0.216 |
| **Total Clarity** | | | **0.822** |
| **Ambiguity** | | | **17.8%** |

## Topology
| Component | Status | Description | Coverage / Deferral Note |
|-----------|--------|-------------|--------------------------|
| Dart Parser/AST | active | Hand-rolled Rust parser for full Dart 3.x grammar | Full Dart 3.x: records, patterns, sealed classes, core language |
| Rule Engine | active | Biome-style visitor/rule runner, diagnostic collection, parallel execution | Rayon-based parallelism for <1s full-project target; LSP server integration |
| Rule Port | active | Translate dart_code_linter + pyramid_lint rules to Rust | Phase 1: ~130 rules from dart_code_linter (^3.2.1) + pyramid_lint (^2.4.0) only |
| CLI & Config | active | falcon check/lint --fix CLI + biome.json-style config file | Config file format to resemble biome.json; covers rule enable/disable, project exclude patterns |
| Nix Flake | active | Package falcon as Nix flake, add as input to jfit's flake.nix | Exposes binary + devShell overlay; added to `/home/jacob/Documents/Developer/jfit/flake.nix` |

## Goal
Build `falcon` — a Rust-based Dart linter that replaces the `custom_lint` plugin layer (Phase 1) and eventually the full `dart analyze` pipeline (Phase 2). Phase 1 ships with a hand-rolled Dart 3.x parser, a Biome-inspired parallel rule engine, a full LSP server for editor integration, a biome.json-style config file, and ports all enabled rules from `dart_code_linter` and `pyramid_lint` as used in the jfit mobile project. It is packaged as a Nix flake and installed as an input to jfit's existing `flake.nix`.

## Constraints
- **Parser**: Hand-rolled recursive descent parser in Rust — no tree-sitter, no Dart SDK dependency
- **Grammar scope**: Full Dart 3.x including records, patterns, sealed classes, and all syntax used in jfit
- **Phase 1 rules**: Only rules enabled in `/home/jacob/Documents/Developer/jfit/apps/mobile/analysis_options.yaml` from `dart_code_linter` and `pyramid_lint` — not the full rule catalogues of those packages
- **very_good_analysis**: Out of scope for Phase 1 (it is a YAML config, not custom rules); Phase 2 covers built-in Dart lint reimplementation
- **LSP**: Full Language Server Protocol server required from Phase 1 (not deferred)
- **Performance**: Must complete full jfit mobile project lint in under 1 second — requires Rayon-based parallelism
- **Nix**: falcon is a Nix workspace (Cargo workspace + Nix flake); binary exposed as a flake output and added as an input to `/home/jacob/Documents/Developer/jfit/flake.nix`
- **Directory**: Current repo at `/home/jacob/Documents/Developer/falcon` must be renamed to `falcon`
- **Config format**: Resembles `biome.json` — JSON, top-level keys per subsystem, rule enable/disable per rule name
- **Architecture reference**: Biome repo at `/home/jacob/Documents/Developer/biome` is the primary structural reference; replicate crate layout and CLI/config patterns

## Non-Goals
- **Phase 1**: Re-implementing Dart built-in lints (the `very_good_analysis` rule set)
- **Phase 1**: Full replacement of `dart analyze` — `dart analyze` continues to run for type-checking and built-in lints
- **Phase 1**: Supporting Dart 2.x-only syntax that jfit does not use
- **Windows support** in Phase 1 (Linux + macOS via Nix)
- **Auto-fix for all rules** in Phase 1 — only rules that have a safe mechanical fix need `--fix` support
- **Publishing to crates.io** or pkg registries in Phase 1

## Acceptance Criteria
- [ ] Repository renamed from `falcon` to `falcon` with updated Cargo.toml `[package] name = "falcon"`
- [ ] Cargo workspace compiles clean (`cargo build --release`) with Rust stable
- [ ] Dart 3.x parser round-trips all `.dart` files in `/home/jacob/Documents/Developer/jfit/apps/mobile/lib` without parse errors
- [ ] All `dart_code_linter` rules enabled in jfit's `analysis_options.yaml` are implemented and produce correct diagnostics on a representative jfit file
- [ ] All `pyramid_lint` rules enabled in jfit's `analysis_options.yaml` are implemented and produce correct diagnostics on a representative jfit file
- [ ] `falcon check .` completes in under 1 second on the jfit mobile project
- [ ] LSP server starts and VS Code (via generic LSP extension) shows falcon diagnostics for a `.dart` file
- [ ] `falcon.json` config at project root controls which rules are enabled/disabled; jfit's config file is committed
- [ ] `nix build` in the falcon repo produces the `falcon` binary
- [ ] `jfit/flake.nix` updated to include `falcon` as an input and expose the binary in `devShell`
- [ ] `nix develop` in jfit makes `falcon` available on PATH

## Assumptions Exposed & Resolved
| Assumption | Challenge | Resolution |
|------------|-----------|------------|
| "Port very_good_analysis source code" | It has no Rust code — it's a YAML config for built-in Dart lints | Phase 1 scope is dart_code_linter + pyramid_lint only; built-ins are Phase 2 |
| Need a tree-sitter or existing parser | User chose hand-rolled from scratch | Full Dart 3.x recursive descent parser in Rust |
| jfit has no flake.nix | Checked repo — flake.nix exists at `/home/jacob/Documents/Developer/jfit/flake.nix` | Add falcon as an `inputs` entry to the existing flake |
| IDE integration could be deferred | Challenged: Dart Analysis Server already handles editor squiggles | User confirmed: full LSP server required from Phase 1 |
| "Fast enough" is vague | Asked for concrete target | Under 1 second on full project; implies Rayon parallelism required |
| Phase 1 = everything | Challenged rule scope | Phase 1 = custom_lint replacement (dart_code_linter + pyramid_lint); Phase 2 = full dart analyze replacement |

## Technical Context

### Source projects to reference
- **Biome**: `/home/jacob/Documents/Developer/biome` — 94-crate Cargo workspace; `biome_analyze`, `biome_cli`, `biome_configuration`, `biome_diagnostics`, `biome_formatter` are key reference crates
- **jfit mobile**: `/home/jacob/Documents/Developer/jfit/apps/mobile` — Flutter/Dart project with `analysis_options.yaml` defining all lint rules to port
- **jfit flake**: `/home/jacob/Documents/Developer/jfit/flake.nix` — existing Nix flake to receive falcon as an input

### Current linters in jfit/apps/mobile
| Package | Version | Role | Port target |
|---------|---------|------|------------|
| `custom_lint` | ^0.8.1 | Plugin runner framework | Replaced entirely by falcon |
| `dart_code_linter` | ^3.2.1 | ~100 Dart/Flutter rules + metrics | Phase 1: port enabled rules from analysis_options.yaml |
| `pyramid_lint` | ^2.4.0 | ~30 Dart/Flutter rules | Phase 1: port enabled rules from analysis_options.yaml |
| `very_good_analysis` | ^10.2.0 | Curated built-in lint YAML | Phase 2: reimplement curated built-in lints in Rust |

### Rules to port (Phase 1) — from analysis_options.yaml
**dart_code_linter rules enabled in jfit:**
`binary-expression-operand-order`, `avoid-dynamic`, `avoid-passing-async-when-sync-expected`, `avoid-redundant-async`, `avoid-throw-in-catch-block`, `avoid-unnecessary-type-assertions`, `avoid-unnecessary-type-casts`, `avoid-unrelated-type-assertions`, `avoid-unused-parameters`, `avoid-nested-conditional-expressions`, `avoid-non-null-assertion`, `avoid-late-keyword`, `avoid-global-state`, `prefer-async-await`, `prefer-correct-identifier-length`, `prefer-correct-type-name`, `prefer-conditional-expressions`, `prefer-first`, `prefer-immediate-return`, `prefer-iterable-of`, `prefer-last`, `prefer-moving-to-variable`, `prefer-trailing-comma`, `double-literal-format`, `format-comment`, `member-ordering`, `newline-before-return`, `no-boolean-literal-compare`, `no-empty-block`, `no-equal-arguments`, `no-equal-then-else`, `no-magic-number`, `no-object-declaration`, `use-design-system-item`

**pyramid_lint rules enabled in jfit:**
`avoid-single-child-column-or-row`, `prefer-async-callback`, `proper-controller-dispose`, `proper-edge-insets-constructor`, `proper-expanded-and-flexible`, `proper-from-environment`, `proper-super-dispose`, `proper-super-init-state`, `avoid-abbreviations-in-doc-comments`, `avoid-dynamic`, `avoid-inverted-boolean-expressions`, `avoid-nested-if`, `avoid-positional-fields-in-records`, `avoid-redundant-pattern-field-names`, `avoid-unused-parameters`, `boolean-prefixes`, `max-lines-for-file` (max: 500), `max-lines-for-function` (max: 100), `no-self-comparisons`, `prefer-declaring-const-constructors`, `prefer-immediate-return`, `prefer-iterable-any`, `prefer-iterable-every`, `prefer-iterable-first`, `prefer-iterable-last`, `prefer-underscore-for-unused-callback-parameters`

### Proposed crate structure (Biome-inspired)
```
falcon/
├── Cargo.toml              # workspace
├── flake.nix               # falcon Nix flake
├── falcon.json             # default config (checked into jfit later)
├── crates/
│   ├── falcon_dart_parser/ # hand-rolled Dart 3.x recursive descent parser
│   ├── falcon_syntax/      # AST node types, SyntaxKind enum
│   ├── falcon_analyze/     # rule trait, visitor, query infrastructure
│   ├── falcon_rules/       # all lint rule implementations
│   ├── falcon_diagnostics/ # Diagnostic, Severity, Span types
│   ├── falcon_lsp/         # LSP server (tower-lsp or lsp-server crate)
│   ├── falcon_config/      # falcon.json deserialization (serde_json)
│   ├── falcon_cli/         # clap-based CLI: check, lint, lsp
│   └── falcon/             # binary entrypoint
└── xtask/
    └── codegen/            # code generation for rule boilerplate (biome-style)
```

## Ontology (Key Entities)
| Entity | Type | Fields | Relationships |
|--------|------|--------|---------------|
| DartParser | core domain | grammar_version, source_text | produces SyntaxTree |
| SyntaxTree | core domain | root_node, source_id | consumed by RuleEngine |
| LintRule | core domain | rule_id, severity, category, has_fix | registered in RuleEngine |
| RuleEngine | core domain | rules: Vec<LintRule>, parallelism | traverses SyntaxTree, emits Diagnostic |
| Diagnostic | core domain | rule_id, span, severity, message, fix | reported by CLI, LSP |
| LSPServer | supporting | transport, capabilities | wraps RuleEngine for editor protocol |
| FalconConfig | supporting | rules_config, exclude_patterns | loaded from falcon.json |
| NixFlake | external system | inputs, devShell, packages | exposes falcon binary to jfit |

## Ontology Convergence
| Round | Entity Count | New | Changed | Stable | Stability Ratio |
|-------|-------------|-----|---------|--------|----------------|
| 1 | 3 | 3 | - | - | N/A |
| 2 | 5 | 2 | 0 | 3 | 60% |
| 3 | 6 | 1 | 0 | 5 | 83% |
| 4 | 7 | 1 | 0 | 6 | 86% |
| 5 | 8 | 1 | 0 | 7 | 88% |
| 6 | 8 | 0 | 0 | 8 | 100% |
| 7 | 8 | 0 | 0 | 8 | 100% |

## Interview Transcript
<details>
<summary>Full Q&A (7 rounds)</summary>

### Round 0 (Topology)
**Q:** I'm reading this as 5 top-level components: Dart Parser/AST, Rule Engine, Rule Port, CLI & Config, Nix Flake. Is that topology right?
**A:** Looks right — all 5 components are in scope.

### Round 1
**Q:** For parsing Dart source files (no official Rust parser exists), which approach do you want to take?
**A:** Write a Dart parser from scratch.
**Ambiguity:** 63.5% (Goal: 0.50, Constraints: 0.35, Criteria: 0.20)

### Round 2
**Q:** very_good_analysis is just a YAML config (no Rust source to port). dart_code_linter has ~100 rules, pyramid_lint ~30. What's the actual rule port scope?
**A:** Phase 1: dart_code_linter + pyramid_lint; Phase 2: built-in lints (very_good_analysis scope).
**Ambiguity:** 53.5% (Goal: 0.60, Constraints: 0.50, Criteria: 0.25)

### Round 3
**Q:** What exactly is falcon replacing — full dart analyze, or just the custom_lint plugin layer?
**A:** Phase 1: replace custom_lint only; Phase 2: full replacement for dart analyze.
**Ambiguity:** 43% (Goal: 0.75, Constraints: 0.55, Criteria: 0.35)

### Round 4 (Contrarian)
**Q:** Does falcon need to show diagnostics in your editor/IDE, or is CLI + CI the success target?
**A:** Full LSP server — editor integration required.
**Ambiguity:** 36.5% (Goal: 0.80, Constraints: 0.60, Criteria: 0.45)

### Round 5
**Q:** How slow is linting today, and what speed would make you say falcon is fast enough?
**A:** Under 1 second on full project.
**Ambiguity:** 28.5% (Goal: 0.85, Constraints: 0.65, Criteria: 0.60)

### Round 6 (Simplifier)
**Q:** jfit uses Dart 3.x. Which parser scope do you actually need?
**A:** Full Dart 3.x grammar (records, patterns, sealed).
**Ambiguity:** 23.7% (Goal: 0.88, Constraints: 0.72, Criteria: 0.65)

### Round 7
**Q:** I found package.nix + shell.nix in jfit/apps/mobile but no flake.nix there. How should falcon integrate with Nix?
**A:** It already exists at /home/jacob/Documents/Developer/jfit/flake.nix — add falcon as an input to it.
**Ambiguity:** 17.8% (Goal: 0.90, Constraints: 0.82, Criteria: 0.72) ✓ PASSED

</details>
