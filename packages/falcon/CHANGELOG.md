# Changelog

## 0.4.3 (2026-10-20)

### Features

- falcon can be installed with `dart pub global activate falcon`.
- `accessible-widget-labels`: Warn about unlabeled interactive gesture targets. Flutter widget checks resolve package identities. Recognize Tooltip rich messages and Semantics attributed labels. Share the active gesture callback and literal-null policy with `minimum-touch-target`, including secondary and tertiary long presses, force presses, and drag cancellation. Match InkWell and InkResponse's separate enablement rules, so cancellation-only ink controls remain inactive. Unwired tap-movement callbacks, literal-null callbacks, and hover, focus, and highlight notifications don't enable gestures.
- `always-pass-global-key`: Require GlobalKey fields to be passed to a widget. Flutter widget checks resolve package identities. Report nonconstant GlobalKey constructors used directly as widget key arguments, including prefixed constructors and type aliases. Preserve shadowed names, constant keys, value-stable GlobalObjectKeys and cached widgets in field or top-level initializers, while still checking callbacks and local variables.
- `arguments-ordering`: Named arguments follow their resolved parameter declaration order by default.
- `avoid-accessing-other-classes-private-members`: Add class-level encapsulation checks for private members, including null-aware access, cascades and context-typed dot shorthands. Access through this/super or a receiver whose static class is the enclosing class is allowed, including inherited private members.
- `avoid-assigning-to-static-field`: Flags instance methods writing resolved static fields, including compound assignments and increments.
- `avoid-banned-annotations`: Ban configured dotted annotation names, without the leading `@`. This check stays inactive until configured with the relevant match lists and diagnostic messages.
- `avoid-banned-exports`: Ban decoded export URIs, including all conditional alternatives. This check stays inactive until configured with the relevant match lists and diagnostic messages.
- `avoid-banned-names`: Add configurable bans on declaration names, including loop variables, catch parameters, named constructors and import prefixes. References, initializing/super formals, and `@override`-annotated members and their parameters are exempt. Matching stays inactive until configured with deny patterns and diagnostic messages.
- `avoid-banned-types`: Add configurable bans on resolved explicit type annotations, including expanded aliases and nested generic arguments in new/const constructions. Constructed class names and unknown types are exempt. Matching stays inactive until configured with deny patterns and diagnostic messages.
- `avoid-barrel-files`: Discourage files containing only re-exports, without declarations or parts.
- `avoid-bloc-public-fields`: Add optional rule: Keep package-confirmed Bloc instance fields private; inherited API overrides are allowed.
- `avoid-bloc-public-methods`: Add optional rule: Enforce the package-confirmed Bloc convention.
- `avoid-border-all`: Avoids Flutter's non-const `Border.all` factory. Flutter widget checks resolve package identities.
- `avoid-complex-arithmetic-expressions`: Reports arithmetic expressions containing too many terms.
- `avoid-constant-assert-conditions`: Add optional correctness rule: Flags assertions whose Boolean condition is statically constant.
- `avoid-constant-conditions`: Add optional correctness rule: Flags statically constant binary branch, loop, collection, and expression conditions.
- `avoid-constant-switches`: Add optional correctness rule: Flags switch statements and expressions with a statically constant subject.
- `avoid-disposing-late-fields`: Avoid releasing late State fields from dispose: they may be unset or lazily initialized solely to release them. Flutter widget checks resolve package identities. Allow release when synchronous initState initializes them through a direct assignment or directly reads a lazy initializer in its initial straight-line expression and declaration statements. Conditional, deferred and uninitialized fields still produce diagnostics, with guidance to initialize them unconditionally or make them nullable.
- `avoid-double-slash-imports`: Reject repeated path separators in directive URIs (import, export and part).
- `avoid-duplicate-cascades`: Add optional correctness rule: Flags repeated field or index assignment targets within one cascade.
- `avoid-duplicate-exports`: Add optional correctness rule: Flags export directives whose visible names are already covered by another export.
- `avoid-duplicate-factories`: Add optional correctness rule: Flags factories in one type with the same implementation, ignoring factory names and alpha-renaming parameters by position.
- `avoid-duplicate-initializers`: Add optional correctness rule: Flags final/const variables with identical initializers in the same scope.
- `avoid-duplicate-mixins`: Add optional correctness rule: Flags repeated mixins in a with clause or a same-library superclass chain.
- `avoid-duplicate-named-imports`: Add optional correctness rule: Flags imports of the same library that differ only in their name prefix.
- `avoid-empty-test-groups`: Reports inline framework groups without any test registrations.
- `avoid-hardcoded-credentials`: Warns when credential-like variables or fields receive nonempty string or numeric literals. Uses conservative credential-name heuristics. Includes optional parameter defaults in functions, methods, constructors and closures, using the same credential-name, literal and storage-key checks.
- `avoid-if-with-many-branches`: Reports long if/else-if chains; replace them with a switch or extracted helpers.
- `avoid-incorrect-image-opacity`: Flags Opacity directly wrapping a Flutter Image, including named constructors. Flutter widget checks resolve package identities.
- `avoid-incorrect-uri`: Reject runtime-invalid Uri constructor arguments: conflicting path/pathSegments or query/queryParameters inputs and query parameter values that are neither null, strings nor iterables of strings.
- `avoid-instantiating-in-value-provider`: Add optional rule: Value providers borrow existing instances; newly constructed values need create.
- `avoid-inverted-boolean-checks`: Prefer direct equality comparisons and De Morgan forms in control-flow conditions. Relational operators are excluded because NaN and custom operators lack reliable inverses.
- `avoid-long-records`: Reports record values and record types with too many fields.
- `avoid-missing-completer-stack-trace`: Preserve error origins when calling SDK `Completer.completeError`.
- `avoid-missing-controller`: Require Flutter text fields to expose their updated value. Flutter widget checks resolve package identities.
- `avoid-missing-enum-constant-in-map`: Add optional correctness rule: Require enum-keyed map literals to include every declared enum value.
- `avoid-missing-image-alt`: Requires semanticLabel on Flutter Image and imageSemanticLabel on FadeInImage. Flutter widget checks resolve package identities.
- `avoid-mounted-in-setstate`: Check mounted before invoking Flutter State.setState, never inside its callback. Flutter widget checks resolve package identities.
- `avoid-multi-assignment`: Split chained assignments into separate statements.
- `avoid-mutating-parameters`: Detect writes through named function, method and constructor parameter objects, excluding closure parameters and parameters annotated `@mutated` from dart-code-metrics-annotations.
- `avoid-negated-conditions`: Prefer positive conditions when both branches can be swapped, including collection and map if/else elements.
- `avoid-nested-assignments`: Require assignments to be standalone statements, arrow bodies or loop operations.
- `avoid-nested-futures`: Avoid `Future<Future<T>>`, `FutureOr<Future<T>>`, and other directly nested future wrappers.
- `avoid-nested-streams-and-futures`: Avoid directly nesting SDK streams and futures: `Stream<Future<T>>` and `Future<Stream<T>>` complicate consumption.
- `avoid-nested-switch-expressions`: Reports deeply nested switch expressions.
- `avoid-nested-switches`: Reports deeply nested switch statements.
- `avoid-nested-try-statements`: Reports deeply nested try statements.
- `avoid-non-final-exception-class-fields`: Exception and Error payloads should not change after construction, including fields declared by primary-constructor parameters.
- `avoid-passing-bloc-to-bloc`: Add optional rule: Rejects direct Bloc and Cubit arguments to other blocs.
- `avoid-passing-bloc-to-widget`: Add optional rule: Rejects direct Bloc and Cubit arguments to Flutter widget constructors.
- `avoid-passing-build-context-to-blocs`: Add optional rule: Keeps Flutter BuildContext arguments outside Bloc and Cubit APIs.
- `avoid-public-notifier-properties`: Keeps notifier state behind Riverpod's observable state property.
- `avoid-recursive-calls`: Reports direct self-calls in functions and methods, including guarded recursion.
- `avoid-ref-inside-state-dispose`: Rejects Riverpod widget references used during State.dispose.
- `avoid-referencing-subclasses`: Keeps base classes independent from transitive `extends`/`with` subclasses, excluding implementers and mixin constraints. Exempts `freezed`, `Freezed`, and `unfreezed` annotations by default; `ignored_annotations` replaces this exact-match list.
- `avoid-self-compare`: Accept `avoid-self-compare` as an alias for the existing `no-self-comparisons` rule; both names resolve to the same implementation.
- `avoid-sensitive-query-params`: Detect explicit secret-bearing URL query keys, not arbitrary occurrences of words such as `token` in paths, fragments, map values, or prose. Detect percent-encoded sensitive keys in URL literals and SDK `Uri` literal queries while leaving `queryParameters` map keys unchanged. Ignore Dart interpolation source punctuation, so embedded `#` or `?` characters do not hide static secret-bearing keys and `&token=` inside an expression does not trigger a false warning. Include literal `query` arguments to SDK `Uri` constructors, including interpolated values, without flagging shadowed constructors or unrelated query calls.
- `avoid-suspicious-super-overrides`: Finds getters hiding fields supplied via the immediate super constructor's `this.` formals or declaring parameters. Distinguishes named from positional super parameters; initializer-list assignments and intermediate forwarding are not tracked.
- `avoid-top-level-members-in-tests`: Requires public declarations in test files, except main, to be private.
- `avoid-unconditional-break`: Reports unconditional break, continue, return or throw in a for-loop body.
- `avoid-unnecessary-block`: Add an optional rule that flags standalone nested blocks introducing no lexical declarations.
- `avoid-unnecessary-call`: Add an optional rule that prefers direct invocation of functions and resolved callable instances.
- `avoid-unnecessary-collections`: Avoid singleton collection literals where SDK APIs accept a single value.
- `avoid-unnecessary-continue`: Add an optional rule that flags an unlabeled `continue` at the end of a loop body or its final branch.
- `avoid-unnecessary-futures`: Prefer synchronous declarations when an async Future only returns scalars.
- `avoid-unnecessary-gesture-detector`: Flags a resolved Flutter GestureDetector without any non-null event handlers. Flutter widget checks resolve package identities. Preserve GestureDetectors with explicit hit-test behavior, and detect tap-only wrappers around resolved Flutter TextButton, ElevatedButton, OutlinedButton and IconButton constructors with inline onPressed callbacks. Keep distinct gestures and nullable child handlers unflagged, and direct tap handling to the button callback.
- `avoid-unnecessary-getter`: Add an optional rule that flags getters exposing only a private final field. Upcasting getters remain allowed by default; set `ignore_upcast: false` to report them.
- `avoid-unnecessary-length-check`: Add an optional rule for length checks around `for-in` and `forEach`: empty collections already skip those bodies.
- `avoid-unnecessary-local-variable`: Add an optional rule that flags a single-use local used only to initialize the next local.
- `avoid-unnecessary-negations`: Add an optional rule that simplifies negated bool literals and equality comparisons with a logical negation on both sides.
- `avoid-unnecessary-overrides-in-state`: Add an optional rule that flags redundant Flutter `State` overrides dropping inherited `@protected` annotations.
- `avoid-unnecessary-reassignment`: Add an optional rule that flags an eager local initializer immediately overwritten without a read.
- `avoid-unnecessary-super`: Add an optional rule to omit implicit `super()` initializers and provably redundant member prefixes. Avoid suggesting removal of `super` when a library-scope binding may shadow the inherited member.
- `avoid-unreachable-for-loop`: Reports iteration of a collection in a branch that requires it to be empty.
- `avoid-unsafe-reduce`: Add optional correctness rule: Warn when SDK Iterable.reduce can receive an empty collection.
- `avoid-unused-after-null-check`: Reports a null-checked local ignored by a branch that passes another local of the same resolved type to a call.
- `avoid-wrapping-in-padding`: Flags Padding directly wrapping a Flutter Container that can own the padding. Flutter widget checks resolve package identities. Exclude Padding wrappers and child Containers with non-null keys because moving the padding changes the keyed element's bounds. Literal-null keys still allow the diagnostic.
- `cognitive-complexity`: Reports function bodies with excessive nesting-weighted control flow.
- `enum-constants-ordering`: Flags enum constants whose declared names violate the configured order.
- `extend-equatable`: Require Equatable semantics for configured class/file name patterns.
- `function-always-returns-null`: Add optional correctness rule: Flags value functions with a proven explicit null return on every path.
- `function-always-returns-same-value`: Add optional correctness rule: Warns when distinct return paths in a function produce the same scalar constant.
- `handle-bloc-event-subclasses`: Add optional rule: Warns when a Bloc constructor leaves a concrete event subclass without a handler.
- `initializers-ordering`: Requires field initializers to follow their containing type's field declaration order by default.
- `map-keys-ordering`: Flags literal string map keys that violate the configured order.
- `match-getter-setter-field-names`: Detect likely copy-paste errors in trivial accessors when a different field matching the accessor name is declared.
- `max-statements`: Reports function bodies with too many statements.
- `minimum-touch-target`: Warn about explicitly undersized Flutter touch targets. Flutter widget checks resolve package identities. Skip explicitly disabled Flutter buttons when `onPressed` is literal null and `onLongPress` is absent or literal null. Dynamic callbacks and active long presses still need a touch target. Share the active gesture callback and literal-null policy described under `accessible-widget-labels`, including InkWell and InkResponse's separate enablement rules.
- `move-variable-closer-to-its-usage`: Move final/const scalar literals into their only branch, or after an if.
- `newline-before-break`: Requires a blank line before a break that follows another statement.
- `newline-before-case`: Requires a blank line between successive switch statement case clauses.
- `newline-before-constructor`: Requires a blank line before constructors following another member.
- `newline-before-continue`: Requires a blank line before a continue that follows another statement.
- `newline-before-method`: Requires a blank line before method declarations following another member.
- `newline-before-throw`: Requires a blank line before a throw that follows another statement.
- `parameters-ordering`: Requires named parameters to follow the explicitly configured alphabetical order.
- `pattern-fields-ordering`: Flags named object and record pattern fields that violate the configured order.
- `prefer-any-or-every`: Prefer `Iterable.every` to `where(predicate).isEmpty`.
- `prefer-bloc-event-suffix`: Add optional rule: Enforce the package-confirmed Bloc convention.
- `prefer-bloc-extensions`: Add optional rule: Prefer context.read/watch over package:flutter_bloc's provider lookups.
- `prefer-bloc-state-suffix`: Add optional rule: Enforce the package-confirmed Bloc convention.
- `prefer-boolean-prefixes`: Accept `prefer-boolean-prefixes` as an alias for the existing `boolean-prefixes` rule; both names resolve to the same implementation.
- `prefer-commenting-analyzer-ignores`: Require explanations for analyzer `ignore:` and `ignore_for_file:` comments.
- `prefer-correct-callback-field-name`: Match callback field names to a configured regular expression.
- `prefer-correct-error-name`: Require configured error parameter names in catches and SDK error callbacks.
- `prefer-correct-handler-name`: Match callback handler methods to a configured regular expression.
- `prefer-correct-json-casts`: Decode JSON containers using their actual runtime generic types. Recognize dart:convert json and JsonDecoder without loaded SDK sources, and flag JSON container casts with non-nullable Object elements or values.
- `prefer-correct-notifier-file-name`: Requires snake_case file names ending in _notifier.dart for resolved Riverpod notifier classes.
- `prefer-correct-provider-file-name`: Requires snake_case file names ending in _provider.dart for resolved Riverpod provider declarations, including functions annotated with riverpod_annotation's riverpod or Riverpod.
- `prefer-correct-test-file-name`: Requires entry-point files under package-root `test/` and `integration_test/` directories to use the _test.dart suffix. Flutter driver entrypoints and helpers under `test_driver/` or `testing/` do not require that suffix.
- `prefer-define-hero-tag`: Require an explicit heroTag on Flutter FloatingActionButton constructors. Flutter widget checks resolve package identities.
- `prefer-equatable-mixin`: Prefer EquatableMixin to directly extending package:equatable's Equatable.
- `prefer-expect-later`: Requires expectLater when a canonical expect call receives an unawaited Future.
- `prefer-getter-over-method`: Prefer getters for synchronous, zero-argument methods that only read a field.
- `prefer-immutable-provider-arguments`: Requires stable value equality for fresh Riverpod family arguments, rejecting non-constant collections, closures, and newly constructed classes without a resolved equality override. Accepts constants, existing values, and records whose components are acceptable.
- `prefer-match-file-name`: Match a file's sole class or enum name to its snake-case basename.
- `prefer-multi-bloc-provider`: Add optional rule: Flatten directly nested package:flutter_bloc BlocProviders with MultiBlocProvider.
- `prefer-multi-provider`: Add optional rule: Prefer a flat MultiProvider to directly nested provider widgets.
- `prefer-named-parameters`: Prefer named parameters when a declaration exceeds `max_number` (default 1).
- `prefer-provider-extensions`: Add optional rule: Prefer context.read/watch when provider extensions are in scope.
- `prefer-random-secure`: Prefer the cryptographically secure `dart:math` random generator. Recognize dart:math Random without loaded SDK sources. Use valid external factory declarations for staged SDK Random constructors in fixtures.
- `prefer-return-await`: Await returned SDK Futures inside a try body protected by a catch clause.
- `prefer-riverpod-notifier-suffix`: Requires a `Notifier` suffix on classes extending resolved Riverpod notifiers, excluding generated Dart files.
- `prefer-riverpod-provider-suffix`: Requires a Provider suffix on resolved Riverpod top-level providers and static fields.
- `prefer-single-setstate`: Merge consecutive Flutter State.setState statements in the same block. Flutter widget checks resolve package identities.
- `prefer-sized-box-square`: Prefers SizedBox.square for an unnamed Flutter SizedBox whose explicit width and height are stable variable reads or numeric literals with identical token spelling. Flutter widget checks resolve package identities.
- `prefer-test-matchers`: Recommends Matcher expectations and direct type, equality, or ordering matchers.
- `prefer-type-over-var`: Prefer explicit types over `var` in ordinary variable declarations.
- `prefer-unique-test-names`: Reports duplicate test descriptions within a group and constant names in loops.
- `prefer-use-callback`: Prefer `useCallback` for synchronous memoized function literals.
- `prefer-use-memo`: Prefer memoizing freshly allocated collection values used as hook keys.
- `prefer-using-list-view`: Flags SingleChildScrollView directly containing a Flutter Column with no arguments other than children. Flutter widget checks resolve package identities. Replacing a scrollable Column with ListView requires preserving child alignment and padding explicitly, including ListView's default MediaQuery padding. Keep this as advice without an automatic fix, rather than promising a layout-neutral replacement. Only suggest ListView when its children list contains at least two unconditional children. Skip absent, empty, single-child and dynamic lists, and do not count spread or control-flow elements toward that minimum.
- `prefer-void-callback`: Prefers Flutter's `VoidCallback` alias over a nongeneric `void Function()` type. Flutter widget checks resolve package identities.
- `record-fields-ordering`: Flags named record type fields that violate the configured order.
- `use-existing-destructuring`: Add an optional rule to reuse or extend existing record destructuring instead of rereading a field.
- `use-existing-variable`: Add an optional rule to reuse a local for stable SDK arithmetic and `String.length`/`int` parity getters.
- Add the `bloc` rules `emit-new-bloc-state-instances`, `prefer-sealed-bloc-events`, `prefer-sealed-bloc-state`, `prefer-immutable-bloc-events`, `prefer-immutable-bloc-state`, `avoid-duplicate-bloc-event-handlers`, `check-is-not-closed-after-async-gap` and `avoid-emit-outside-bloc`. This completes the must-have rules in the 1.0 checklist.
- Add `avoid-complex-conditions` and `avoid-complex-loop-conditions` (both take a `threshold` option), `avoid-collapsible-if`, `avoid-slow-collection-methods` and `move-variable-outside-iteration`.
- Add `avoid-duplicate-map-keys`, `avoid-duplicate-collection-elements`, `avoid-duplicate-switch-case-conditions`, `avoid-missing-interpolation`, `avoid-contradictory-expressions` and `avoid-unmodified-loop-condition`. All but `avoid-duplicate-collection-elements` are recommended, since repeated list values are often intentional.
- Add the cross-file rules `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles` and `banned-imports`, all opt-in. When `undeclared-dependencies` is enabled it takes over the cases it shares with `depend-on-referenced-packages`. `banned-imports` takes `files`/`deny` glob rules; see the configuration docs.
- Add `avoid-undisposed-instances`, `dispose-focus-node-and-timer`, `dispose-fields`, `always-remove-listener`, `avoid-recursive-widget-calls` and `avoid-unnecessary-stateful-widgets`. A disposable passed to another call counts as handed over, so `Navigator.push(MaterialPageRoute(...))` isn't flagged.
- Add `test`, `bloc`, `riverpod`, `provider`, `flutter_hooks` and `equatable` to the `linter.domains` config. They gate the package-specific rules that follow, so each can be set to `all`, `recommended` or `none` like `flutter`.
- Add the Flutter lifecycle rules `use-setstate-synchronously`, `require-super-dispose`, `avoid-empty-setstate`, `avoid-unnecessary-setstate`, `use-closest-build-context`, `avoid-late-context` and `avoid-inherited-widget-in-initstate`. `use-setstate-synchronously` shares the async-gap analysis behind `use-build-context-synchronously`.
- Add the `flutter_hooks` rules `avoid-conditional-hooks`, `avoid-hooks-outside-build`, `avoid-misused-hooks` and `avoid-unnecessary-hook-widgets`.
- Add the `provider` rules `avoid-read-inside-build`, `avoid-watch-outside-build`, `dispose-providers` and `prefer-read-for-methods`.
- Add the `riverpod` rules `use-ref-read-synchronously`, `use-ref-and-state-synchronously`, `dispose-provided-instances` and `dispose-provider-resources`.
- Add the `riverpod` rules `avoid-ref-watch-outside-build`, `avoid-ref-read-inside-build`, `prefer-ref-read-for-methods`, `avoid-calling-notifier-members-inside-build` and `avoid-unnecessary-consumer-widgets`.
- Add `prefer-early-return`, `prefer-returning-condition`, `prefer-switch-expression`, `prefer-for-in` and `prefer-named-boolean-parameters`. `prefer-named-boolean-parameters` covers the private functions and closures that `avoid-positional-boolean-parameters` skips, so enabling both covers every positional `bool`.
- Add `avoid-commented-out-code`, `avoid-shadowing`, `avoid-shadowed-extension-methods`, `avoid-assignments-as-conditions`, `avoid-bitwise-operators-with-booleans` and `cascade-after-null-coalescing`.
- Add `avoid-duplicate-test-assertions` and `missing-test-assertion` for `*_test.dart` files, and `list-all-equatable-fields`, `add-equatable-props` and `avoid-mutable-equatable-subclass-fields` for `package:equatable` classes.
- Add `avoid-collection-equality-checks`, `avoid-unrelated-type-casts`, `avoid-unsafe-collection-methods`, `avoid-uncaught-future-errors` and the first `security` rule, `avoid-weak-cryptographic-algorithms`. All but `avoid-unsafe-collection-methods` are recommended, since it flags every `.first` on a list that might be empty.
- Add `avoid-unnecessary-if`, `avoid-unnecessary-return` and `avoid-redundant-else`. `avoid-unnecessary-conditionals` is accepted as an alias of `avoid-bool-literals-in-conditional-expressions`, which already covers it.
- Add the Flutter widget rules `avoid-shrink-wrap-in-lists`, `prefer-align-over-container`, `prefer-center-over-align`, `prefer-padding-over-container`, `prefer-constrained-box-over-container`, `prefer-transform-over-container`, `prefer-container` (with a `min_sequence` option) and `prefer-for-loop-in-children`, plus the first `a11y` rule, `prefer-action-button-tooltip`.
- `widget-tree-depth`: Limits the depth of an inline Flutter widget construction tree.

