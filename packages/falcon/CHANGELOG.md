# Changelog

## 0.4.7 (2026-10-31)

### Features

- Implement `architecture-layer-boundaries` to flag forbidden dependencies between clean-architecture layers and feature internals within a package.
- Implement `avoid-cubits` to recommend event-driven Bloc classes instead of direct or indirect Cubit subclasses, including local inheritance when the package is unresolved.
- Implement `avoid-empty-build-when` to require an explicit `buildWhen` argument on Bloc builders and consumers.
- Implement `avoid-excessive-expressions` with documented diagnostics and regression fixtures.
- Implement `avoid-importing-entrypoint-exports` to detect own-package barrel imports even outside the analyzed file set, with configurable source-directory restriction.
- Implement `avoid-local-functions` with documented diagnostics and regression fixtures.
- Implement `avoid-missing-test-files` to require mirrored tests for source paths selected by configurable regular expressions and name the expected test path.
- Add `avoid-misused-test-matchers` to detect matchers incompatible with known actual types, distinguish sets from lists, and check `isIn` against collection element types.
- Implement `avoid-non-ascii-symbols` to flag non-ASCII characters written literally in strings while accepting escapes and excluding interpolation expressions.
- Add `avoid-not-encodable-in-to-json` to detect non-encodable return values in serialization methods, including classes from fully loaded dependency packages while leaving incomplete hierarchies alone, with configurable additional method names.
- Implement `avoid-notifier-constructors` using resolved Riverpod identities to report constructor initialization logic across notifier, async, stream, auto-dispose, family, buildless, `AnyNotifier`, and generated variants while allowing empty constructors, legacy `StateNotifier` constructors, and unrelated same-name types.
- Add `avoid-returning-value-from-cubit-methods` to keep public Cubit commands focused on observable state, including indirect subclasses when the package is unresolved.
- Add `avoid-similar-names` to detect confusing declaration names within overlapping local scopes or the same member/top-level declaration level, with bounded edit-distance comparisons, configurable similarity thresholds, score display, and ignored names.
- Implement `duplicate-barrel-exports` to flag different declarations with the same name exported by independent public barrels in the same package.
- Implement `format-test-name` with independently configurable regular expressions for `test` and `testWidgets` descriptions, resolved framework identities, constant-string evaluation, and diagnostics naming the failed pattern.
- Implement `handle-throwing-invocations` to require matching catch clauses or covering `@Throws` metadata from `dart_code_metrics_annotations`, including constant and generic thrown-type sets.
- Implement `match-class-name-pattern` with package-relative path patterns and configurable class-name regular expressions.
- Add `missing-assets` to flag literal asset paths missing from Flutter asset declarations.
- Add `no-deep-package-imports` to flag imports of dependency libraries already exported unrestricted by their public barrel.
- Add `pass-correct-accepted-type` to check invocation arguments against package-identified `@AcceptedTypes` metadata, including inherited fields and super formals, imported declarations, contextual shorthand construction, and constant generic type sets.
- Implement `prefer-async-value-when` to flag manual state-dispatch chains on resolved Riverpod async values.
- Implement `prefer-bytes-builder` to recommend efficient byte composition instead of integer-list concatenation, bulk additions, and spreads, reporting each concatenation chain once and avoiding duplicate concatenation reports inside byte spreads.
- Add `prefer-prefixed-global-constants` to enforce `allowed-prefixes` (default `k`) on top-level constants only, listing accepted prefixes in diagnostics, with `ignore-private` (default false) to exempt private constants.
- Add `prefer-single-declaration-per-file` to keep public top-level types, including class aliases, in separate files, with private, enum and resolved `package:meta` testing exclusions.
- Add `prefer-single-notifier-per-file` to keep resolved Riverpod notifier subclasses in separate files, including family, buildless, `AnyNotifier`, and legacy `StateNotifier` variants.
- Add `prefer-single-widget-per-file` to keep Flutter widget subclasses in separate files, with private and testing exclusions.
- Implement `prefer-test-structure` for framework test callbacks, enforcing ordered arrange/act/assert comments with configurable given/when/then alternatives and section-specific diagnostics.
- Add `prefer-text-rich` to recommend Flutter's `Text.rich` instead of `RichText`, resolving constructors by package identity without flagging unrelated or shadowed names.
- Implement `proper-super-calls` to enforce first or last direct-statement ordering for existing Flutter `State` lifecycle superclass calls, including State-constrained mixins, without checking `didChangeDependencies` or reporting missing calls.
- Implement `require-atomic-async-updates` to detect assignments that reuse a value read before an asynchronous suspension, excluding uncaptured local variables and parameters.
- Implement `unexported-public-api` to flag implementation declarations used across a package but missing from its public export namespace.
- Add `unused-assets` to flag Flutter asset declarations without references in analyzed Dart sources.
- Implement `unused-dev-dependencies` to flag resolved pubspec dev dependencies unused by analyzed Dart sources, excluding SDK dependencies, analysis-options includes and plugins, configured tools, resolved builders, executables, and `bin/*.dart` tools.
- Implement `unused-l10n` to flag template ARB message keys with no identifier references outside generated localization files in analyzed Dart sources.

