# Falcon 1.0 checklist

Everything here must be resolved before falcon 1.0. Engine items are checked off when they land; a rule row is deleted once the rule ships. `RULE_METADATA` in `crates/falcon_rules/src/meta.rs` is authoritative for shipped rule ids, groups, domains, execution kinds, and default enablement.

## Engine

- [ ] **Per-package-context library identity.** When one repository holds several package contexts that resolve the same `package:` URI to different copies (for example two apps pinned to different versions of a dependency), every file must resolve against the copy from its own package context. Today the lexically last copy wins for all files: deterministic, but a two-app reproduction (`foo` 1.0 vs 2.0) still produces one false positive in the app whose copy loses.
- [x] **Don't cache failed parser-thread spawns.** If the parser thread can't start while loading a dependency or companion part, the empty result is cached as a real parse (companion directive cache, external-load "attempted" set). The LSP then keeps that library unresolved until the environment resets, and a part's import/part edges stay empty until its text changes.
- [x] **Surface cross-file thread spawn failures in the LSP.** The error is attached to the first collected file only; when that file isn't an open document, the error never shows and cross-file rules silently report nothing.
- [ ] **Concrete syntax tree.** Replace the AST plus retained token stream with an arena CST that keeps every token and trivia, so new rules read syntax from one tree and never need parser changes.

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

- **syntax-only**: AST/token pass, no types.
- **needs-local-type-inference**: local flow/types.
- **needs-full-type-resolution**: element model / SDK type hierarchies.
- **needs-cross-file/project**: whole-program or pubspec/asset analysis.

## Rule totals

| Source | must-have | nice-to-have | post-1.0 | total |
|---|---:|---:|---:|---:|
| DCM-inspired | 75 | 132 | 24 | 231 |
| Cross-file additions | 4 | 3 | 4 | 11 |
| **Total** | **79** | **135** | **28** | **242** |