#### `banned-usage`: Add configurable bans on resolved API references, matching top-level names or `DeclaringType.member`. Covers calls, tear-offs, properties, cascades, context-typed dot shorthands, constructor invocations, initializers and redirecting factories. Lexical declarations take precedence over inherited members; unresolved targets are ignored. Matching stays inactive until configured with deny patterns and diagnostic messages.

Unqualified instance and static fields remain checked unless shadowed by a local declaration. Named extensions resolve their own members without consulting the on-type. Property reads and writes use their respective declaring owners, including extension setters. Compound updates prefer matching setter bans and fall back to getter bans, preserving all configured messages for the selected owner.

Cascade assignment targets resolve only against the cascade receiver; assigned values retain normal lexical resolution.

#### `no-magic-string`: Extract inline string literals into named variables or constants. Invocation exemptions apply only to direct arguments, interpolation expressions are checked, and `allow_only_once` counts exempt literals too.

Count string literal patterns for `allow_only_once` while keeping pattern literals and annotations exempt from diagnostics. Directive URIs and conditional-import configuration strings remain excluded from occurrence counts.

### Fixes

- Fix string interpolation parsing. `'${await x}'` inside an async function was dropped from the syntax tree, so anything used only there looked unused (for example to `avoid-unused-parameters`). An escaped brace such as `${a\\}b}` is now rejected with a parse error, matching Dart.
- `newline-before-return`: Count blank lines above attached comments, avoiding diagnostics on already separated return statements.
- Parse switch-expression guards with a parenthesized operand, like `when (a = b) => 1`, as guards. They were read as closures, which broke the arm and anything checking the guard.