### Fixes

- Reject configuration and package/workspace metadata larger than 16 MiB before parsing or retaining fingerprints, preventing oversized manifests from exhausting memory during CLI and editor analysis.
- `deprecated-member-use-from-same-package` now reports deprecated static members accessed through an import prefix.
- Recognize Riverpod family and buildless stream notifier bases consistently in `prefer-correct-notifier-file-name` while preserving `AnyNotifier` and legacy `StateNotifier` support.
- Recognize Riverpod family and buildless stream notifier bases consistently in `prefer-riverpod-notifier-suffix` while preserving `AnyNotifier` and legacy `StateNotifier` support.
- The `falcon.json` JSON schema now reports `recommended` as the default for each `linter.domains` entry, matching falcon's behavior.

## 0.4.6 (2026-10-28)

### Features

- Stop publishing Intel macOS binaries; macOS releases now support Apple Silicon only.
- Add baselines of existing findings with `--write-baseline [PATH]`, `--baseline PATH` and the `baseline` configuration key, matched from any subdirectory of the project.
- Support installing prebuilt releases with cargo-binstall.
- Offer quick fixes and a safe fix-all on save in the editor.
- Configure each rule's fix as `none`, `safe` or `unsafe` with the `fix` key, including in path overrides.
- Publish a Homebrew formula to `JacobDevelops/homebrew-tap` with each release.
- Add `self-update` and native version selection to the Dart pub launcher, verified against the release's `SHA256SUMS`.
- Sign and notarise macOS binaries with a Developer ID certificate.
- Sign release archives and `SHA256SUMS` with Minisign; the public key is in the installation guide.
- Add safe fixes for `unnecessary-new`, `prefer-single-quotes` and `prefer-double-quotes`.
- Add SARIF 2.1.0, GitLab Code Quality, Checkstyle XML, Sonar generic issue JSON and HTML report formats.
- Warnings no longer fail `falcon check` by default; pass `--error-on-warnings` or set `"error-on-warnings": true` to fail on them.
- Resolve imports and exports across workspace packages, so `unused-files`, `unused-code` and `no-import-cycles` cover sibling packages.
- Detect pub and Melos workspaces and apply each member package's `falcon.json` in the CLI and editor.
- Name the owning package on workspace diagnostics, with per-package summaries in text and SARIF reports.
- Add `falcon check --write` to apply safe fixes until each file stops changing, with `--unsafe` for unsafe fixes and `--dry-run` to preview a diff.

### Fixes

- Discover configuration from the current directory the same way whether or not input paths are given.
- Analyze every workspace folder the editor initializes, not only the first.
- Include ancestor package manifests in the editor's cross-file analysis, matching the CLI.
- Cancel queued editor requests and stop fix-all runs when the client cancels them.
- Publish each file's merged editor diagnostics once, with UTF-16 positions, and drop diagnostics superseded by newer edits.
- Keep the editor responsive by caching workspace parses and reusing project indexes for edits that don't change declarations.
- Resolve configuration and cross-file rules per editor workspace folder, matching a CLI check of that folder.
- Pick up created, changed and removed files from `workspace/didChangeWatchedFiles` events, and revalidate closed files in editors without file watching.
- Fix `falcon check` hanging on expressions nested past the parser's depth limit, such as thousands of nested list literals; the excess nesting now reports one syntax error.
- Fix parsing of const record-typed local declarations and nullable record fields in casts and type tests.
- Reuse semantic indexes for unrelated libraries after declaration, import, export and part edits in the editor and during autofixes.
- Fix stack overflows and repeated analysis when lint rules inspect long expression chains.
- Fix stack overflows on very long expression chains in parameter defaults, async safety checks, switch guards, `avoid-print`, `library-annotations`, `null-check-on-nullable-type-parameter`, `prefer-final-fields`, and controller disposal checks.
- Report a malformed member configuration in the editor without dropping the base or other members' configurations.
- Warn when a member configuration sets `max-errors`, `error-on-warnings` or `baseline`, which only the base configuration controls.
- Report a missing input path instead of panicking when part of the path is a regular file.
- Sort syntax errors before lint findings in both the CLI and editor.
- Skip files with syntax errors, symlinks or concurrent disk changes when writing fixes.

