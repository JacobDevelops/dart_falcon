# Falcon 1.0 checklist

Workflow items and must-have rules must be resolved before falcon 1.0; nice-to-have rules are stretch goals and post-1.0 rules are out of scope (see [Priorities](#priorities)). A workflow item or rule row is deleted once it ships. `RULE_METADATA` in `crates/falcon_rules/src/meta.rs` is authoritative for shipped rule ids, groups, domains, execution kinds, and default enablement.

## Workflow

- [ ] **Safe autofixes.** Add CLI preview/apply, editor quick fixes, fix-all, and fix-on-save.
- [ ] **Existing-debt baselines.** Let teams adopt falcon without fixing historical findings, matching findings robustly across line changes rather than by rule/path/line/message.
- [ ] **CI report formats.** Add SARIF first, then CodeClimate/GitLab, Checkstyle, and Sonar where users need them, plus HTML; falcon already supports JSON and text.
- [ ] **Git-aware PR review.** Analyse changes since a git ref with project context, produce a PR-focused report, and optionally write a GitHub step summary or comment. This is a workflow gap, not an engine gap.
- [ ] **Severity-based CI thresholds.** Add an explicit fail-on warnings/errors policy alongside baselines; max-errors and custom exit codes are not the same interface.
- [ ] **Easier install and update.** Add an npm wrapper that downloads and verifies binaries, self-update and version selection, cargo-binstall metadata, optional signing/notarisation, and Homebrew publishing.
- [ ] **Package-aware monorepo runs.** Detect Melos/pub workspaces, apply per-package config, and aggregate per-package reports; falcon already analyses multiple paths.

## Priorities

- **must-have**: blocks 1.0. The rule is either an official preset member or a
  high-signal correctness/idiom check expected of a serious Dart/Flutter linter.
- **nice-to-have**: 1.0 stretch. Shipped if the workstream reaches it, but the
  release does not block on it.
- **post-1.0**: explicitly out of 1.0 scope (experimental, hyper-niche, or
  needs config/schema design that is itself post-1.0 work).

**Falcon groups** are `a11y | complexity | correctness | performance | security |
style | suspicious`.

**Analysis types** (what a rule needs from the engine):

- **syntax-only**: syntax-tree pass, no types.
- **needs-local-type-inference**: local flow/types.
- **needs-full-type-resolution**: element model / SDK type hierarchies.
- **needs-cross-file/project**: whole-program or pubspec/asset analysis.

## Rule totals

| Source | must-have | nice-to-have | post-1.0 | total |
|---|---:|---:|---:|---:|
| DCM-inspired | 39 | 132 | 24 | 195 |
| Cross-file additions | 4 | 4 | 4 | 12 |
| Additional rules | 6 | 7 | 3 | 16 |
| **Total** | **49** | **143** | **31** | **223** |

Six DCM rows were deduplicated against shipped official lints and the cross-file list (see
[Deduplication notes](#deduplication-notes)); the counts above are post-dedupe.

Additional rules are planned rows from outside the DCM catalog; their paywall column reads `n/a`. Requirements folded into existing rows are not counted again.

## Planned DCM-inspired rules (195)

These are candidates from the [DCM catalog](https://dcm.dev/docs/rules/). Paywall labels reflect the original survey, not a current pricing check; `paid?` means the per-rule badge was not verified.

Analysis legend: **S** syntax-only, **LT** needs-local-type-inference, **FT**
needs-full-type-resolution, **X** needs-cross-file/project. Falcon group assignments
for domain rules (flutter/test/bloc/riverpod/provider/flutter_hooks/equatable) are
provisional and should be confirmed at implementation time.

### 2a. core: correctness

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-duplicate-cascades` | correctness | S | paid? | Same cascade section invoked twice. | nice-to-have |
| `avoid-duplicate-exports` | correctness | S | paid | Duplicate export directives are redundant only when URI and combinators make the directive genuinely redundant. | nice-to-have |
| `avoid-duplicate-named-imports` | correctness | S | paid | Duplicate named import. | nice-to-have |
| `avoid-duplicate-mixins` | correctness | S | paid | Same mixin applied twice. | nice-to-have |
| `avoid-duplicate-factories` | correctness | S | paid | Duplicate factory declaration. | nice-to-have |
| `avoid-duplicate-initializers` | correctness | S | paid | Duplicate initializer entry. | nice-to-have |
| `avoid-unsafe-reduce` | correctness | FT | paid | `reduce` on possibly-empty iterable throws. | nice-to-have |
| `function-always-returns-null` | correctness | LT | paid | Function whose every return yields null. | nice-to-have |
| `function-always-returns-same-value` | correctness | LT | paid | Function returns one constant on all paths. | nice-to-have |
| `avoid-missing-enum-constant-in-map` | correctness | FT | paid | Map keyed by enum missing a constant. | nice-to-have |
| `avoid-constant-conditions` | correctness | LT | paid | Condition statically known constant. | nice-to-have |
| `avoid-constant-assert-conditions` | correctness | LT | paid | Assert condition statically known constant. | nice-to-have |
| `avoid-constant-switches` | correctness | LT | paid | Switch on a statically constant value. | nice-to-have |
| `avoid-self-compare` | correctness | S | paid | `x == x` (verify vs existing no-self-comparisons). | nice-to-have |
| `avoid-unused-after-null-check` | correctness | LT | paid | Value discarded after a null check. | nice-to-have |
| `avoid-unreachable-for-loop` | correctness | LT | paid | Loop body provably never runs. | nice-to-have |
| `avoid-unconditional-break` | correctness | S | paid | `break` that always fires on first iteration. | nice-to-have |
| `avoid-nested-futures` | correctness | FT | paid | `Future<Future<T>>` nesting mistake. | nice-to-have |
| `avoid-nested-streams-and-futures` | correctness | FT | paid | Nested stream/future type mistake. | nice-to-have |
| `avoid-missing-completer-stack-trace` | correctness | FT | paid | `Completer.completeError` without stack trace. | nice-to-have |
| `prefer-random-secure` | security | FT | paid | `Random()` where `Random.secure()` intended. | nice-to-have |
| `avoid-sensitive-query-params` | security | S | paid | Secrets/tokens in URL query params. | nice-to-have |
| `avoid-assigning-to-static-field` | correctness | LT | paid | Mutating static state. | nice-to-have |
| `match-getter-setter-field-names` | correctness | LT | paid | Getter/setter names disagree with backing field. | nice-to-have |
| `prefer-correct-json-casts` | correctness | FT | paid | Unsafe casts decoding JSON maps. | nice-to-have |
| `avoid-recursive-calls` | correctness | LT | paid | Unbounded self-recursion (falcon has recursive-getters only). | nice-to-have |
| `avoid-not-encodable-in-to-json` | correctness | FT | paid | `toJson` returns a non-encodable value. | post-1.0 |
| `require-atomic-async-updates` | correctness | FT | paid | Read-modify-write across an await race. | post-1.0 |
| `pass-correct-accepted-type` | correctness | FT | paid | Argument type mismatch (DCM-style). | post-1.0 |
| `handle-throwing-invocations` | correctness | FT | paid | Call to throwing API without try/catch. | post-1.0 |

### 2b. core: suspicious

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-banned-names` | suspicious | S | paid | Config-driven forbidden identifiers. | nice-to-have |
| `avoid-banned-types` | suspicious | FT | paid | Config-driven forbidden types. | nice-to-have |
| `avoid-banned-annotations` | suspicious | S | paid | Config-driven forbidden annotations. | nice-to-have |
| `avoid-banned-exports` | suspicious | S | paid | Config-driven forbidden exports. | nice-to-have |
| `banned-usage` | suspicious | FT | paid | Config-driven banned API usage. | nice-to-have |
| `avoid-barrel-files` | suspicious | X | paid | Discourage barrel/index re-export files. | nice-to-have |
| `avoid-double-slash-imports` | suspicious | S | paid | `//` in import path. | nice-to-have |
| `avoid-incorrect-uri` | suspicious | S | paid | Malformed URI in import/annotation. | nice-to-have |
| `avoid-accessing-other-classes-private-members` | suspicious | FT | paid | Reaching into another class's privates. | nice-to-have |
| `avoid-referencing-subclasses` | suspicious | FT | paid | Base class referencing its subclasses. | nice-to-have |
| `avoid-suspicious-super-overrides` | suspicious | FT | paid | Override that discards super behavior. | nice-to-have |
| `no-magic-string` | suspicious | S | paid | String-literal analog of no-magic-number. | nice-to-have |
| `avoid-non-final-exception-class-fields` | suspicious | LT | paid | Mutable fields on exception classes. | nice-to-have |
| `avoid-mutating-parameters` | suspicious | LT | paid | Reassigning/mutating a parameter. | nice-to-have |
| `avoid-nested-assignments` | suspicious | S | paid | Assignment buried in an expression. | nice-to-have |
| `avoid-multi-assignment` | suspicious | S | paid | Chained multi-assignment. | nice-to-have |
| `avoid-negated-conditions` | suspicious | S | paid | Prefer positive conditions for readability. | nice-to-have |
| `avoid-inverted-boolean-checks` | suspicious | S | paid | Prefer non-inverted boolean checks. | nice-to-have |
| `avoid-importing-entrypoint-exports` | suspicious | X | paid | Importing from an app entrypoint. | post-1.0 |
| `avoid-similar-names` | suspicious | S | paid | Confusingly similar identifiers in scope. | post-1.0 |

### 2c. core: style

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `prefer-any-or-every` | style | FT | paid | Manual loop replaceable by any/every. | nice-to-have |
| `prefer-getter-over-method` | style | LT | paid | Zero-arg method that should be a getter. | nice-to-have |
| `prefer-named-parameters` | style | S | paid | Too many positional params → named. | nice-to-have |
| `prefer-boolean-prefixes` | style | S | paid | Bool getters/fields lacking is/has/should prefix (verify vs boolean-prefixes). | nice-to-have |
| `prefer-match-file-name` | style | X | paid | Sole class or enum in a file should match the filename; naming conventions are checked separately. | nice-to-have |
| `prefer-type-over-var` | style | LT | paid | Explicit type instead of `var` where unclear. | nice-to-have |
| `prefer-correct-error-name` | style | S | paid | Naming convention for error types. | nice-to-have |
| `prefer-correct-handler-name` | style | S | paid | Naming convention for handlers. | nice-to-have |
| `prefer-correct-callback-field-name` | style | S | paid | Naming convention for callback fields. | nice-to-have |
| `prefer-commenting-analyzer-ignores` | style | S | paid | `// ignore:` must carry an explanation. | nice-to-have |
| `newline-before-method` | style | S | paid | Blank line before method declarations. | nice-to-have |
| `newline-before-case` | style | S | paid | Blank line before case clauses. | nice-to-have |
| `newline-before-throw` | style | S | paid | Blank line before throw. | nice-to-have |
| `newline-before-continue` | style | S | paid | Blank line before continue. | nice-to-have |
| `newline-before-break` | style | S | paid | Blank line before break. | nice-to-have |
| `newline-before-constructor` | style | S | paid | Blank line before constructor. | nice-to-have |
| `enum-constants-ordering` | style | S | paid | Config-driven enum constant ordering. | nice-to-have |
| `map-keys-ordering` | style | S | paid | Config-driven map key ordering. | nice-to-have |
| `parameters-ordering` | style | S | paid | Config-driven parameter ordering. | nice-to-have |
| `arguments-ordering` | style | S | paid | Config-driven argument ordering. | nice-to-have |
| `initializers-ordering` | style | S | paid | Config-driven initializer ordering. | nice-to-have |
| `pattern-fields-ordering` | style | S | paid | Config-driven pattern-field ordering. | nice-to-have |
| `record-fields-ordering` | style | S | paid | Config-driven record-field ordering. | nice-to-have |
| `prefer-single-declaration-per-file` | style | S | paid | One public type per file. | post-1.0 |
| `prefer-single-widget-per-file` | style | S | paid | One widget per file. | post-1.0 |
| `prefer-prefixed-global-constants` | style | S | paid | Global const naming convention. | post-1.0 |
| `match-class-name-pattern` | style | S | paid | Class names must match a configured regex. | post-1.0 |

### 2d. core: complexity

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-long-records` | complexity | S | paid | Record size limit (falcon has param-list). | nice-to-have |
| `avoid-complex-arithmetic-expressions` | complexity | S | paid | Arithmetic expression complexity. | nice-to-have |
| `avoid-if-with-many-branches` | complexity | S | paid | Long if/else-if chains → switch. | nice-to-have |
| `avoid-nested-switches` | complexity | S | paid | Deeply nested switch statements. | nice-to-have |
| `avoid-nested-switch-expressions` | complexity | S | paid | Deeply nested switch expressions. | nice-to-have |
| `avoid-nested-try-statements` | complexity | S | paid | Deeply nested try statements. | nice-to-have |
| `max-statements` | complexity | S | paid | Statement-count-per-function metric. | nice-to-have |
| `cognitive-complexity` | complexity | S | n/a | Nesting-weighted complexity metric, distinct from cyclomatic complexity and maximum nesting level. | nice-to-have |
| `widget-tree-depth` | complexity | FT | n/a | Widget-tree nesting depth from real widget structure, not parenthesis counting. | nice-to-have |
| `avoid-local-functions` | complexity | S | paid | Overuse of local functions. | post-1.0 |
| `avoid-excessive-expressions` | complexity | S | paid | Expression-length metric. | post-1.0 |

### 2e. core: performance

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-unnecessary-collections` | performance | LT | paid | Collection built only to be iterated once. | nice-to-have |
| `move-variable-closer-to-its-usage` | performance | LT | paid | Hoisted variable used in one branch. | nice-to-have |
| `avoid-unnecessary-futures` | performance | FT | paid | `async`/Future wrapper that adds no value. | nice-to-have |
| `prefer-return-await` | performance | FT | paid | `return await` needed for correct stack traces. | nice-to-have |
| `prefer-bytes-builder` | performance | FT | paid | Building byte lists without BytesBuilder. | post-1.0 |

### 2f. core: unnecessary/redundant cleanups (style)

Not covered by falcon's existing `unnecessary-*` family; verify each against the
shipped set before implementing. Several are near-twins that should be aliased,
not re-added.

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-unnecessary-block` | style | S | paid | Redundant block statement. | nice-to-have |
| `avoid-unnecessary-call` | style | FT | paid | Redundant `.call()`. | nice-to-have |
| `avoid-unnecessary-continue` | style | S | paid | Redundant trailing continue. | nice-to-have |
| `avoid-unnecessary-negations` | style | S | paid | Double/redundant negation. | nice-to-have |
| `avoid-unnecessary-length-check` | style | LT | paid | Redundant `.length` check before access. | nice-to-have |
| `avoid-unnecessary-local-variable` | style | LT | paid | Local variable used once immediately. | nice-to-have |
| `avoid-unnecessary-super` | style | LT | paid | Redundant super call. | nice-to-have |
| `avoid-unnecessary-overrides-in-state` | style | LT | paid | State override that only calls super. | nice-to-have |
| `avoid-unnecessary-getter` | style | LT | paid | Getter that only returns a field. | nice-to-have |
| `avoid-unnecessary-reassignment` | style | LT | paid | Variable reassigned to the same value. | nice-to-have |
| `use-existing-variable` | style | LT | paid | Reuse an existing binding instead of recomputing. | nice-to-have |
| `use-existing-destructuring` | style | LT | paid | Reuse an existing destructuring binding. | nice-to-have |

### 2g. flutter domain

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-undisposed-instances` | correctness | FT | paid | Controllers/notifiers never disposed. | must-have |
| `dispose-focus-node-and-timer` | correctness | FT | n/a | Extend disposal checks to FocusNode and Timer fields, proving FocusNode.dispose() or Timer.cancel() actually runs. | must-have |
| `avoid-hardcoded-credentials` | security | S | n/a | Warn when credential-like names are assigned literal values; heuristic, not a full secrets scanner. | nice-to-have |
| `accessible-widget-labels` | a11y | LT | n/a | Warn on interactive widgets missing labels; heuristic, not compliance validation. Image labels are covered by `avoid-missing-image-alt`. | nice-to-have |
| `minimum-touch-target` | a11y | LT | n/a | Warn on explicitly undersized interactive touch targets; heuristic, not compliance validation. | nice-to-have |
| `dispose-fields` | correctness | FT | paid | Disposable fields not disposed in `dispose()`. | must-have |
| `always-remove-listener` | correctness | FT | paid | `addListener` without `removeListener`. | must-have |
| `avoid-recursive-widget-calls` | correctness | LT | paid | Widget builds itself → stack overflow. | must-have |
| `avoid-shrink-wrap-in-lists` | performance | LT | paid | `shrinkWrap: true` perf trap in scrollables. | must-have |
| `prefer-align-over-container` | style | LT | paid | Container used only for alignment → Align. | must-have |
| `prefer-center-over-align` | style | LT | paid | Align used for centering → Center. | must-have |
| `prefer-padding-over-container` | style | LT | paid | Container used only for padding → Padding. | must-have |
| `prefer-constrained-box-over-container` | style | LT | paid | Container used only for constraints → ConstrainedBox. | must-have |
| `prefer-transform-over-container` | style | LT | paid | Container used only for transform → Transform. | must-have |
| `prefer-container` | style | LT | paid | Collapse stacked layout widgets into one Container. | must-have |
| `prefer-for-loop-in-children` | style | S | paid | `.map().toList()` in children → collection-for. | must-have |
| `prefer-action-button-tooltip` | a11y | LT | paid | IconButton/action lacking a tooltip (a11y). | must-have |
| `avoid-unnecessary-stateful-widgets` | correctness | FT | paid | StatefulWidget with no state → Stateless. | must-have |
| `avoid-mounted-in-setstate` | correctness | FT | paid | `mounted` check misuse around setState. | nice-to-have |
| `prefer-single-setstate` | performance | FT | paid | Multiple setState calls collapsible. | nice-to-have |
| `avoid-disposing-late-fields` | correctness | LT | paid | Disposing a late field that may be unset. | nice-to-have |
| `avoid-missing-controller` | correctness | FT | paid | Widget expecting a controller left unset. | nice-to-have |
| `always-pass-global-key` | correctness | LT | paid | GlobalKey should be passed, not created inline. | nice-to-have |
| `avoid-border-all` | performance | S | paid | `Border.all` → const-friendly alternative. | nice-to-have |
| `avoid-incorrect-image-opacity` | correctness | S | paid | Opacity widget wrapping Image (use color/opacity). | nice-to-have |
| `avoid-missing-image-alt` | a11y | S | paid | Image without a semantic label (a11y). | nice-to-have |
| `avoid-wrapping-in-padding` | style | LT | paid | Padding widget where Container padding suffices. | nice-to-have |
| `prefer-sized-box-square` | style | S | paid | `SizedBox(w:x,h:x)` → `SizedBox.square`. | nice-to-have |
| `prefer-using-list-view` | performance | LT | paid | Column + SingleChildScrollView → ListView. | nice-to-have |
| `prefer-define-hero-tag` | correctness | LT | paid | Hero without a tag. | nice-to-have |
| `prefer-void-callback` | style | S | paid | `void Function()` → `VoidCallback`. | nice-to-have |
| `avoid-unnecessary-gesture-detector` | performance | LT | paid | GestureDetector wrapping an already-tappable widget. | nice-to-have |
| `prefer-text-rich` | style | LT | paid | `Text.rich`/RichText guidance. | post-1.0 |
| `proper-super-calls` | correctness | FT | paid | initState/dispose super-call ordering (verify vs proper-super-init-state). | post-1.0 |
| `avoid-non-ascii-symbols` | style | S | n/a | Opt-in warning for non-ASCII symbols. | post-1.0 |

### 2h. test domain (new)

Activated for `*_test.dart`. Falcon ships none of these today.

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-duplicate-test-assertions` | correctness | S | paid | Same assertion repeated in a test. | must-have |
| `missing-test-assertion` | correctness | LT | paid | Test body with no expect/verify. | must-have |
| `avoid-empty-test-groups` | correctness | S | paid | `group()` with no tests. | nice-to-have |
| `prefer-unique-test-names` | suspicious | S | paid | Duplicate test descriptions. | nice-to-have |
| `prefer-correct-test-file-name` | style | X | paid | Test file naming convention. | nice-to-have |
| `prefer-expect-later` | correctness | FT | paid | Async matcher needs `expectLater`. | nice-to-have |
| `prefer-test-matchers` | style | FT | paid | Use dedicated matchers over manual expect. | nice-to-have |
| `avoid-top-level-members-in-tests` | suspicious | S | paid | Top-level state leaking across tests. | nice-to-have |
| `allow-top-level-test-helpers` | suspicious | S | n/a | Opt-in convention check for top-level helpers in test files. | post-1.0 |
| `avoid-missing-test-files` | correctness | X | paid | Source file without a matching test. | post-1.0 |
| `avoid-misused-test-matchers` | correctness | FT | paid | Matcher applied to the wrong type. | post-1.0 |
| `format-test-name` | style | S | paid | Test description formatting. | post-1.0 |
| `prefer-test-structure` | style | S | paid | Arrange/act/assert structure. | post-1.0 |

### 2i. bloc domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `emit-new-bloc-state-instances` | correctness | FT | paid | Emitting the same state instance. | must-have |
| `prefer-sealed-bloc-events` | correctness | FT | paid | Bloc events should be sealed. | must-have |
| `prefer-sealed-bloc-state` | correctness | FT | paid | Bloc state should be sealed. | must-have |
| `prefer-immutable-bloc-events` | correctness | FT | paid | Bloc events must be immutable. | must-have |
| `prefer-immutable-bloc-state` | correctness | FT | paid | Bloc state must be immutable. | must-have |
| `avoid-duplicate-bloc-event-handlers` | correctness | FT | paid | Same event registered twice. | must-have |
| `check-is-not-closed-after-async-gap` | correctness | FT | paid | `emit` after await without an isClosed check. | must-have |
| `handle-bloc-event-subclasses` | correctness | FT | paid | `on<Event>` missing subclass handling. | nice-to-have |
| `avoid-passing-bloc-to-bloc` | suspicious | FT | paid | Bloc dependency anti-pattern. | nice-to-have |
| `avoid-passing-build-context-to-blocs` | suspicious | FT | paid | BuildContext leaking into a bloc. | nice-to-have |
| `avoid-bloc-public-fields` | style | LT | paid | Bloc encapsulation (fields). | nice-to-have |
| `avoid-bloc-public-methods` | style | LT | paid | Bloc encapsulation (methods). | nice-to-have |
| `avoid-emit-outside-bloc` | correctness | FT | n/a | Prevent emitting outside a Bloc. | must-have |
| `avoid-passing-bloc-to-widget` | suspicious | FT | n/a | Avoid passing a Bloc directly to a widget. | nice-to-have |
| `prefer-bloc-extensions` | style | LT | n/a | Prefer Bloc extensions for supported APIs. | nice-to-have |
| `prefer-bloc-event-suffix` | style | S | paid | Event naming convention. | nice-to-have |
| `prefer-bloc-state-suffix` | style | S | paid | State naming convention. | nice-to-have |
| `prefer-multi-bloc-provider` | style | LT | paid | Nested providers → MultiBlocProvider. | nice-to-have |
| `avoid-empty-build-when` | correctness | S | paid | Empty `buildWhen` callback. | post-1.0 |
| `avoid-cubits` | style | LT | paid | Opinionated: prefer Bloc over Cubit. | post-1.0 |
| `avoid-returning-value-from-cubit-methods` | style | LT | paid | Cubit methods should be void. | post-1.0 |

### 2j. riverpod domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-ref-watch-outside-build` | correctness | FT | paid | `ref.watch` outside build. | must-have |
| `avoid-ref-read-inside-build` | correctness | FT | paid | `ref.read` inside build. | must-have |
| `use-ref-read-synchronously` | correctness | FT | paid | Ref used after await without mounted. | must-have |
| `prefer-ref-read-for-methods` | correctness | FT | n/a | Prefer ref.read for method actions rather than watching state. | must-have |
| `use-ref-and-state-synchronously` | correctness | FT | paid | Ref/state used after await without mounted. | must-have |
| `avoid-calling-notifier-members-inside-build` | correctness | FT | paid | Mutating a notifier during build. | must-have |
| `dispose-provided-instances` | correctness | FT | paid | Provided disposables not disposed. | must-have |
| `dispose-provider-resources` | correctness | FT | n/a | Provider-created resources must use autoDispose or ref.onDispose for cleanup; avoid misleading scope-leak terminology. | must-have |
| `prefer-async-value-when` | style | LT | n/a | Optional style preference for AsyncValue.when. | post-1.0 |
| `avoid-unnecessary-consumer-widgets` | performance | LT | paid | ConsumerWidget without ref use. | must-have |
| `avoid-ref-inside-state-dispose` | correctness | FT | paid | ref used in dispose. | nice-to-have |
| `avoid-public-notifier-properties` | style | LT | paid | Notifier exposing mutable state. | nice-to-have |
| `prefer-immutable-provider-arguments` | correctness | FT | paid | Provider family args must be immutable. | nice-to-have |
| `prefer-riverpod-notifier-suffix` | style | S | paid | Notifier naming convention. | nice-to-have |
| `prefer-riverpod-provider-suffix` | style | S | paid | Provider naming convention. | nice-to-have |
| `prefer-correct-notifier-file-name` | style | X | paid | Notifier file naming convention. | nice-to-have |
| `prefer-correct-provider-file-name` | style | X | paid | Provider file naming convention. | nice-to-have |
| `avoid-notifier-constructors` | style | LT | paid | Notifiers shouldn't declare constructors. | post-1.0 |
| `prefer-single-notifier-per-file` | style | S | paid | One notifier per file. | post-1.0 |

### 2k. provider domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-read-inside-build` | correctness | FT | paid | `.read` inside build. | must-have |
| `avoid-watch-outside-build` | correctness | FT | paid | `.watch` outside build. | must-have |
| `dispose-providers` | correctness | FT | paid | Provided disposables not disposed. | must-have |
| `prefer-read-for-methods` | correctness | FT | n/a | Prefer read for method actions rather than watching state. | must-have |
| `avoid-instantiating-in-value-provider` | correctness | FT | paid | `.value` provider creating a new instance. | nice-to-have |
| `prefer-multi-provider` | style | LT | paid | Nested providers → MultiProvider. | nice-to-have |
| `prefer-provider-extensions` | style | LT | paid | Use context extensions over `Provider.of`. | nice-to-have |

### 2l. flutter_hooks domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-conditional-hooks` | correctness | S | paid | Hooks called conditionally break rules-of-hooks. | must-have |
| `avoid-hooks-outside-build` | correctness | FT | paid | Hook used outside a build method. | must-have |
| `avoid-misused-hooks` | correctness | FT | paid | Hook misuse. | must-have |
| `avoid-unnecessary-hook-widgets` | performance | LT | paid | HookWidget with no hooks. | must-have |
| `prefer-use-callback` | performance | FT | paid | Wrap callbacks in `useCallback`. | nice-to-have |
| `prefer-use-memo` | performance | FT | paid | Wrap values in `useMemo`. | nice-to-have |

### 2m. equatable domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `list-all-equatable-fields` | correctness | FT | paid | Field missing from Equatable props. | must-have |
| `add-equatable-props` | correctness | FT | paid | Equatable subclass missing a props override. | must-have |
| `extend-equatable` | style | LT | paid | Value class should extend Equatable; a library preference, not a correctness improvement. | nice-to-have |
| `avoid-mutable-equatable-subclass-fields` | correctness | FT | n/a | Mutable fields on subclasses inheriting Equatable equality can invalidate equality/hash behavior. | must-have |
| `prefer-equatable-mixin` | style | LT | paid | Use EquatableMixin where appropriate. | nice-to-have |

### Deduplication notes

These DCM rows are covered elsewhere and are **not** counted in the DCM totals:

| DCM rule | listed instead as | reason |
|---|---|---|
| `avoid-collection-methods-with-unrelated-types` | official `collection_methods_unrelated_type` | same check; prefer official naming |
| `prefer-contains` | official `prefer_contains` | same check; prefer official naming |
| `avoid-self-assignment` | official `no_self_assignments` | same check; prefer official naming |
| `avoid-unnecessary-parentheses` | official `unnecessary_parenthesis` | same check; prefer official naming |
| `avoid-unnecessary-statements` | official `unnecessary_statements` | same check; prefer official naming |
| `avoid-banned-imports` | cross-file `banned-imports` | same architecture-boundary check; lives in the cross-file pass |

Related-but-distinct pairs kept separately (noted for implementers):
`avoid-collection-equality-checks` (collections) vs official
`unrelated_type_equality_checks` (unrelated types); DCM's granular BuildContext
checks (`use-setstate-synchronously`, `use-closest-build-context`) vs official
`use_build_context_synchronously`; `avoid-duplicate-switch-case-conditions` vs
falcon's shipped `no-duplicate-case-values`; `avoid-self-compare` vs falcon's
shipped `no-self-comparisons`.
Deduplicated or partially covered requirements: `avoid-bloc-public-methods` and `prefer-multi-bloc-provider` are existing rows; `prefer-ref-read-for-methods` is covered by neither existing provider nor riverpod rows and is listed separately. setState after an async gap is `use-setstate-synchronously` and setState in lifecycle methods is `avoid-unnecessary-setstate`, so neither gets its own row; the Equatable preference is `extend-equatable`. Image labels are covered by `avoid-missing-image-alt`; disposal work is limited to `super.dispose()`, FocusNode.dispose(), and Timer.cancel() coverage absent from existing disposal checks. The cascade-precedence, cognitive-complexity, widget-tree-depth, and filename-correspondence checks are distinct from shipped style or complexity checks; duplicate exports must account for combinators.

The survey excludes rules already ported by falcon and niche-package rules for flame, patrol, mocktail, get_it, and fake_async.

Additional planned rows cover uncovered disposal cases, accessibility labels and touch targets, provider cleanup, mutable Equatable subclasses, cognitive complexity, widget-tree depth, redundant exports, and optional conventions.

---

## Planned cross-file rules (12)

Whole-project / app-wide rules live in the `cross-file` config section.
The current shipped set is defined by the cross-file entries in `RULE_METADATA`;
the rows below record the planned additions. Difficulty reflects existing
infrastructure: **A** reuses the resolved directive graph / lexical name index;
**B** needs a new pubspec index
(`dependencies`/`dev_dependencies`/`flutter.assets`); **C** needs a new
asset/l10n index (filesystem walk and/or `.arb` parsing).

**Shipped rules:** `unused-code`, `unused-files`, `unnecessary-nullable`, `depend-on-referenced-packages`, `package-names`, `secure-pubspec-urls`, and `sort-pub-dependencies`. The four shipped official lints are not counted in the 12 planned additions below.

### 3a. Tier 1: must-have (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-dependencies` | correctness | needs-cross-file/project | B | A package in pubspec `dependencies:` that no source file imports via `package:<dep>/…`. |
| `undeclared-dependencies` | correctness | needs-cross-file/project | B | A `package:<x>/…` import whose `<x>` is neither a declared (dev)dependency nor the package itself. |
| `no-import-cycles` | correctness | needs-cross-file/project | A | Import/export cycle between library files; report the full cycle chain. |
| `banned-imports` | correctness | needs-cross-file/project | A | User-configured architecture boundaries: files matching glob X may not import glob Y. |

### 3b. Tier 2: nice-to-have (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-assets` | correctness | needs-cross-file/project | C | Asset declared in pubspec `flutter: assets:` but no string literal references its path. |
| `unused-l10n` | correctness | needs-cross-file/project | C | Generated localization key (`.arb`) never referenced through the generated accessor. |
| `architecture-layer-boundaries` | correctness | needs-cross-file/project | A | Warn when imports cross clean-architecture or feature-first layers classified heuristically by path. |
| `unexported-public-api` | style | needs-cross-file/project | A | Public declaration used across the package but never surfaced through a barrel/public library. |

### 3c. Tier 3: post-1.0 (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `missing-assets` | suspicious | needs-cross-file/project | C | String literal that looks like an asset path (`assets/…`) not declared in pubspec. |
| `unused-dev-dependencies` | correctness | needs-cross-file/project | B | Dev-dependency imported nowhere under `test/`, `tool/`, `integration_test/`. |
| `no-deep-package-imports` | correctness | needs-cross-file/project | A | Importing another package's internal file past its public barrel. |
| `duplicate-barrel-exports` | suspicious | needs-cross-file/project | A | Same symbol re-exported by two barrels → ambiguous import. |