## 0.4.2 (2026-10-17)

### Breaking Changes

- Prebuilt macOS binaries now require macOS 11.7.1 or newer.

### Features

- Release downloads now include `SHA256SUMS` and GitHub build provenance attestations, with instructions for verifying both.
- Add Neovim LSP setup instructions.

### Fixes

- Resolve `package:` imports in each package's dependency context, so workspaces containing different versions of the same dependency no longer share the wrong library identities or produce false positives.
- Recognize Flutter and meta declarations across multiple dependency copies, and cache import path canonicalization to reduce analysis time.
- Fix package discovery, language versions, project membership, and cross-file diagnostics when a workspace is opened through a symlink.
- Retry dependency and companion parsing after a parser thread fails to start instead of caching the failure as a completed parse.
- Publish cross-file thread-spawn failures on every open LSP document, and prevent suppression comments from hiding those failures.
- Fix the Zed extension's repository URL to point at `JacobDevelops/dart_falcon`.
- Quick-start install links now download the latest release without a version pin via `releases/latest/download/falcon-<platform>.tar.gz`, and releases include versionless platform tarballs alongside versioned archives.
- Manual re-releases now build the requested tag rather than the current main branch.

## 0.4.1 (2026-10-17)

### Fixes

- Run semantic lint rules in a shared analysis pass instead of walking the same syntax separately for each rule.
- Run syntax lint rules in a shared structural pass to reduce repeated traversal during analysis.