## 0.4.4 (2026-10-23)

### Features

- Make `falcon check` lint the current directory when no paths are supplied.
- Report missing input paths and runs that process no files as errors; use `--no-errors-on-unmatched` to allow them in automation.
- Count package manifests as processed inputs and explain when explicit paths are excluded by file filters.
- Resolve file filters, exclusions and overrides relative to the configuration file in the CLI and editor.
- Resolve user-level `~/.falcon.json` globs from the CLI current directory or editor workspace root.
- Make single-star globs stop at directory separators and double-star globs match nested paths.
- Reject rules under the wrong group or section and unsupported override preset toggles; publish a matching override schema.
- Serialize JSON diagnostic severities as lowercase `error`, `warning`, and `info`.
- Translate legacy settings and YAML exclusions with `falcon migrate`, preserving severity aliases and recursive exclusions where possible.
- Warn how many formerly implicit rules need explicit enablement during migration.
- Normalize keys and resolve collisions during `falcon migrate`.
- Move known rules to their declared group and section during migration.
- Report migration errors with structured configuration paths and repair them without deleting unrelated settings.
- Omit unresolved source positions from JSON and text diagnostics instead of displaying zero values.
- Remove `prefer-any-or-every`; point existing config keys and suppressions to `complexity/prefer-iterable-every`.
- Reject unknown configuration keys, rules, groups and domains with full paths, including in overrides; warn on accepted legacy spellings.
- Add typed rule options with kebab-case keys and rule-specific JSON schema completion.
- Identify invalid rule options by configuration path and reject unsupported options, invalid values, mixed lists and unknown member-order tokens.
- Accept whole-number numeric options and `max-errors` values such as `1.0`; report allowed ranges for out-of-range integers.
- Accept null optional sections in the loader and editor schema.
- Accept recognized legacy snake_case option keys with deprecation warnings.

### Fixes

- Match globs when the configuration root or a subdirectory is reached through a directory alias, such as macOS `/var` resolving to `/private/var`.
- Treat `~/.falcon.json` reached through a symlinked home directory as user-level settings.
- Resolve `avoid-returning-widgets`, `avoid-unused-parameters`, `provide-deprecation-message`, `boolean-prefixes`, `member-ordering` and `avoid-unused-constructor-parameters` using semantic analysis.
- Speed up analysis of long arithmetic chains, constant expressions and async-safety rules.
- Keep all diagnostics when determining the `falcon check` exit status, so `--max-errors` limits printed output only.
- Prune excluded directories while discovering Dart sources and package manifests.
- `no-logic-in-create-state`: Report a `createState` method that returns a constructed type whose known inheritance hierarchy excludes `State`; skip unresolved hierarchies.
- Apply the same severity and options collision policy to rule aliases in configuration loading and `falcon migrate`.
- Deliver configuration warnings and errors to the editor through the language server; the CLI reports them instead of the library printing them directly.
- Recognize `const (...)` patterns as constant contexts for named-constant suggestions and redundant-`const` checks.
- Standardize lint diagnostic messages as complete sentences with terminal punctuation, backtick-quoting code keywords, types, operators, and interpolated identifiers.
- Let a block documentation comment immediately after line documentation comments replace that group's documentation for the declaration.
- Include comments inside string interpolation bodies in documentation queries.
- Warn in the CLI and editor about unreadable or malformed package configuration, lockfiles, SDK and embedder mappings, SDK version files and external Dart sources.
- `exhaustive-cases` uses resolved deprecation identities when choosing between static constants with the same value, so a user-defined `Deprecated` annotation no longer changes the suggested case.
- Preserve type bounds, alias identities and diagnostics during generic type analysis.
- Fix package ownership checks through directory aliases.
- Fix analysis-environment invalidation through directory aliases, including missing files and macOS temporary directories.
- Fix crashes when checking very long arithmetic, logical, null-coalescing and string expressions, including initializers and call arguments.
- Avoid exponential parsing for nested nullable declarations and conditional expressions while preserving valid syntax.
- Recognize test_core and test_api consistently in test assertion rules, including framework import gating and duplicate matcher checks.
- `avoid-catches-without-on-clauses` recognizes `.error(...)` forwarding only for the resolved SDK `Future`, so logger calls still report untyped catches.
- Fix `unnecessary-nullable` false positives for method tear-offs, cascade references and parameter writes that may be null.
- Fix `unnecessary-nullable` false positives for arguments typed `FutureOr<T?>` or as extension types that do not implement `Object`.
- Resolve cross-file rules consistently even when no file rules require semantic resolution.
- Do not suggest named constants or remove redundant `const` in parameter defaults, relational-pattern operands or map-pattern keys.
- Preserve symlink and parent-component resolution while avoiding repeated canonicalization for indexed CLI inputs.
- Prevent language-server crashes when dependencies export many files on machines with few processors.
- Distinguish user declarations from SDK and package declarations with colliding names, including part-file uses.
- `use-existing-variable` now detects reuse when a computation depends on an earlier variable in the same declaration statement.
- Fix crashes when checking very long binary expressions.
- Make `falcon migrate` refuse ambiguous exclusions or repairs that erase a section; reject unusable overrides and invalid top-level file scopes without writing.
- Make checks of deeply nested closures finish promptly.
- Fix closure return types when non-returning calls use block-scoped variables or promoted callables later assigned another type.
- Prevent nested virtual directory aliases from crossing package boundaries when resolving package identities.
- Prevent exponential parsing of nested collections and malformed lambdas, and recover from deeply nested patterns and types without crashing.
- Keep valid long expression and pattern chains working.
- Recover unclosed string interpolations without swallowing subsequent declarations.
- Fix `prefer-iterable-every` recommendations for filtered emptiness, including negated predicates and filtered-length comparisons.
- Resolve `prefer-iterable-every` against SDK `Iterable.where` calls.
- Preserve configuration key order during `falcon migrate` and apply the same collision policy to YAML conversion, JSON upgrades and configuration loading.
- Recognize empty import paths as imports of the current library without misleading warnings; treat paths to directories as absent optional sources.
- Resolve `avoid-private-typedef-functions` and `avoid-duplicate-mixins` across sibling part files.
- Resolve Flutter widget, state and SDK async-error APIs by declaration identity in `avoid-returning-widgets`, `member-ordering`, `avoid-returning-null-for-void` and `avoid-catches-without-on-clauses`.
- Resolve override and deprecation annotations to their `dart:core` identities, so user-defined lookalikes do not suppress diagnostics.
- Fix crashes in parsing and linting deeply nested Dart code, and report worker panics as analysis diagnostics.
- `avoid-dynamic-calls` no longer reports inherited members whose types cannot be resolved.
- Pin the VS Code configuration schema to the extension release and write a release-pinned `$schema` URL when `falcon migrate` creates a configuration.
- Keep both editor extension versions synchronized with releases.
- Preserve unknown generic arity for types such as `ValueNotifier` and `Provider`.
- `unreachable-from-main` ignores references in Markdown link display text and definitions, code blocks, Dartdoc directives and trailing inline documentation comments; reference-style destination labels remain references.

