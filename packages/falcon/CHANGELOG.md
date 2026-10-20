# Changelog

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