## 0.4.0 (2026-10-17)

### Breaking Changes

- Use kebab-case rule IDs throughout configuration and diagnostics, and merge duplicate rule variants such as `no_empty_block` and `avoid_empty_blocks` into `no-empty-block`. Registered legacy IDs still resolve in configuration and suppressions; use `falcon migrate --input falcon.json --write` to rewrite them.
- Rename the top-level `project` configuration section to `cross-file`. Existing `project` configuration and suppression paths remain accepted as legacy aliases; `falcon migrate` rewrites the configuration.
- Choose falcon's own recommended set of 165 rules out of 290 shipped rules instead of following upstream presets. `unused-files`, `unused-code`, and `unnecessary-nullable` are now opt-in; `depend-on-referenced-packages`, `package-names`, and `secure-pubspec-urls` are recommended.
- Add `a11y` and `security` rule groups, and move `secure-pubspec-urls` from `suspicious` to `security`. Update its configuration and suppression paths to use the new group.
- Remove the SDK-retired `unsafe-html`, `package-prefixed-library-names`, and `package-api-docs` rules.
- Always analyze files in parallel and remove `--parallel`. Use `RAYON_NUM_THREADS` to limit workers; diagnostic ordering remains deterministic.
- Count constructors, setters, operators, closures, local functions, and function types in `max-parameters-for-function`, including constructors and `copyWith` methods that were previously exempt.