## 0.4.3 (2026-10-20)

### Features

- falcon can be installed with `dart pub global activate falcon`.
- `accessible-widget-labels`: Warn about unlabeled interactive gesture targets, including Tooltip and Semantics labels and configured gesture callbacks.
- `always-pass-global-key`: Require GlobalKey fields to be passed to widgets, while excluding constant keys and cached widgets.
- `arguments-ordering`: Named arguments follow their resolved parameter declaration order by default.
- `avoid-accessing-other-classes-private-members`: Flag access to another class's private members, including cascades and null-aware access.
- `avoid-assigning-to-static-field`: Flags instance methods writing resolved static fields, including compound assignments and increments.
- `avoid-banned-annotations`: Ban configured dotted annotation names, without the leading `@`. This check stays inactive until configured with the relevant match lists and diagnostic messages.
- `avoid-banned-exports`: Ban decoded export URIs, including all conditional alternatives. This check stays inactive until configured with the relevant match lists and diagnostic messages.
- `avoid-banned-names`: Ban configured declaration names, including loop variables, catch parameters, named constructors and import prefixes.
- `avoid-banned-types`: Ban configured resolved type annotations, including expanded aliases and nested generic types.
- `avoid-barrel-files`: Discourage files containing only re-exports, without declarations or parts.
- `avoid-bloc-public-fields`: Add optional rule: Keep package-confirmed Bloc instance fields private; inherited API overrides are allowed.
- `avoid-bloc-public-methods`: Add optional rule: Enforce the package-confirmed Bloc convention.
- `avoid-border-all`: Avoids Flutter's non-const `Border.all` factory. Flutter widget checks resolve package identities.
- `avoid-complex-arithmetic-expressions`: Reports arithmetic expressions containing too many terms.
- `avoid-constant-assert-conditions`: Add optional correctness rule: Flags assertions whose Boolean condition is statically constant.
- `avoid-constant-conditions`: Add optional correctness rule: Flags statically constant binary branch, loop, collection, and expression conditions.
- `avoid-constant-switches`: Add optional correctness rule: Flags switch statements and expressions with a statically constant subject.
- `avoid-disposing-late-fields`: Warn when `dispose` releases a late State field that may be unset or lazily initialized.
- `avoid-double-slash-imports`: Reject repeated path separators in directive URIs (import, export and part).
- `avoid-duplicate-cascades`: Add optional correctness rule: Flags repeated field or index assignment targets within one cascade.
- `avoid-duplicate-exports`: Add optional correctness rule: Flags export directives whose visible names are already covered by another export.
- `avoid-duplicate-factories`: Add optional correctness rule: Flags factories in one type with the same implementation, ignoring factory names and alpha-renaming parameters by position.
- `avoid-duplicate-initializers`: Add optional correctness rule: Flags final/const variables with identical initializers in the same scope.
- `avoid-duplicate-mixins`: Add optional correctness rule: Flags repeated mixins in a with clause or a same-library superclass chain.
- `avoid-duplicate-named-imports`: Add optional correctness rule: Flags imports of the same library that differ only in their name prefix.
- `avoid-empty-test-groups`: Reports inline framework groups without any test registrations.
- `avoid-hardcoded-credentials`: Warn when credential-like variables or fields receive nonempty string or numeric literals, including optional parameter defaults.
- `avoid-if-with-many-branches`: Reports long if/else-if chains; replace them with a switch or extracted helpers.
- `avoid-incorrect-image-opacity`: Flags Opacity directly wrapping a Flutter Image, including named constructors. Flutter widget checks resolve package identities.
- `avoid-incorrect-uri`: Reject invalid `Uri` constructor combinations and query parameter values of unsupported types.
- `avoid-instantiating-in-value-provider`: Add optional rule: Value providers borrow existing instances; newly constructed values need create.
- `avoid-inverted-boolean-checks`: Prefer direct equality comparisons and De Morgan forms in control-flow conditions.
- `avoid-long-records`: Reports record values and record types with too many fields.
- `avoid-missing-completer-stack-trace`: Preserve error origins when calling SDK `Completer.completeError`.
- `avoid-missing-controller`: Require Flutter text fields to expose their updated value. Flutter widget checks resolve package identities.
- `avoid-missing-enum-constant-in-map`: Add optional correctness rule: Require enum-keyed map literals to include every declared enum value.
- `avoid-missing-image-alt`: Requires semanticLabel on Flutter Image and imageSemanticLabel on FadeInImage. Flutter widget checks resolve package identities.
- `avoid-mounted-in-setstate`: Check mounted before invoking Flutter State.setState, never inside its callback. Flutter widget checks resolve package identities.
- `avoid-multi-assignment`: Split chained assignments into separate statements.
- `avoid-mutating-parameters`: Detect writes through named function, method and constructor parameter objects.
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
- `avoid-referencing-subclasses`: Keep base classes independent from transitive subclasses; configure `ignored_annotations` to replace the default annotation exclusions.
- `avoid-self-compare`: Accept `avoid-self-compare` as an alias for the existing `no-self-comparisons` rule; both names resolve to the same implementation.
- `avoid-sensitive-query-params`: Detect explicit secret-bearing URL query keys in literals and SDK `Uri` queries, including percent-encoded keys, without flagging unrelated text or shadowed constructors.
- `avoid-suspicious-super-overrides`: Find getters hiding fields supplied through the immediate super constructor's `this.` formals or declaring parameters.
- `avoid-top-level-members-in-tests`: Requires public declarations in test files, except main, to be private.
- `avoid-unconditional-break`: Reports unconditional break, continue, return or throw in a for-loop body.
- `avoid-unnecessary-block`: Add an optional rule that flags standalone nested blocks introducing no lexical declarations.
- `avoid-unnecessary-call`: Add an optional rule that prefers direct invocation of functions and resolved callable instances.
- `avoid-unnecessary-collections`: Avoid singleton collection literals where SDK APIs accept a single value.
- `avoid-unnecessary-continue`: Add an optional rule that flags an unlabeled `continue` at the end of a loop body or its final branch.
- `avoid-unnecessary-futures`: Prefer synchronous declarations when an async Future only returns scalars.
- `avoid-unnecessary-gesture-detector`: Flag Flutter GestureDetectors without non-null event handlers, while preserving explicit hit-test behavior and distinct child gestures.
- `avoid-unnecessary-getter`: Add an optional rule that flags getters exposing only a private final field. Upcasting getters remain allowed by default; set `ignore_upcast: false` to report them.
- `avoid-unnecessary-length-check`: Add an optional rule for length checks around `for-in` and `forEach`: empty collections already skip those bodies.
- `avoid-unnecessary-local-variable`: Add an optional rule that flags a single-use local used only to initialize the next local.
- `avoid-unnecessary-negations`: Add an optional rule that simplifies negated bool literals and equality comparisons with a logical negation on both sides.
- `avoid-unnecessary-overrides-in-state`: Add an optional rule that flags redundant Flutter `State` overrides dropping inherited `@protected` annotations.
- `avoid-unnecessary-reassignment`: Add an optional rule that flags an eager local initializer immediately overwritten without a read.
- `avoid-unnecessary-super`: Omit implicit `super()` initializers and provably redundant member prefixes.
- `avoid-unreachable-for-loop`: Reports iteration of a collection in a branch that requires it to be empty.
- `avoid-unsafe-reduce`: Add optional correctness rule: Warn when SDK Iterable.reduce can receive an empty collection.
- `avoid-unused-after-null-check`: Reports a null-checked local ignored by a branch that passes another local of the same resolved type to a call.
- `avoid-wrapping-in-padding`: Flag Padding directly wrapping a Flutter Container that can own the padding, except when moving it changes keyed element bounds.
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
- `minimum-touch-target`: Warn about explicitly undersized Flutter touch targets while exempting disabled buttons.
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
- `prefer-correct-json-casts`: Decode JSON containers using their runtime generic types and flag casts with non-nullable Object elements or values.
- `prefer-correct-notifier-file-name`: Requires snake_case file names ending in _notifier.dart for resolved Riverpod notifier classes.
- `prefer-correct-provider-file-name`: Require snake_case `_provider.dart` files for resolved Riverpod provider declarations.
- `prefer-correct-test-file-name`: Require `_test.dart` suffixes for entry points under package-root `test/` and `integration_test/`.
- `prefer-define-hero-tag`: Require an explicit heroTag on Flutter FloatingActionButton constructors. Flutter widget checks resolve package identities.
- `prefer-equatable-mixin`: Prefer EquatableMixin to directly extending package:equatable's Equatable.
- `prefer-expect-later`: Requires expectLater when a canonical expect call receives an unawaited Future.
- `prefer-getter-over-method`: Prefer getters for synchronous, zero-argument methods that only read a field.
- `prefer-immutable-provider-arguments`: Require stable value equality for fresh Riverpod family arguments.
- `prefer-match-file-name`: Match a file's sole class or enum name to its snake-case basename.
- `prefer-multi-bloc-provider`: Add optional rule: Flatten directly nested package:flutter_bloc BlocProviders with MultiBlocProvider.
- `prefer-multi-provider`: Add optional rule: Prefer a flat MultiProvider to directly nested provider widgets.
- `prefer-named-parameters`: Prefer named parameters when a declaration exceeds `max_number` (default 1).
- `prefer-provider-extensions`: Add optional rule: Prefer context.read/watch when provider extensions are in scope.
- `prefer-random-secure`: Prefer the cryptographically secure `dart:math` random generator.
- `prefer-return-await`: Await returned SDK Futures inside a try body protected by a catch clause.
- `prefer-riverpod-notifier-suffix`: Requires a `Notifier` suffix on classes extending resolved Riverpod notifiers, excluding generated Dart files.
- `prefer-riverpod-provider-suffix`: Requires a Provider suffix on resolved Riverpod top-level providers and static fields.
- `prefer-single-setstate`: Merge consecutive Flutter State.setState statements in the same block. Flutter widget checks resolve package identities.
- `prefer-sized-box-square`: Prefer `SizedBox.square` when an unnamed Flutter `SizedBox` has identical explicit width and height.
- `prefer-test-matchers`: Recommends Matcher expectations and direct type, equality, or ordering matchers.
- `prefer-type-over-var`: Prefer explicit types over `var` in ordinary variable declarations.
- `prefer-unique-test-names`: Reports duplicate test descriptions within a group and constant names in loops.
- `prefer-use-callback`: Prefer `useCallback` for synchronous memoized function literals.
- `prefer-use-memo`: Prefer memoizing freshly allocated collection values used as hook keys.
- `prefer-using-list-view`: Recommend `ListView` for a `SingleChildScrollView` containing a `Column` with at least two unconditional children; preserve alignment and padding, and do not offer an automatic fix.
- `prefer-void-callback`: Prefers Flutter's `VoidCallback` alias over a nongeneric `void Function()` type. Flutter widget checks resolve package identities.
- `record-fields-ordering`: Flags named record type fields that violate the configured order.
- `use-existing-destructuring`: Add an optional rule to reuse or extend existing record destructuring instead of rereading a field.
- `use-existing-variable`: Add an optional rule to reuse a local for stable SDK arithmetic and `String.length`/`int` parity getters.
- Add `emit-new-bloc-state-instances`.
- Add `prefer-sealed-bloc-events` and `prefer-sealed-bloc-state`.
- Add `prefer-immutable-bloc-events` and `prefer-immutable-bloc-state`.
- Add `avoid-duplicate-bloc-event-handlers`.
- Add `check-is-not-closed-after-async-gap`.
- Add `avoid-emit-outside-bloc`.
- Add `avoid-complex-conditions` and `avoid-complex-loop-conditions`, with a `threshold` option.
- Add `avoid-collapsible-if`.
- Add `avoid-slow-collection-methods`.
- Add `move-variable-outside-iteration`.
- Add `avoid-duplicate-map-keys`, `avoid-duplicate-collection-elements`, `avoid-duplicate-switch-case-conditions`, `avoid-missing-interpolation`, `avoid-contradictory-expressions` and `avoid-unmodified-loop-condition`.
- Add the opt-in cross-file rules `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles` and `banned-imports`.
- Add `avoid-undisposed-instances`, `dispose-focus-node-and-timer`, `dispose-fields`, `always-remove-listener`, `avoid-recursive-widget-calls` and `avoid-unnecessary-stateful-widgets`.
- Add the `test`, `bloc`, `riverpod`, `provider`, `flutter_hooks` and `equatable` values for `linter.domains`.
- Add Flutter lifecycle rules `use-setstate-synchronously`, `require-super-dispose`, `avoid-empty-setstate`, `avoid-unnecessary-setstate`, `use-closest-build-context`, `avoid-late-context` and `avoid-inherited-widget-in-initstate`.
- Add `avoid-conditional-hooks`, `avoid-hooks-outside-build`, `avoid-misused-hooks` and `avoid-unnecessary-hook-widgets`.
- Add `avoid-read-inside-build`, `avoid-watch-outside-build`, `dispose-providers` and `prefer-read-for-methods`.
- Add `use-ref-read-synchronously`, `use-ref-and-state-synchronously`, `dispose-provided-instances` and `dispose-provider-resources`.
- Add `avoid-ref-watch-outside-build`, `avoid-ref-read-inside-build`, `prefer-ref-read-for-methods`, `avoid-calling-notifier-members-inside-build` and `avoid-unnecessary-consumer-widgets`.
- Add `prefer-early-return`, `prefer-returning-condition`, `prefer-switch-expression`, `prefer-for-in` and `prefer-named-boolean-parameters`.
- Add `avoid-commented-out-code`, `avoid-shadowing`, `avoid-shadowed-extension-methods`, `avoid-assignments-as-conditions`, `avoid-bitwise-operators-with-booleans` and `cascade-after-null-coalescing`.
- Add `avoid-duplicate-test-assertions` and `missing-test-assertion` for `*_test.dart` files.
- Add `list-all-equatable-fields`, `add-equatable-props` and `avoid-mutable-equatable-subclass-fields` for `package:equatable` classes.
- Add `avoid-collection-equality-checks`, `avoid-unrelated-type-casts`, `avoid-unsafe-collection-methods` and `avoid-uncaught-future-errors`.
- Add the `security` rule `avoid-weak-cryptographic-algorithms`.
- Add `avoid-unnecessary-if`, `avoid-unnecessary-return` and `avoid-redundant-else`.
- Accept `avoid-unnecessary-conditionals` as an alias of `avoid-bool-literals-in-conditional-expressions`.
- Add Flutter widget rules `avoid-shrink-wrap-in-lists`, `prefer-align-over-container`, `prefer-center-over-align`, `prefer-padding-over-container`, `prefer-constrained-box-over-container`, `prefer-transform-over-container`, `prefer-container` (with a `min_sequence` option) and `prefer-for-loop-in-children`.
- Add the `a11y` rule `prefer-action-button-tooltip`.
- `widget-tree-depth`: Limits the depth of an inline Flutter widget construction tree.