Six DCM rows were deduplicated against shipped official lints and the cross-file list (see
[Deduplication notes](#deduplication-notes)); the counts above are post-dedupe.

## Planned DCM-inspired rules (231)

These are candidates from the [DCM catalog](https://dcm.dev/docs/rules/). Paywall labels reflect the original survey, not a current pricing check; `paid?` means the per-rule badge was not verified.

Analysis legend: **S** syntax-only, **LT** needs-local-type-inference, **FT**
needs-full-type-resolution, **X** needs-cross-file/project. Falcon group assignments
for domain rules (flutter/test/bloc/riverpod/provider/flutter_hooks/equatable) are
provisional and should be confirmed at implementation time.

### 2a. core: correctness

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-duplicate-map-keys` | correctness | S | paid? | Duplicate keys in a map literal silently drop entries. | must-have |
| `avoid-duplicate-collection-elements` | correctness | LT | paid | Duplicate elements in set/list literal. | must-have |
| `avoid-duplicate-switch-case-conditions` | correctness | S | paid | Two switch cases with equal constant conditions. | must-have |
| `avoid-collection-equality-checks` | correctness | FT | paid | `==` between collections never compares contents. | must-have |
| `avoid-unrelated-type-casts` | correctness | FT | paid | Cast between unrelated types always fails. | must-have |
| `avoid-unsafe-collection-methods` | correctness | FT | paid | `.first`/`.single`/`[i]` that can throw at runtime. | must-have |
| `avoid-missing-interpolation` | correctness | S | paid | `$name` written as literal text outside interpolation. | must-have |
| `avoid-contradictory-expressions` | correctness | LT | paid | `a && !a` style always-false/true conditions. | must-have |
| `avoid-unmodified-loop-condition` | correctness | LT | paid | Loop variable never changes → infinite loop. | must-have |
| `avoid-uncaught-future-errors` | correctness | FT | paid | Future error path with no catch. | must-have |
| `avoid-weak-cryptographic-algorithms` | security | FT | paid | MD5/SHA1/weak cipher usage. | must-have |
| `avoid-duplicate-cascades` | correctness | S | paid? | Same cascade section invoked twice. | nice-to-have |
| `avoid-duplicate-exports` | correctness | S | paid | Duplicate export directive. | nice-to-have |
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
| `avoid-commented-out-code` | suspicious | S | paid | Commented-out code blocks left behind. | must-have |
| `avoid-shadowing` | suspicious | LT | paid | Local shadows an outer member. | must-have |
| `avoid-shadowed-extension-methods` | suspicious | LT | paid | Local shadows an extension member. | must-have |
| `avoid-assignments-as-conditions` | suspicious | S | paid | `if (x = y)` likely-bug. | must-have |
| `avoid-bitwise-operators-with-booleans` | suspicious | S | paid | `&`/`\|` on bools instead of `&&`/`\|\|`. | must-have |
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
| `prefer-early-return` | style | S | paid | Flatten nested if via early return. | must-have |
| `prefer-returning-condition` | style | S | paid | `if (x) return true; return false;` → `return x;`. | must-have |
| `prefer-switch-expression` | style | S | paid | Convert switch statement to Dart 3 switch expression. | must-have |
| `prefer-for-in` | style | S | paid | Index loop convertible to for-in. | must-have |
| `prefer-named-boolean-parameters` | style | S | paid | Bare bool positional params hurt call sites. | must-have |
| `prefer-any-or-every` | style | FT | paid | Manual loop replaceable by any/every. | nice-to-have |
| `prefer-getter-over-method` | style | LT | paid | Zero-arg method that should be a getter. | nice-to-have |
| `prefer-named-parameters` | style | S | paid | Too many positional params → named. | nice-to-have |
| `prefer-boolean-prefixes` | style | S | paid | Bool getters/fields lacking is/has/should prefix (verify vs boolean-prefixes). | nice-to-have |
| `prefer-match-file-name` | style | X | paid | Top-level type name should match file name. | nice-to-have |
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
| `avoid-complex-conditions` | complexity | S | paid | Boolean-expression complexity limit. | must-have |
| `avoid-complex-loop-conditions` | complexity | S | paid | Loop-condition complexity limit. | must-have |
| `avoid-collapsible-if` | complexity | S | paid | Nested if mergeable into one condition. | must-have |
| `avoid-long-records` | complexity | S | paid | Record size limit (falcon has param-list). | nice-to-have |
| `avoid-complex-arithmetic-expressions` | complexity | S | paid | Arithmetic expression complexity. | nice-to-have |
| `avoid-if-with-many-branches` | complexity | S | paid | Long if/else-if chains → switch. | nice-to-have |
| `avoid-nested-switches` | complexity | S | paid | Deeply nested switch statements. | nice-to-have |
| `avoid-nested-switch-expressions` | complexity | S | paid | Deeply nested switch expressions. | nice-to-have |
| `avoid-nested-try-statements` | complexity | S | paid | Deeply nested try statements. | nice-to-have |
| `max-statements` | complexity | S | paid | Statement-count-per-function metric. | nice-to-have |
| `avoid-local-functions` | complexity | S | paid | Overuse of local functions. | post-1.0 |
| `avoid-excessive-expressions` | complexity | S | paid | Expression-length metric. | post-1.0 |

### 2e. core: performance

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-slow-collection-methods` | performance | FT | paid | Slow patterns (`.length == 0`, repeated `.where().first`). | must-have |
| `move-variable-outside-iteration` | performance | LT | paid | Loop-invariant allocation inside a loop. | must-have |
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
| `avoid-unnecessary-if` | style | LT | paid | Redundant if that can be simplified. | must-have |
| `avoid-unnecessary-conditionals` | style | LT | paid | Redundant conditional expression. | must-have |
| `avoid-unnecessary-return` | style | S | paid | Redundant trailing return. | must-have |
| `avoid-redundant-else` | style | S | paid | `else` after a returning if. | must-have |
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
| `use-setstate-synchronously` | correctness | FT | paid | `setState` after await without mounted check. | must-have |
| `avoid-empty-setstate` | correctness | S | paid | `setState(() {})` with an empty body. | must-have |
| `avoid-unnecessary-setstate` | correctness | FT | paid | setState in build/init where needless. | must-have |
| `use-closest-build-context` | correctness | FT | paid | Wrong BuildContext across an async gap. | must-have |
| `avoid-late-context` | correctness | FT | paid | Storing BuildContext in a late field. | must-have |
| `avoid-inherited-widget-in-initstate` | correctness | FT | paid | `.of(context)` in initState. | must-have |
| `avoid-undisposed-instances` | correctness | FT | paid | Controllers/notifiers never disposed. | must-have |
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
| `use-ref-and-state-synchronously` | correctness | FT | paid | Ref/state used after await without mounted. | must-have |
| `avoid-calling-notifier-members-inside-build` | correctness | FT | paid | Mutating a notifier during build. | must-have |
| `dispose-provided-instances` | correctness | FT | paid | Provided disposables not disposed. | must-have |
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
| `extend-equatable` | style | LT | paid | Value class should extend Equatable. | nice-to-have |
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

The survey excludes rules already ported by falcon and niche-package rules for flame, patrol, mocktail, get_it, and fake_async.

---

## Planned cross-file rules (11)

Whole-project / app-wide rules live in the `cross-file` config section.
The current shipped set is defined by the cross-file entries in `RULE_METADATA`;
the rows below record the planned additions. Difficulty reflects existing
infrastructure: **A** reuses the resolved directive graph / lexical name index;
**B** needs a new pubspec index
(`dependencies`/`dev_dependencies`/`flutter.assets`); **C** needs a new
asset/l10n index (filesystem walk and/or `.arb` parsing).

**Shipped rules:** `unused-code`, `unused-files`, `unnecessary-nullable`, `depend-on-referenced-packages`, `package-names`, `secure-pubspec-urls`, and `sort-pub-dependencies`. The four shipped official lints are not counted in the 11 planned additions below.

### 3a. Tier 1: must-have (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-dependencies` | correctness | needs-cross-file/project | B | A package in pubspec `dependencies:` that no source file imports via `package:<dep>/…`. |
| `undeclared-dependencies` | correctness | needs-cross-file/project | B | A `package:<x>/…` import whose `<x>` is neither a declared (dev)dependency nor the package itself. |
| `no-import-cycles` | correctness | needs-cross-file/project | A | Import/export cycle between library files (SCC over the resolved directive graph). |
| `banned-imports` | correctness | needs-cross-file/project | A | User-configured architecture boundaries: files matching glob X may not import glob Y. |

### 3b. Tier 2: nice-to-have (3)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-assets` | correctness | needs-cross-file/project | C | Asset declared in pubspec `flutter: assets:` but no string literal references its path. |
| `unused-l10n` | correctness | needs-cross-file/project | C | Generated localization key (`.arb`) never referenced through the generated accessor. |
| `unexported-public-api` | style | needs-cross-file/project | A | Public declaration used across the package but never surfaced through a barrel/public library. |

### 3c. Tier 3: post-1.0 (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `missing-assets` | suspicious | needs-cross-file/project | C | String literal that looks like an asset path (`assets/…`) not declared in pubspec. |
| `unused-dev-dependencies` | correctness | needs-cross-file/project | B | Dev-dependency imported nowhere under `test/`, `tool/`, `integration_test/`. |
| `no-deep-package-imports` | correctness | needs-cross-file/project | A | Importing another package's internal file past its public barrel. |
| `duplicate-barrel-exports` | suspicious | needs-cross-file/project | A | Same symbol re-exported by two barrels → ambiguous import. |