### Features

- Add the official `package:lints` and `package:flutter_lints` preset rules, plus 120 additional official Dart lints covering naming, documentation, declarations, collection idioms, nullability, and redundant syntax.
- Extend `falcon migrate` to read both list and map forms of `linter.rules` in analysis options.
- Add Dart 3.9 syntax support, including dot shorthands and digit separators, plus function type tests, contextual keyword identifiers, and adjacent string literals in constant patterns.
- Resolve lint checks using declaration identity, lexical scope, inferred and contextual types, constants, external libraries, and Flutter BuildContext flow rather than name-only heuristics.
- Implement `depend-on-referenced-packages`, `package-names`, and `secure-pubspec-urls`, with manifest diagnostics pointing into `pubspec.yaml`.
- Implement `prefer-contains` for resolved String and List membership tests, and improve const-constructor checks across inheritance and redirecting constructors.
- Add a searchable documentation site and rule catalog.

### Fixes

- Report parse failures as `syntax-error` diagnostics in the CLI and LSP instead of treating invalid files as clean, and print real one-based line and column positions in text output.
- Reject malformed `falcon.json` files instead of silently using defaults, warn about unknown top-level keys, and reject `max-errors: 0`.
- Stop following directory symlinks during CLI and LSP workspace collection so Flutter plugin symlinks do not pull the pub cache into analysis.
- Detect unreachable groups of files in `unused-files` using entrypoint reachability, while honoring conditional imports and preserving public barrel files.
- Traverse patterns, labeled statements, record and collection literals, extension types, switch expressions, and nested closures consistently so rules do not miss code or misreport unused parameters.
- Fix type-based checks for casts, type assertions, boolean comparisons, ignored return values, overrides, const constructors, and Flutter widget identities, including locally declared types with Flutter-like names.
- Align official Dart lint diagnostics with analyzer behavior and source ranges for declaration naming, string literals and interpolation, receiver chains, return spacing, and expression rules.
- Improve inference for generic methods and getters, staged callback contexts, nullable constants, numeric division, `Never`, and const-constructor assignments.
- Honor each package's language version and analysis environment, preserve library identities across package contexts, load project dependencies, and select duplicate library copies deterministically.
- Invalidate LSP caches when library environments change and watch loaded project libraries and companion files so dependency edits refresh diagnostics.
- Use UTF-16 LSP positions, handle malformed JSON-RPC parameters without terminating the server, and return an error status for `exit` before `shutdown`.
- Handle deeply nested Dart code on large-stack workers with parser recursion guards, and report analysis thread-spawn failures instead of panicking.
- Reduce repeated analysis work through shared flow facts, on-demand override resolution, indexed line lookups, and sharded caches; use jemalloc on x86_64 GNU Linux.