- `banned-usage`: Ban configured resolved API references by top-level name or `DeclaringType.member`, including calls, properties, constructors, cascades and initializers.
- `banned-usage` resolves lexical declarations, inherited members, named extensions, property setters and cascade targets.
- `no-magic-string`: Extract inline string literals into named variables or constants; invocation exemptions apply only to direct arguments, and interpolation expressions are checked.
- `no-magic-string` counts exempt literals for `allow_only_once`, while excluding pattern literals, annotations, directive URIs and conditional-import configuration strings from diagnostics or counts as applicable.

### Fixes

- Fix string interpolation parsing so `'${await x}'` is retained in the syntax tree and escaped braces such as `${a\\}b}` produce a parse error.
- `newline-before-return`: Count blank lines above attached comments, avoiding diagnostics on already separated return statements.
- Parse switch-expression guards with parenthesized operands, such as `when (a = b) => 1`, as guards rather than closures.

## 0.4.2 (2026-10-17)

### Breaking Changes

- Prebuilt macOS binaries now require macOS 11.7.1 or newer.

### Features

- Release downloads now include `SHA256SUMS` and GitHub build provenance attestations, with instructions for verifying both.
- Add Neovim LSP setup instructions.

### Fixes

- Resolve `package:` imports in each package's dependency context so workspaces using different versions of a dependency avoid incorrect identities and false positives.
- Recognize Flutter and meta declarations across dependency copies to avoid incorrect resolution.
- Fix package discovery and cross-file diagnostics when a workspace is opened through a symlink.
- Fix the Zed extension repository URL to point to `JacobDevelops/dart_falcon`.
- Make quick-start links install the latest release without a version pin and include versionless platform tarballs alongside versioned archives.
- Build the requested tag for manual re-releases instead of the current main branch.

