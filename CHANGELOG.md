# Changelog

## 0.4.3 (2026-10-20)

### Features

- Warn about unlabeled interactive gesture targets. Flutter widget checks resolve package identities.
- Require GlobalKey fields to be passed to a widget. Flutter widget checks resolve package identities.
- Named arguments follow their resolved parameter declaration order by default.
- Add class-level encapsulation checks for private members, including null-aware access, cascades and context-typed dot shorthands. Access through this/super or a receiver whose static class is the enclosing class is allowed, including inherited private members.
- Flags instance methods writing resolved static fields, including compound assignments and increments.
- Ban configured dotted annotation names, without the leading `@`. This check stays inactive until configured with the relevant match lists and diagnostic messages.
- Ban decoded export URIs, including all conditional alternatives. This check stays inactive until configured with the relevant match lists and diagnostic messages.
- Add configurable bans on declaration names, including loop variables, catch parameters, named constructors and import prefixes. References, initializing/super formals, and `@override`-annotated members and their parameters are exempt. Matching stays inactive until configured with deny patterns and diagnostic messages.
- Add configurable bans on resolved explicit type annotations, including expanded aliases and nested generic arguments in new/const constructions. Constructed class names and unknown types are exempt. Matching stays inactive until configured with deny patterns and diagnostic messages.
- Discourage files containing only re-exports, without declarations or parts.
- Add optional rule: Keep package-confirmed Bloc instance fields private; inherited API overrides are allowed.
- Add optional rule: Enforce the package-confirmed Bloc convention.
- Avoids Flutter's non-const `Border.all` factory. Flutter widget checks resolve package identities.
- Reports arithmetic expressions containing too many terms.
- Add optional correctness rule: Flags assertions whose Boolean condition is statically constant.
- Add optional correctness rule: Flags statically constant binary branch, loop, collection, and expression conditions.
- Add optional correctness rule: Flags switch statements and expressions with a statically constant subject.
- Avoid releasing late State fields from dispose: they may be unset or lazily initialized solely to release them. Flutter widget checks resolve package identities.
- Reject repeated path separators in directive URIs (import, export and part).
- Add optional correctness rule: Flags repeated field or index assignment targets within one cascade.
- Add optional correctness rule: Flags export directives whose visible names are already covered by another export.
- Add optional correctness rule: Flags factories in one type with the same implementation, ignoring factory names and alpha-renaming parameters by position.
- Add optional correctness rule: Flags final/const variables with identical initializers in the same scope.
- Add optional correctness rule: Flags repeated mixins in a with clause or a same-library superclass chain.
- Add optional correctness rule: Flags imports of the same library that differ only in their name prefix.
- Reports inline framework groups without any test registrations.
- Warns when credential-like variables or fields receive nonempty string or numeric literals. Uses conservative credential-name heuristics.
- Reports long if/else-if chains; replace them with a switch or extracted helpers.
- Flags Opacity directly wrapping a Flutter Image, including named constructors. Flutter widget checks resolve package identities.
- Reject runtime-invalid Uri constructor arguments: conflicting path/pathSegments or query/queryParameters inputs and query parameter values that are neither null, strings nor iterables of strings.
- Add optional rule: Value providers borrow existing instances; newly constructed values need create.
- Prefer direct equality comparisons and De Morgan forms in control-flow conditions. Relational operators are excluded because NaN and custom operators lack reliable inverses.
- Reports record values and record types with too many fields.
- Preserve error origins when calling SDK `Completer.completeError`.
- Require Flutter text fields to expose their updated value. Flutter widget checks resolve package identities.
- Add optional correctness rule: Require enum-keyed map literals to include every declared enum value.
- Requires semanticLabel on Flutter Image and imageSemanticLabel on FadeInImage. Flutter widget checks resolve package identities.
- Check mounted before invoking Flutter State.setState, never inside its callback. Flutter widget checks resolve package identities.
- Split chained assignments into separate statements.
- Detect writes through named function, method and constructor parameter objects, excluding closure parameters and parameters annotated `@mutated` from dart-code-metrics-annotations.
- Prefer positive conditions when both branches can be swapped, including collection and map if/else elements.
- Require assignments to be standalone statements, arrow bodies or loop operations.
- Avoid `Future<Future<T>>`, `FutureOr<Future<T>>`, and other directly nested future wrappers.
- Avoid directly nesting SDK streams and futures: `Stream<Future<T>>` and `Future<Stream<T>>` complicate consumption.
- Reports deeply nested switch expressions.
- Reports deeply nested switch statements.
- Reports deeply nested try statements.
- Exception and Error payloads should not change after construction, including fields declared by primary-constructor parameters.
- Add optional rule: Rejects direct Bloc and Cubit arguments to other blocs.
- Add optional rule: Rejects direct Bloc and Cubit arguments to Flutter widget constructors.
- Add optional rule: Keeps Flutter BuildContext arguments outside Bloc and Cubit APIs.
- Keeps notifier state behind Riverpod's observable state property.
- Reports direct self-calls in functions and methods, including guarded recursion.
- Rejects Riverpod widget references used during State.dispose.
- Keeps base classes independent from transitive `extends`/`with` subclasses, excluding implementers and mixin constraints. Exempts `freezed`, `Freezed`, and `unfreezed` annotations by default; `ignored_annotations` replaces this exact-match list.
- Accept `avoid-self-compare` as an alias for the existing `no-self-comparisons` rule; both names resolve to the same implementation.
- Detect explicit secret-bearing URL query keys, not arbitrary occurrences of words such as `token` in paths, fragments, map values, or prose.
- Finds getters hiding fields supplied via the immediate super constructor's `this.` formals or declaring parameters. Distinguishes named from positional super parameters; initializer-list assignments and intermediate forwarding are not tracked.
- Requires public declarations in test files, except main, to be private.
- Reports unconditional break, continue, return or throw in a for-loop body.
- Add an optional rule that flags standalone nested blocks introducing no lexical declarations.
- Add an optional rule that prefers direct invocation of functions and resolved callable instances.
- Avoid singleton collection literals where SDK APIs accept a single value.
- Add an optional rule that flags an unlabeled `continue` at the end of a loop body or its final branch.
- Prefer synchronous declarations when an async Future only returns scalars.
- Flags a resolved Flutter GestureDetector without any non-null event handlers. Flutter widget checks resolve package identities.
- Add an optional rule that flags getters exposing only a private final field. Upcasting getters remain allowed by default; set `ignore_upcast: false` to report them.
- Add an optional rule for length checks around `for-in` and `forEach`: empty collections already skip those bodies.
- Add an optional rule that flags a single-use local used only to initialize the next local.
- Add an optional rule that simplifies negated bool literals and equality comparisons with a logical negation on both sides.
- Add an optional rule that flags redundant Flutter `State` overrides dropping inherited `@protected` annotations.
- Add an optional rule that flags an eager local initializer immediately overwritten without a read.
- Add an optional rule to omit implicit `super()` initializers and provably redundant member prefixes.
- Reports iteration of a collection in a branch that requires it to be empty.
- Add optional correctness rule: Warn when SDK Iterable.reduce can receive an empty collection.
- Reports a null-checked local ignored by a branch that passes another local of the same resolved type to a call.
- Flags Padding directly wrapping a Flutter Container that can own the padding. Flutter widget checks resolve package identities.
- Reports function bodies with excessive nesting-weighted control flow.
- Flags enum constants whose declared names violate the configured order.
- Require Equatable semantics for configured class/file name patterns.
- Add optional correctness rule: Flags value functions with a proven explicit null return on every path.
- Add optional correctness rule: Warns when distinct return paths in a function produce the same scalar constant.
- Add optional rule: Warns when a Bloc constructor leaves a concrete event subclass without a handler.
- Requires field initializers to follow their containing type's field declaration order by default.
- Flags literal string map keys that violate the configured order.
- Detect likely copy-paste errors in trivial accessors when a different field matching the accessor name is declared.
- Reports function bodies with too many statements.
- Warn about explicitly undersized Flutter touch targets. Flutter widget checks resolve package identities.
- Move final/const scalar literals into their only branch, or after an if.
- Requires a blank line before a break that follows another statement.
- Requires a blank line between successive switch statement case clauses.
- Requires a blank line before constructors following another member.
- Requires a blank line before a continue that follows another statement.
- Requires a blank line before method declarations following another member.
- Requires a blank line before a throw that follows another statement.
- Requires named parameters to follow the explicitly configured alphabetical order.
- Flags named object and record pattern fields that violate the configured order.
- Prefer `Iterable.every` to `where(predicate).isEmpty`.
- Add optional rule: Enforce the package-confirmed Bloc convention.
- Add optional rule: Prefer context.read/watch over package:flutter_bloc's provider lookups.
- Add optional rule: Enforce the package-confirmed Bloc convention.
- Accept `prefer-boolean-prefixes` as an alias for the existing `boolean-prefixes` rule; both names resolve to the same implementation.
- Require explanations for analyzer `ignore:` and `ignore_for_file:` comments.
- Match callback field names to a configured regular expression.
- Require configured error parameter names in catches and SDK error callbacks.
- Match callback handler methods to a configured regular expression.
- Decode JSON containers using their actual runtime generic types.
- Requires snake_case file names ending in _notifier.dart for resolved Riverpod notifier classes.
- Requires snake_case file names ending in _provider.dart for resolved Riverpod provider declarations, including functions annotated with riverpod_annotation's riverpod or Riverpod.
- Requires entry-point files under a test directory to use the _test.dart suffix.
- Require an explicit heroTag on Flutter FloatingActionButton constructors. Flutter widget checks resolve package identities.
- Prefer EquatableMixin to directly extending package:equatable's Equatable.
- Requires expectLater when a canonical expect call receives an unawaited Future.
- Prefer getters for synchronous, zero-argument methods that only read a field.
- Requires stable value equality for fresh Riverpod family arguments, rejecting non-constant collections, closures, and newly constructed classes without a resolved equality override. Accepts constants, existing values, and records whose components are acceptable.
- Match a file's sole class or enum name to its snake-case basename.
- Add optional rule: Flatten directly nested package:flutter_bloc BlocProviders with MultiBlocProvider.
- Add optional rule: Prefer a flat MultiProvider to directly nested provider widgets.
- Prefer named parameters when a declaration exceeds `max_number` (default 1).
- Add optional rule: Prefer context.read/watch when provider extensions are in scope.
- Prefer the cryptographically secure `dart:math` random generator.
- Await returned SDK Futures inside a try body protected by a catch clause.
- Requires a `Notifier` suffix on classes extending resolved Riverpod notifiers, excluding generated Dart files.
- Requires a Provider suffix on resolved Riverpod top-level providers and static fields.
- Merge consecutive Flutter State.setState statements in the same block. Flutter widget checks resolve package identities.
- Prefers SizedBox.square for an unnamed Flutter SizedBox whose explicit width and height are stable variable reads or numeric literals with identical token spelling. Flutter widget checks resolve package identities.
- Recommends Matcher expectations and direct type, equality, or ordering matchers.
- Prefer explicit types over `var` in ordinary variable declarations.
- Reports duplicate test descriptions within a group and constant names in loops.
- Prefer `useCallback` for synchronous memoized function literals.
- Prefer memoizing freshly allocated collection values used as hook keys.
- Flags SingleChildScrollView directly containing a Flutter Column with no arguments other than children. Flutter widget checks resolve package identities.
- Prefers Flutter's `VoidCallback` alias over a nongeneric `void Function()` type. Flutter widget checks resolve package identities.
- Flags named record type fields that violate the configured order.
- Add an optional rule to reuse or extend existing record destructuring instead of rereading a field.
- Add an optional rule to reuse a local for stable SDK arithmetic and `String.length`/`int` parity getters.
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
- Limits the depth of an inline Flutter widget construction tree.