## 0.3.0 (2026-07-22)

### Breaking Changes

- Replace Dart analyzer-style `// ignore:` and `// ignore_for_file:` suppressions with `// falcon-ignore <section>/<group>/<rule>: <reason>` and file-scoped `// falcon-ignore-all` comments. Reasons are mandatory, and Dart's own ignore comments no longer suppress falcon diagnostics.

### Features

- Report `malformed-suppression` warnings for missing reasons, incorrect groups, and unknown rules instead of silently ignoring invalid suppression comments.

## 0.2.1 (2026-07-22)

### Fixes

- Count same-file references as usage in `unused-code` so declarations used within their own file are not reported as unused.
- Align argument, dynamic-type, literal-formatting, magic-number, comment-formatting, nesting, and nullable-return checks with dart_code_linter and pyramid_lint behavior, including their documented exemptions.
- Restrict identifier-length and boolean-prefix checks to declarations, honor member-ordering options, and exempt constructors from parameter-count checks.
- Make `no-boolean-literal-compare` opt-in until type information is available to avoid false positives.
- Parse statement-form pattern destructuring, pattern `for-in` bindings, and null-aware collection elements.

## 0.2.0 (2026-07-22)

### Features

- Add inline `// ignore:` and file-scoped `// ignore_for_file:` suppression comments using Dart analyzer syntax.
- Add per-rule options for thresholds, member ordering, comment patterns, identifier-length exceptions, and boolean prefixes.
- Add per-path configuration overrides.
- Add project-wide `unused-files`, `unused-code`, and `unnecessary-nullable` checks under a separate top-level `project` section.
- Expand the catalog to 79 rules, including eight additional pyramid_lint rules and cyclomatic-complexity and nesting-depth metrics.

### Fixes

- Fix parsing of pattern collection elements, `is` and `as` expressions in ternaries, generic calls, function-type parameters, map comprehensions, and closures.
- Honor unused-parameter exemptions, make trailing-comma checks formatter-aware, and strip private-name prefixes before checking boolean naming conventions.

## 0.1.0 (2026-07-22)

### Features

- Initial release of `falcon`, a standalone Dart and Flutter linter written in Rust with a Dart 3 parser and 60 lint rules ported from dart_code_linter and pyramid_lint.
- Add command-line linting and diagnostics.
- Configure grouped rules, severity levels, file selection, and Flutter-domain rules through `falcon.json` and its JSON schema.
- Add an LSP server with incremental analysis caching and VS Code and Zed editor integrations.
- Publish prebuilt binaries for x86_64 and aarch64 Linux and macOS, with Nix binary and source packages.