## 0.4.1 (2026-10-17)

### Fixes


## 0.4.0 (2026-10-17)

### Breaking Changes

- Use kebab-case rule IDs in configuration and diagnostics, merging duplicate variants such as `no_empty_block` and `avoid_empty_blocks` into `no-empty-block`; `falcon migrate --input falcon.json --write` rewrites legacy IDs.
- Rename the `project` configuration section to `cross-file`; legacy settings and suppression paths remain accepted and `falcon migrate` rewrites the configuration.
- Use Falcon's recommended rule set: make `unused-files`, `unused-code` and `unnecessary-nullable` opt-in, and recommend `depend-on-referenced-packages`, `package-names` and `secure-pubspec-urls`.
- Add `a11y` and `security` rule groups and move `secure-pubspec-urls` to `security`.
- Remove the retired `unsafe-html`, `package-prefixed-library-names` and `package-api-docs` rules.
- Always analyze files in parallel; remove `--parallel` and use `RAYON_NUM_THREADS` to limit workers.
- Count constructors, setters, operators, closures, local functions and function types in `max-parameters-for-function`.

### Features

- Add the official `package:lints` and `package:flutter_lints` presets and 120 additional official Dart lints.
- Extend `falcon migrate` to read both list and map forms of `linter.rules` in analysis options.
- Add Dart 3.9 syntax support, including dot shorthands, digit separators, function type tests and adjacent string literals in constant patterns.
- Resolve lint checks using declaration identity, lexical scope, types, constants, external libraries and Flutter BuildContext flow.
- Implement `depend-on-referenced-packages`, `package-names` and `secure-pubspec-urls`, with diagnostics pointing into `pubspec.yaml`.
- Implement `prefer-contains` for resolved String and List membership tests.
- Improve const-constructor checks across inheritance and redirecting constructors.
- Add a searchable documentation site and rule catalog.