#### Add configurable bans on resolved API references, matching top-level names or `DeclaringType.member`. Covers calls, tear-offs, properties, cascades, context-typed dot shorthands, constructor invocations, initializers and redirecting factories. Lexical declarations take precedence over inherited members; unresolved targets are ignored. Matching stays inactive until configured with deny patterns and diagnostic messages.

Unqualified instance and static fields remain checked unless shadowed by a local declaration. Named extensions resolve their own members without consulting the on-type. Property reads and writes use their respective declaring owners, including extension setters. Compound updates prefer matching setter bans and fall back to getter bans, preserving all configured messages for the selected owner.

Cascade assignment targets resolve only against the cascade receiver; assigned values retain normal lexical resolution.

#### Extract inline string literals into named variables or constants. Invocation exemptions apply only to direct arguments, interpolation expressions are checked, and `allow_only_once` counts exempt literals too.

Count string literal patterns for `allow_only_once` while keeping pattern literals and annotations exempt from diagnostics. Directive URIs and conditional-import configuration strings remain excluded from occurrence counts.

### Fixes

- Recognize dart:math Random and dart:convert json and JsonDecoder without loaded SDK sources, and flag JSON container casts with non-nullable Object elements or values.
- Detect percent-encoded sensitive keys in URL literals and SDK `Uri` literal queries while leaving `queryParameters` map keys unchanged.
- Recognize Tooltip rich messages and Semantics attributed labels in `accessible-widget-labels`, so labeled gesture targets are not reported as unlabeled.
- Recognize Flutter callbacks that enable active gestures in `accessible-widget-labels` and `minimum-touch-target`, including secondary and tertiary long presses, force presses, and drag cancellation. Match InkWell and InkResponse's separate enablement rules, so cancellation-only ink controls remain inactive. Unwired tap-movement callbacks, literal-null callbacks, and hover, focus, and highlight notifications don't enable gestures.
- Fix the generated configuration schema's rule ordering for `prefer-define-hero-tag` and keep its metadata aligned with the docs. Remove the shipped rules from the v1 checklist and update its totals. Register the new GlobalKey and late-field semantic corpus fixtures for SDK-backed analysis. This changes configuration, test registration and documentation, not rule behavior.
- Skip explicitly disabled Flutter buttons in `minimum-touch-target` when `onPressed` is literal null and `onLongPress` is absent or literal null. Dynamic callbacks and active long presses still need a touch target.
- Preserve GestureDetectors with explicit hit-test behavior, and detect tap-only wrappers around resolved Flutter TextButton, ElevatedButton, OutlinedButton and IconButton constructors with inline onPressed callbacks. Keep distinct gestures and nullable child handlers unflagged, and direct tap handling to the button callback.
- Detect hardcoded credential values in optional parameter defaults, including functions, methods, constructors and closures. Reuse the existing credential-name, literal and storage-key checks.
- Report nonconstant GlobalKey constructors used directly as widget key arguments, including prefixed constructors and type aliases. Preserve shadowed names, constant keys, value-stable GlobalObjectKeys and cached widgets in field or top-level initializers, while still checking callbacks and local variables.
- Fix string interpolation parsing. `'${await x}'` inside an async function was dropped from the syntax tree, so anything used only there looked unused (for example to `avoid-unused-parameters`). An escaped brace such as `${a\}b}` is now rejected with a parse error, matching Dart.
- Exclude Padding wrappers and child Containers with non-null keys from avoid-wrapping-in-padding because moving the padding changes the keyed element's bounds. Literal-null keys still allow the diagnostic.
- Allow late State fields to be released in dispose when synchronous initState initializes them through a direct assignment or directly reads a lazy initializer in its initial straight-line expression and declaration statements. Conditional, deferred and uninitialized fields still produce diagnostics, with guidance to initialize them unconditionally or make them nullable.
- Clarify that replacing a scrollable Column with ListView requires preserving child alignment and padding explicitly, including ListView's default MediaQuery padding. Keep this as advice without an automatic fix, rather than promising a layout-neutral replacement.
- Only suggest ListView for a scrollable Column when its children list contains at least two unconditional children. Skip absent, empty, single-child and dynamic lists, and do not count spread or control-flow elements toward that minimum. Preserve the existing alignment and padding advice without offering an automatic fix.
- Avoid suggesting removal of `super` when a library-scope binding may shadow the inherited member.
- Count blank lines above attached comments in `newline-before-return`, avoiding diagnostics on already separated return statements.
- Limit `prefer-correct-test-file-name` to package-root `test/` and `integration_test/` directories so Flutter driver entrypoints and helpers under `test_driver/` or `testing/` don't require the `_test.dart` suffix.
- falcon can be installed with `dart pub global activate falcon`.
- Use valid external factory declarations for staged SDK Random constructors in `prefer-random-secure` fixtures.
- Ignore Dart interpolation source punctuation when detecting sensitive URL query parameters, so embedded `#` or `?` characters do not hide static secret-bearing keys and `&token=` inside an expression does not trigger a false warning.
- Share the active gesture callback check between `accessible-widget-labels` and `minimum-touch-target` so both rules keep the same callback and literal-null policy.
- Detect sensitive keys in literal `query` arguments to SDK `Uri` constructors, including interpolated values, without flagging shadowed constructors or unrelated query calls.
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