### Fixes

- Report parse failures as `syntax-error` diagnostics in the CLI and LSP, with one-based line and column positions in text output.
- Reject malformed `falcon.json` files instead of silently using defaults, warn about unknown top-level keys, and reject `max-errors: 0`.
- Stop following directory symlinks during CLI and LSP workspace collection so Flutter plugin symlinks do not pull the pub cache into analysis.
- Detect unreachable files in `unused-files` using entrypoint reachability while honoring conditional imports and preserving public barrel files.
- Traverse Dart patterns, statements, literals, switch expressions and nested closures consistently so rules do not miss code or misreport unused parameters.
- Fix type-based checks for casts, type assertions, boolean comparisons, ignored return values, overrides, const constructors and Flutter widget identities.
- Align official Dart lint diagnostics and source ranges with analyzer behavior.
- Respect each package's language version and analysis environment, load project dependencies, and select duplicate libraries deterministically.
- Use UTF-16 LSP positions and handle malformed JSON-RPC parameters without terminating the server.
- Handle deeply nested Dart code without crashing and report analysis thread-spawn failures instead of panicking.

## 0.3.0 (2026-07-22)

### Breaking Changes

- Replace analyzer `// ignore:` comments with `// falcon-ignore <section>/<group>/<rule>: <reason>` and `// falcon-ignore-all`; require reasons and do not let Dart ignore comments suppress Falcon diagnostics.

### Features

- Report `malformed-suppression` warnings for missing reasons, incorrect groups, and unknown rules instead of silently ignoring invalid suppression comments.

## 0.2.1 (2026-07-22)

### Fixes

- Count same-file references as usage in `unused-code` so declarations used within their own file are not reported as unused.
- Align argument, dynamic-type, literal-formatting, magic-number, comment-formatting, nesting and nullable-return checks with dart_code_linter and pyramid_lint behavior, including documented exemptions.
- Restrict identifier-length and boolean-prefix checks to declarations.
- Honor member-ordering options.
- Exempt constructors from parameter-count checks.
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
