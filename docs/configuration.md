# Configuring falcon (`falcon.json`)

falcon reads `falcon.json`. Discovery order (first match wins):

1. `falcon.json` in the current directory
2. `falcon.json` at the enclosing git root
3. `~/.falcon.json`

If no config is found, falcon enables its own recommended set: 215 recommended
rules out of 513 shipped rules, at their default severity (warning), chosen independently of upstream presets.

A config that **is** found but fails to load is a hard **error**: falcon prints
a message and exits non-zero rather than silently falling back to defaults. This
covers invalid JSON, wrong-typed values, `max-errors: 0` (see
[`max-errors`](#max-errors)), and the [legacy flat
schema](#migrating-from-the-legacy-flat-schema). Discovery and explicit
`--config` behave identically here, so a typo can never quietly re-enable every
rule.

## Unknown and legacy keys

Unknown keys at any configuration depth and unknown domain names are **errors**:
the message names the full key path, and the configuration does not load. Rule
`options` retain their rule-specific validation. An override may omit `includes`
to apply everywhere, but a misspelled `includes` is rejected.

The deprecated spellings `project` and `cross_file` (for `cross-file`) and
`max_errors` (for `max-errors`) still load with deprecation warnings pointing at
`falcon migrate`, including inside overrides. Registered legacy rule ids also
load with warnings. The CLI prints configuration warnings; the language server
sends them to the editor. The published JSON schema accepts canonical keys only.

## Editor autocomplete (`$schema`)

Point the top-level `$schema` at the published JSON schema to get rule-name
autocomplete and validation in editors:

```json
"$schema": "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/v0.4.3/schema/falcon.schema.json"
```

The VS Code extension enables the [published schema](../schema/falcon.schema.json) automatically for files named `falcon.json`.

## Full example

```json
{
  "$schema": "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/v0.4.3/schema/falcon.schema.json",
  "files": {
    "includes": ["**", "!.dart_tool/**", "!build/**", "!**/*.g.dart", "!**/*.freezed.dart"]
  },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "complexity": {
        "max-lines-for-file": "warn"
      },
      "style": {
        "prefer-trailing-comma": { "level": "error", "options": {} }
      }
    },
    "domains": { "flutter": "recommended" }
  },
  "cross-file": {
    "enabled": true,
    "rules": {
      "correctness": {
        "unused-files": "warn",
        "unused-code": "warn",
        "unnecessary-nullable": "off"
      }
    }
  },
  "max-errors": null
}
```

## `files.includes`

A single mixed list of globs:

- Plain entries are **positive includes**: only files matching at least one are
  linted. A list containing a bare catch-all (`**` or `**/*`), or an empty list,
  means "no positive filtering": every file is a candidate, even outside the
  configuration directory. Exclusions still resolve relative to that directory.
- Entries prefixed with `!` are **exclusions**, applied on top of the includes.

Exclusions and any CLI `--exclude` patterns are unioned.

Invalid glob patterns produce a warning and are skipped consistently in CLI
file selection, LSP selection, and overrides. If no valid positive patterns
remain, positive filtering is unrestricted; valid exclusions still win.

All globs resolve relative to the directory containing `falcon.json`, including
CLI `--exclude` patterns and override filters. User-level `~/.falcon.json` is
settings rather than a workspace: its globs resolve from the CLI current working
directory or the LSP workspace root, not the home directory. This also applies
when the home directory is reached through a symlink or another directory alias;
a config symlink in another directory still uses that directory as its root.
With no config, the current directory is the base. CLI relative paths, absolute paths, and LSP
document paths therefore select the same files, including symlinked configuration
or document roots and not-yet-created document paths beneath those roots. A link
to a subdirectory also preserves its root-relative path: a document reached
through `lib-link -> <root>/lib` matches `lib/**` in file filters and overrides.
`*` never crosses `/`; `**` crosses directories.
For example, `lib/*.dart` matches direct children, while `lib/**/*.dart` also
matches nested files. An exclusion matching a directory excludes its whole
subtree, and that directory is pruned before traversal. For example, `!build`
excludes the build directory and its contents, while `!lib/*.dart` excludes only
direct Dart children (unless a matching child is itself a directory).

## `linter`

- `enabled` (default `true`): when `false`, no file rule runs. Cross-file checks are configured separately.
- `domains`: per-domain gating. Keys are domain names (`flutter`, `test`,
  `bloc`, `riverpod`, `provider`, `flutter_hooks`, `equatable`); values are
  `all`, `recommended`, or `none`.
- `rules`: falcon's recommended set plus per-group rule levels.

### Language server reloads

The language server reloads configuration and mutable dependencies when the editor sends watched-file notifications. This includes path and git dependencies, Flutter SDK packages, and newly generated Dart files.

Configuration warnings and errors appear in the editor and identify the source
file. Invalid reloads keep the last successfully loaded settings until the file
is fixed. Repeated notifications with unchanged warnings do not show duplicate
messages.

Out-of-workspace dependency watches need editor support for LSP 3.17 relative patterns. Without it, restart the server after changing those dependencies. If watched-file registration is unavailable or fails, saving workspace configuration files still reloads configuration; restart after dependency changes. Restart after manually editing hosted-package or Dart SDK sources too.

### `linter.rules`

- `recommended` (default `true`): whether falcon's recommended set is active.
- Every other key is a **group** (`a11y`, `complexity`, `correctness`, `performance`, `security`, `style`, or `suspicious`) mapping rule names to a configuration.

> **Rule ids are kebab-case**, for example `max-lines-for-file` and `no-magic-number`. Registered legacy `snake_case` aliases still work in config and suppression comments. Run `falcon migrate --input falcon.json --write` to rewrite them to canonical ids (see [Migrating](#migrating-from-dart_code_linter--pyramid_lint)).

A rule configuration is either a **level string** or an **object**:

```json
"avoid-dynamic": "error"
"use-design-system-item": { "level": "warn", "options": { "items": [] } }
```

Levels:

| Level   | Meaning                                   |
|---------|-------------------------------------------|
| `off`   | disabled                                  |
| `on`    | enabled at the default severity (warning) |
| `info`  | enabled, reported as info                 |
| `warn`  | enabled, reported as warning              |
| `error` | enabled, reported as error                |

### Resolution order

For each rule falcon resolves an effective severity (or "disabled"):

1. If `linter.enabled` is `false` → disabled.
2. An **explicit** entry under the rule's group wins and bypasses domain gating.
3. Otherwise, a domain-scoped rule follows its domain setting (`flutter`, `test`, `bloc`, `riverpod`, `provider`, `flutter_hooks`, or `equatable`). `all` enables every rule in that domain; `none` disables them; `recommended` enables only recommended rules when falcon's recommended set is active. A missing domain setting defaults to `recommended`.
4. Otherwise, the rule runs only if it is recommended and falcon's recommended set is active.

With no config file, only falcon's recommended rules run, at warning severity.

## `max-errors`

Optional cap on printed diagnostics (`null` = unlimited). In `falcon.json`, the value must be **at least 1**; `0` is rejected at load. The CLI `--max-errors` flag overrides the config value and accepts `0` to hide every diagnostic. The cap never changes the exit code: all diagnostics count, including those not printed. The legacy config key `max_errors` still loads with a deprecation warning.
Whole-number JSON values such as `1.0` are accepted; fractional and out-of-range
values are rejected.

## Rule options

Some rules accept an `options` object under the `{ "level": ..., "options": {…} }`
form. Missing options use the rule's defaults. Unknown keys and invalid option
types reject the configuration at load time with the full configuration path.

An individual option field set to `null` is invalid unless that field explicitly
accepts null. For compatibility, setting the entire `options` value to `null`
is treated like omitting options and uses the rule defaults.

Integer-valued rule options also accept whole-number JSON values such as `1.0`.
Fractional, negative, and out-of-range values are rejected with the allowed range.

```json
{
  "linter": {
    "rules": {
      "complexity": {
        "max-lines-for-file": { "level": "warn", "options": { "max-lines": 400 } },
        "cyclomatic-complexity": { "level": "warn", "options": { "max-complexity": 15 } }
      }
    }
  }
}
```

Option names use `kebab-case`. Recognized historical `snake_case` option keys
still load with a deprecation warning, including keys nested in arrays and
objects. When an alias and canonical key are both present, the canonical key wins
and a collision warning reports the discarded alias value. Unknown `snake_case`
keys are left unchanged, not guessed into new names, and remain subject to
unknown-field validation. `falcon migrate` rewrites recognized keys and reports
collisions as warnings. The JSON schema provides rule-specific option completion,
types, descriptions, and defaults.

The table below is generated from the typed rule options by `cargo xtask docgen`.

<!-- rule-options:start -->

| Rule | Option | Type | Default | Description | Allowed values |
|------|--------|------|---------|-------------|----------------|
| `arguments-ordering` | `alphabetize` | boolean | `false` | Alphabetizes named arguments in ascending order when true. |  |
| `arguments-ordering` | `order` | string &#124; null | `null` | Alphabetical ordering direction for named arguments; unset uses parameter declaration order. | `"ascending"`, `"descending"`, `null` |
| `avoid-banned-annotations` | `rules` | object[] | `[]` | Banned annotation-name globs and the diagnostic message to report on a match. |  |
| `avoid-banned-annotations` | `rules[].deny` | string[] | required | Non-empty glob patterns (JSON key &#96;deny&#96;) that trigger this entry. |  |
| `avoid-banned-annotations` | `rules[].message` | string | required | Diagnostic text emitted when any pattern in this entry matches. |  |
| `avoid-banned-exports` | `rules` | object[] | `[]` | Banned export-URI globs and the diagnostic message to report on a match. |  |
| `avoid-banned-exports` | `rules[].deny` | string[] | required | Non-empty glob patterns (JSON key &#96;deny&#96;) that trigger this entry. |  |
| `avoid-banned-exports` | `rules[].message` | string | required | Diagnostic text emitted when any pattern in this entry matches. |  |
| `avoid-banned-names` | `rules` | object[] | `[]` | Banned declaration-name globs and the diagnostic message to report on a match. |  |
| `avoid-banned-names` | `rules[].deny` | string[] | required | Non-empty glob patterns (JSON key &#96;deny&#96;) that trigger this entry. |  |
| `avoid-banned-names` | `rules[].message` | string | required | Diagnostic text emitted when any pattern in this entry matches. |  |
| `avoid-banned-types` | `rules` | object[] | `[]` | Banned type-name globs and the diagnostic message to report on a match. |  |
| `avoid-banned-types` | `rules[].deny` | string[] | required | Non-empty glob patterns (JSON key &#96;deny&#96;) that trigger this entry. |  |
| `avoid-banned-types` | `rules[].message` | string | required | Diagnostic text emitted when any pattern in this entry matches. |  |
| `avoid-complex-arithmetic-expressions` | `max-terms` | integer | `6` | Maximum number of arithmetic operands in one connected expression. |  |
| `avoid-complex-conditions` | `threshold` | integer | `10` | Maximum number of logical operands in a condition. |  |
| `avoid-complex-loop-conditions` | `threshold` | integer | `1` | Maximum number of logical operands in a loop condition. |  |
| `avoid-empty-test-groups` | `include-methods` | string[] | `[]` | Additional method names recognized as test registrations inside a group. |  |
| `avoid-if-with-many-branches` | `max-branches` | integer | `3` | Maximum number of branches in an &#96;if&#96;/&#96;else if&#96; chain. |  |
| `avoid-long-records` | `max-fields` | integer | `6` | Maximum number of fields in a record. |  |
| `avoid-negated-conditions` | `ignore-null-checks` | boolean | `false` | Exempt null comparisons from the negated-condition diagnostic. |  |
| `avoid-nested-assignments` | `ignore-null-aware-assignments` | boolean | `false` | Exempt null-aware assignments (&#96;??=&#96;) from the nested-assignment diagnostic. |  |
| `avoid-nested-switch-expressions` | `max-nesting` | integer | `1` | Maximum number of nested switch expressions. |  |
| `avoid-nested-switches` | `max-nesting` | integer | `1` | Maximum number of nested switch statements. |  |
| `avoid-nested-try-statements` | `max-nesting` | integer | `2` | Maximum number of nested &#96;try&#96; statements. |  |
| `avoid-referencing-subclasses` | `ignored-annotations` | string[] | `["freezed","Freezed","unfreezed"]` | Annotation names that exempt a class from subclass-reference checks. |  |
| `avoid-unnecessary-getter` | `ignore-upcast` | boolean | `true` | Ignore getters whose value is explicitly upcast to a broader type. |  |
| `banned-imports` | `rules` | object[] | `[]` | Package-relative import boundaries applied to matching source files. |  |
| `banned-imports` | `rules[].deny` | string[] | required | Import URI or package-relative target globs prohibited within &#96;files&#96;. |  |
| `banned-imports` | `rules[].files` | string[] | required | Source-file globs selecting where this boundary applies. |  |
| `banned-imports` | `rules[].message` | string | required | Warning text emitted when a prohibited import matches. |  |
| `banned-usage` | `rules` | object[] | `[]` | Banned API-name globs and the diagnostic message to report on a match. |  |
| `banned-usage` | `rules[].deny` | string[] | required | Non-empty glob patterns (JSON key &#96;deny&#96;) that trigger this entry. |  |
| `banned-usage` | `rules[].message` | string | required | Diagnostic text emitted when any pattern in this entry matches. |  |
| `boolean-prefixes` | `valid-prefixes` | string[] | `[]` | Additional accepted prefixes for boolean names; these extend built-ins. |  |
| `class-members-ordering` | `order` | string[] &#124; null | `null` | Optional member category sequence; unset uses the built-in ordering. | `"public-fields"`, `"private-fields"`, `"fields"`, `"static-fields"`, `"constructors"`, `"named-constructors"`, `"static-methods"`, `"private-methods"`, `"public-methods"`, `"methods"`, `"getters"`, `"setters"`, `null` |
| `cognitive-complexity` | `max-complexity` | integer | `15` | Maximum cognitive complexity allowed for a function. |  |
| `cyclomatic-complexity` | `max-complexity` | integer | `20` | Maximum cyclomatic complexity allowed for a function. |  |
| `enum-constants-ordering` | `order` | string &#124; null | `null` | Alphabetical order for enum constants; unset leaves this rule inactive. | `"ascending"`, `"descending"`, `null` |
| `extend-equatable` | `class-name-pattern` | string &#124; null | `null` | Regex selecting class names that must use Equatable semantics. |  |
| `extend-equatable` | `file-name-pattern` | string &#124; null | `null` | Regex selecting file names whose classes must use Equatable semantics. |  |
| `format-comment` | `ignored-patterns` | string[] | `[]` | Regex patterns that exclude matching comments from formatting checks. |  |
| `format-comment` | `only-doc-comments` | boolean | `false` | When true, checks only documentation comments (&#96;///&#96;). |  |
| `initializers-ordering` | `order` | string &#124; null | `null` | Alphabetical initializer order; unset follows field declaration order. | `"ascending"`, `"descending"`, `null` |
| `map-keys-ordering` | `order` | string &#124; null | `null` | Alphabetical key order; unset leaves this rule inactive. | `"ascending"`, `"descending"`, `null` |
| `max-lines-for-file` | `max-lines` | integer | `200` | Maximum number of source lines allowed in a file. |  |
| `max-lines-for-function` | `max-lines` | integer | `100` | Maximum number of source lines allowed in a function. |  |
| `max-parameters-for-function` | `max-parameters` | integer | `5` | Maximum number of parameters allowed in a function. |  |
| `max-statements` | `max-statements` | integer | `20` | Maximum number of statements allowed in a function. |  |
| `max-switch-cases` | `max-cases` | integer | `10` | Maximum number of non-default cases in a switch. |  |
| `maximum-nesting-level` | `max-nesting` | integer | `5` | Maximum nesting depth allowed in a function. |  |
| `member-ordering` | `order` | string[] &#124; null | `null` | Optional member category sequence; unset uses the built-in ordering. | `"public-fields"`, `"private-fields"`, `"fields"`, `"static-fields"`, `"constructors"`, `"named-constructors"`, `"static-methods"`, `"private-methods"`, `"public-methods"`, `"methods"`, `"getters"`, `"setters"`, `null` |
| `member-ordering` | `widgets-order` | string[] &#124; null | `null` | Optional lifecycle sequence for classes extending &#96;State&#96;. | `"constructor"`, `"init-state"`, `"did-change-dependencies"`, `"did-update-widget"`, `"dispose"`, `"build"`, `"overridden-methods"`, `null` |
| `missing-test-assertion` | `include-assertions` | string[] | `[]` | Additional assertion function names recognized within test callbacks. |  |
| `move-variable-closer-to-its-usage` | `ignore-after-if` | boolean | `true` | Skip moving declarations that follow an &#96;if&#96; statement. |  |
| `move-variable-closer-to-its-usage` | `ignore-const` | boolean | `false` | Skip const variables even when their literal initializer has no side effects. |  |
| `no-magic-number` | `allowed` | number[] | `[-1.0,0.0,1.0]` | Numeric values that are exempt from magic-number diagnostics. |  |
| `no-magic-string` | `allow-only-once` | boolean | `false` | Allow string literal values that occur only once. |  |
| `no-magic-string` | `ignored-invocations` | string[] | `["print","debugPrint"]` | Invocation names whose direct string arguments are exempt from this rule. |  |
| `parameters-ordering` | `order` | string &#124; null | `null` | Alphabetical order for named parameters; unset leaves this rule inactive. | `"ascending"`, `"descending"`, `null` |
| `pattern-fields-ordering` | `order` | string &#124; null | `null` | Alphabetical order for named pattern fields; unset leaves this rule inactive. | `"ascending"`, `"descending"`, `null` |
| `prefer-commenting-analyzer-ignores` | `ignore-global` | boolean | `true` | Do not require explanations for file-wide analyzer ignore directives. |  |
| `prefer-container` | `min-sequence` | integer | `3` | Minimum number of compatible nested layout widgets required for a diagnostic; values below three are treated as three. |  |
| `prefer-correct-callback-field-name` | `name-pattern` | string &#124; null | `null` | Regular expression matching callback field names; absent or empty patterns disable this rule. |  |
| `prefer-correct-error-name` | `allowed-names` | string[] | `[]` | Accepted names for exception variables and error parameters; an empty list disables this rule. |  |
| `prefer-correct-error-name` | `ignored-names` | string[] | `[]` | Names exempt from the accepted-name check. |  |
| `prefer-correct-handler-name` | `ignore-public` | boolean | `false` | Whether to skip public handler methods. |  |
| `prefer-correct-handler-name` | `name-pattern` | string &#124; null | `null` | Regular expression matching handler method names; absent or empty patterns disable this rule. |  |
| `prefer-correct-identifier-length` | `exceptions` | string[] | `[]` | Identifier names exempt from length checks, compared after removing one leading privacy underscore. |  |
| `prefer-correct-identifier-length` | `max-length` | integer | `300` | Longest permitted identifier length, excluding one leading privacy underscore. |  |
| `prefer-correct-identifier-length` | `min-length` | integer | `3` | Shortest permitted identifier length, excluding one leading privacy underscore. |  |
| `prefer-extracting-callbacks` | `allowed-line-count` | integer &#124; null | `null` | Callback body line threshold; unset disables the length filter. |  |
| `prefer-extracting-callbacks` | `ignored-named-arguments` | string[] | `[]` | Named widget-constructor arguments whose callbacks are ignored. |  |
| `prefer-moving-to-variable` | `allowed-duplicated-chains` | integer | `2` | Minimum repeated expression-chain count for extraction (clamped to at least two). |  |
| `prefer-named-parameters` | `max-number` | integer | `1` | Maximum number of positional parameters allowed before named parameters are preferred. |  |
| `prefer-unique-test-names` | `include-methods` | string[] | `[]` | Additional method names recognized as test registrations inside a group. |  |
| `record-fields-ordering` | `order` | string &#124; null | `null` | Lexicographic direction in which named fields in each record type must be ordered. | `"ascending"`, `"descending"`, `null` |
| `use-design-system-item` | `items` | object[] | `[]` | Class names whose construction should be replaced with a design-system equivalent. |  |
| `use-design-system-item` | `items[].class-name` | string | required | Class name of the construction to flag. |  |
| `use-design-system-item` | `items[].use-instead` | string &#124; null | unset | Suggested design-system replacement name, shown in the diagnostic when provided. |  |
| `widget-tree-depth` | `max-nesting` | integer | `8` | Maximum depth of nested widget construction allowed in a widget tree. |  |

<!-- rule-options:end -->

### Banned declarations and API usage (group `suspicious`)

`avoid-banned-names`, `avoid-banned-types`, `avoid-banned-annotations`,
`avoid-banned-exports` and `banned-usage` do nothing until configured.
Like `banned-imports`, each entry in `options.rules` contains a `deny` match
list and a nonblank `message`. Matches are case-sensitive globs; malformed
entries reject the configuration with the full option path.

```json
{
  "linter": {
    "rules": {
      "suspicious": {
        "avoid-banned-names": {
          "level": "warn",
          "options": {
            "rules": [
              { "deny": ["temp", "legacy*"], "message": "Use a descriptive, current identifier instead." }
            ]
          }
        }
      }
    }
  }
}
```

| Rule | Matched value |
|------|---------------|
| `avoid-banned-names` | Declared identifier names, excluding references, initializing/super formals, and `@override`-annotated members and their parameters. |
| `avoid-banned-types` | Resolved type declaration names, including expanded type aliases. |
| `avoid-banned-annotations` | Exact dotted source annotation names, including import prefixes and named constructors. |
| `avoid-banned-exports` | Export URI strings, including conditional alternatives. |
| `banned-usage` | Resolved top-level names or `Type.member` selectors; unnamed constructors use `Type.new`. |

For `banned-usage`, a bare class name matches references to the type itself,
not construction. Ban `Client.new` or `Client.named` to target its constructors.

### Equatable selection

`extend-equatable` (`style`, `equatable` domain) does nothing until configured.
Set `class-name-pattern` and/or `file-name-pattern` to regular expressions selecting
value classes that should extend Equatable. Either matching pattern selects a
class; file patterns match the file basename, not its full path. Missing patterns
select nothing; invalid regular expressions reject the configuration. For example:

```json
"extend-equatable": {
  "level": "warn",
  "options": { "class-name-pattern": "State$" }
}
```

### Test registration helpers

`avoid-empty-test-groups` (`correctness`) and `prefer-unique-test-names` (`suspicious`) accept `include-methods` (array of strings, default `[]`). List custom test registration helper names, for example `"include-methods": ["testCase"]`, to count these calls as tests in addition to canonical test-framework registrations.

### Additional test assertions

`missing-test-assertion` (`correctness`, `test` domain) accepts
`include-assertions` (array of strings, default `[]`). These names extend the
built-in framework expectations and mock verification calls with trusted project
assertion helpers, including local functions. Names match their exact call-site
spelling; include the prefix for prefixed calls.

```json
"missing-test-assertion": {
  "level": "warn",
  "options": { "include-assertions": ["assertResult", "helpers.verifyState"] }
}
```

Only `_test.dart` files importing `test` or `flutter_test` are checked, and the
test registration must resolve to a framework package. Assertions in nested
tests or unused local closures and functions do not satisfy the enclosing test.

### `boolean-prefixes` (`style`)

The `prefer-boolean-prefixes` rule id is an alias for `boolean-prefixes` and uses the same options and declaration scope. Like the other legacy ids, the JSON schema only lists the canonical `boolean-prefixes`, so prefer that id in new configs.

### `format-comment` (`style`)

Checks that comments read like sentences (start upper-case, end with `.`/`!`/`?`/`:`).
Consecutive comment lines are treated as one block: a multi-line block is joined
and split into sentences, so a sentence that wraps across lines is judged as a
whole and continuation lines are never flagged on their own. Both `//` line
comments and `///` doc comments are checked unless `only-doc-comments` is set.

### `no-magic-number` (`style`)

Flags numeric literals that are not extracted to a named constant. Literals are
exempt when they are in the allow-list, inside a variable/field initializer, a
collection literal, a const map or const constructor, a `DateTime` constructor,
or used directly as an index.

### `prefer-extracting-callbacks` (`complexity`, `flutter`)

Flags block-body function-expression callbacks passed as arguments to widget
constructors inside a `Widget`/`State` subclass. Arrow (`=>`) callbacks, empty
blocks and Flutter builders (first parameter `BuildContext`) are never flagged.

### `member-ordering` (`style`)

Without options, the built-in order applies (static const → static fields →
instance fields → constructors → static methods → instance methods).

> `widgets-order` detects `State` from the written `extends State`/`State<T>` clause. Only recognized lifecycle members are ordered; `overridden-methods` matches any other `@override` method.

`class-members-ordering` (`style`) accepts the same `order` option with the same
category tokens; it differs from `member-ordering` only in its default sequence.

### Name-ordering rules (`style`)

`enum-constants-ordering`, `map-keys-ordering`, `parameters-ordering`,
`pattern-fields-ordering`, and `record-fields-ordering` share the following option.
Unlike DCM's alphabetical defaults, these config-driven falcon rules do nothing
unless `order` is configured.

`arguments-ordering` defaults to the callee's declared parameter order and skips
calls whose signature is unknown. `initializers-ordering` defaults to the class's
field declaration order. Both also accept `order` to select alphabetical ordering.
For arguments, `alphabetize: true` also selects ascending order; an explicit
`order` takes precedence, and `alphabetize` defaults to `false`.
Non-field initializers, including `super`, `this`, and assertions, retain their
positions.
For initializer ordering, only literal values and direct constructor-parameter
reads participate; other expressions break ordering runs to preserve evaluation
order when applying the recommendation.

Parameter and argument ordering applies only to named entries, not positional
entries whose order changes the API. Map ordering compares simple literal string
keys, with unknown keys, spreads, and collection control flow breaking the run.
Record ordering applies to named record-type fields, not record literals.
Pattern ordering applies to named object and record fields. Unknown names are
not assigned an alphabetical position.

For example, `"arguments-ordering": { "level": "warn", "options": {
"order": "ascending" } }` requires named call arguments in ascending order.

The `newline-before-method`, `newline-before-constructor`, `newline-before-case`,
`newline-before-throw`, `newline-before-continue`, and `newline-before-break`
rules have no options. Like `newline-before-return`, they require a blank line
only when the declaration, clause, or statement follows another entry in its
containing list. Statement rules apply to blocks, not unbraced switch-case bodies.

## Overrides

`overrides` re-configures rules per path, mirroring biome's `overrides`. The
base `linter` block applies everywhere; each override then re-patches the
resolution for the files its `includes` match.

```json
{
  "linter": {
    "rules": { "recommended": true }
  },
  "overrides": [
    {
      "includes": ["test/**", "!test/fixtures/**"],
      "linter": {
        "rules": {
          "complexity": { "max-lines-for-function": "off" }
        }
      }
    },
    {
      "includes": ["**/theme.dart"],
      "linter": {
        "rules": { "style": { "prefer-correct-identifier-length": "off" } }
      }
    }
  ]
}
```

### `overrides[].includes`

Same glob syntax as `files.includes`: plain entries are positive includes,
`!`-prefixed entries are exclusions. A file matches an override when it is not
excluded by any `!`-pattern and either matches a positive pattern or none are
given.

### `overrides[].linter` and `overrides[].cross-file`

Each block accepts an optional `enabled` switch and a `rules` object with grouped
rule levels and options. Unlike base rule blocks, override rule blocks do not
accept `recommended`; set explicit rule levels or use `enabled` instead. Neither
block accepts `domains`, nested `overrides`, or `files`. An override may carry
either or both blocks. The legacy cross-file keys `project` and `cross_file`
still load with deprecation warnings.
Setting either override block to `null` is equivalent to omitting it. Optional
sections such as base `linter.domains` likewise treat `null` as absent.

- Each explicit rule entry (under its group) **replaces** the base resolution
  for that rule on matching files: `off` disables it; `on`/`info`/`warn`/`error`
  enables it at that severity, even turning on a rule the base config disabled.
- A rule entry's `options` block **replaces** the base rule's options on matching
  files (options are not deep-merged). An override that sets only a level leaves
  the base options intact.
- `"enabled": false` disables every rule in that section for matching files (a
  later override may re-enable a specific one).

### Ordering

For a given file, every override whose `includes` match applies **in order**:
later overrides win over earlier ones, and all win over the base config. A rule
is registered (and run) if it is enabled for *any* path (base or override), so
an override can re-enable a rule the base config turned off.

### Path matching

Overrides use the same config-directory-relative matching as `files.includes`.
For a config in the project root, `test/**` selects test files in both the CLI
and the editor, regardless of how the CLI paths were supplied.

### Per-path options

An override may carry rule `options`, letting a rule run with different options on
different paths, for example a stricter `max-lines-for-file` under `test/`:

```jsonc
"overrides": [
  {
    "includes": ["test/**"],
    "linter": {
      "rules": {
        "complexity": {
          "max-lines-for-file": { "level": "warn", "options": { "max-lines": 1000 } }
        }
      }
    }
  }
]
```

A matching override's `options` **replace** the base rule's options wholesale
(they are not deep-merged); the last matching override wins. An override that
sets only a level leaves the base options intact.

## Suppressing diagnostics

`falcon.json` turns whole rules on or off. To silence a single occurrence
instead, annotate the code with an inline `// falcon-ignore <section>/<group>/<rule>:
<reason>` comment (the reason is mandatory), or `// falcon-ignore-all` for a
whole file. The full grammar, placement rules, stacking, and error cases are
documented in **[suppressions.md](./suppressions.md)**.

```dart
dynamic payload = decode(bytes); // falcon-ignore lint/suspicious/avoid-dynamic: interop boundary
```

## `cross-file`: cross-file rules

Most rules analyze one file at a time. **Cross-file rules** instead reason across
the analyzed file set or package manifests, checking references and package
metadata. They are a **separate feature** from the linter and live under their
own top-level `cross-file` block, *not* under
`linter`. The pre-1.0 spellings `project` and `cross_file` (underscore) are still
accepted as deprecated aliases (each warns on load; run `falcon migrate`):

```json
"cross-file": {
  "enabled": true,
  "rules": {
    "correctness": {
      "unused-files": "warn",
      "unused-code": "warn",
      "unnecessary-nullable": "off"
    }
  }
}
```

- `enabled` (default `true`): when `false`, no cross-file rule runs.
- `rules`: falcon's recommended set plus per-group rule levels, the **same shape**
  as `linter.rules` (level strings or `{ "level", "options" }` objects), but with
  **no `domains`** gating, since cross-file rules are not domain-scoped.

Cross-file rules are grouped under their category (`correctness`, `style`, `security`, or `suspicious`),
share the same metadata table as file rules, and are suppressible with
`// falcon-ignore cross-file/<group>/<rule>: <reason>` comments (or the
`// falcon-ignore-all` variant). Configuring a cross-file rule under
`linter.rules` (or a file rule under `cross-file.rules`) is an error. The message
identifies the full key path and the correct section and group, and the
configuration does not load.

| Rule                     | Group        | Recommended | Replaces (dart_code_linter)   |
|--------------------------|--------------|-------------|-------------------------------|
| `unused-files`           | correctness  | no          | `check-unused-files`          |
| `unused-code`            | correctness  | no          | `check-unused-code`           |
| `unnecessary-nullable`   | correctness  | no          | `check-unnecessary-nullable`  |
| `depend-on-referenced-packages` | correctness | yes | |
| `package-names`          | style        | yes         |                              |
| `secure-pubspec-urls`    | security     | yes         |                              |
| `sort-pub-dependencies`  | correctness  | no          |                              |
| `unused-dependencies`    | correctness  | no          |                              |
| `undeclared-dependencies` | correctness | no          |                              |
| `no-import-cycles`       | correctness  | no          |                              |
| `banned-imports`         | correctness  | no          |                              |

Notes:

- **Where they run.** Both `falcon check` and the language server run cross-file checks. The language server includes unsaved edits in open files.
- **Scope.** `unused-files` and `unused-code` only flag files/declarations under
  the package `lib/` directory (resolved from the nearest `pubspec.yaml`), while
  counting references from every analyzed file (including `test/`). Exclude
  generated code (`lib/gen/**`, `*.pb*.dart`, etc.) via `files.includes` the same
  way you would for any rule.
- **Package boundaries.** Each `pubspec.yaml` starts a package boundary. An unreadable or invalid manifest doesn't inherit the enclosing package's name.
- **`unnecessary-nullable`** checks private (`_`-prefixed) declarations whose call sites are visible in the project. It is opt-in; enable it under `cross-file.rules.correctness`. Use `overrides` to disable it for particular files.
- **`undeclared-dependencies`** overlaps `depend-on-referenced-packages`. When both are enabled for a file, `undeclared-dependencies` reports the shared cases and `depend-on-referenced-packages` keeps only its stricter `bin/` and build-hook checks.
- **`no-import-cycles`** reports one shortest concrete cycle per group of mutually importing libraries (plus self-imports), so a large tangle produces one diagnostic, not one per cycle.
- **`banned-imports`** does nothing until configured. Each entry in `options.rules` lists `files` globs, `deny` globs and a `message`:

  ```json
  "cross-file": {
    "rules": {
      "correctness": {
        "banned-imports": {
          "level": "error",
          "options": {
            "rules": [
              {
                "files": ["lib/domain/**"],
                "deny": ["package:flutter/**", "lib/data/**"],
                "message": "Keep domain code independent of Flutter and the data layer."
              }
            ]
          }
        }
      }
    }
  }
  ```

  Patterns are case-sensitive whole-string globs: `*` stays within one directory, `**` crosses directories. `files` matches paths relative to the nearest `pubspec.yaml` (including `lib/`). `deny` matches the import URI as written and, for relative or same-package imports, the normalized package-relative target. Conditional import alternatives are checked too; exports and parts are not. Both lists must be non-empty and `message` non-blank; invalid entries reject the configuration with the full option path. Each matching entry reports once per import URI, so a matching conditional alternative gets its own report. Per-path overrides replace the base options.

## Migrating from dart_code_linter / pyramid_lint

falcon replaces the `dart_code_linter` and `pyramid_lint` linters, and can
generate a `falcon.json` from an existing `analysis_options.yaml`:

```sh
# Print the generated config to stdout
falcon migrate --input analysis_options.yaml

# Write it to ./falcon.json
falcon migrate --write
```

Migration reads explicit rules from `dart_code_linter.rules`, legacy `custom_lint.rules`, and the analyzer's top-level `linter.rules` (list or map form). It doesn't expand `include:` presets or read `plugins:` configuration. Each mapped rule is emitted under its falcon group as `"warn"`; disabled entries become `"off"`, and supported option values are preserved.

The generated `linter.rules` sets `recommended: false`. If cross-file rules are migrated, their block also sets `recommended: false`; otherwise the block is omitted and falcon's recommended cross-file rules remain active. Add `"cross-file": { "enabled": false }` if you don't want those checks.

Notes:

- Recognized option keys are normalized to **kebab-case**, including nested keys. If both legacy and canonical spellings exist, the canonical value wins and a collision warning reports the discarded alias value. Unknown keys, invalid values, and options on rules that accept none are dropped with warnings naming their full configuration paths and discarded values. Migration writes a configuration that falcon can load; review the warnings to restore any intended settings with supported values.
- Upstream rules with no falcon equivalent are reported as warnings on stderr and
  omitted from the output.
- Rule and option warnings use canonical falcon paths after key rewrites.
  Unrecognized upstream rules retain their YAML source paths.
- The former twin rules (`no_empty_block`/`avoid_empty_blocks`, `no_magic_number`,
  `avoid_unused_parameters`) were unified into a single canonical rule each
  (`no-empty-block`, `no-magic-number`, `avoid-unused-parameters`); their upstream
  ids all still map to the surviving rule.
- A rule-level YAML `exclude` list becomes an override disabling that rule for
  those paths, preserving its declared group and section. An exclusion-only list
  is skipped with a warning rather than disabling the rule outside those paths.

### Upgrading an existing falcon.json

`migrate` also upgrades a `falcon.json` written for an older falcon: pass it as
the input and any legacy `snake_case` rule ids (and the removed twin ids) are
rewritten to their canonical kebab-case form, preserving levels and supported option values while normalizing option keys to kebab-case.
Duplicate twin entries collapse into the surviving rule, keeping the more severe
level. The input kind is auto-detected.

Upgrades preserve the order of configuration keys. Duplicate entries use the
same severity ranking and canonical-name tie breaking as normal config loading.

Unknown falcon rule ids, unsupported keys, and invalid values are dropped with
warnings naming their full configuration paths and discarded values. Known rules
in the wrong group or section are moved to their declared location, preserving
levels and supported options; collisions use the same severity policy described
above, and warnings name both paths.

Migration converts a single string `includes` into a one-element array when it
is a valid glob. Invalid include entries are dropped only while usable patterns
remain. If the original list had a positive pattern, at least one valid positive
pattern must survive; exclusion-only leftovers cannot replace that scope.
Invalid override scopes, including unsupported scope keys such as `excludes`,
discard the whole override with a warning naming its original index. Invalid
top-level `files` scope fails migration without writing. Intentionally empty or
omitted includes remain unchanged. Migration also refuses repairs that would
remove the root or a whole configuration section.

Other supported settings remain unchanged, and the resulting configuration passes
falcon's loader. Migration applies the loader's structural, rule-name, and options
checks. Malformed JSON or YAML still fails before writing.

Glob matching now uses the configuration file's directory in both the CLI and
the editor, rather than paths as walked. User-level `~/.falcon.json` globs instead
use the CLI current directory or LSP workspace root. Review filters when upgrading:
`!build/**` now excludes the project's build directory with `falcon check .`,
and `*` no longer spans directory separators. Replace `lib/*.dart` with
`lib/**/*.dart` when nested files should match. Existing `includes` glob text
is preserved; legacy flat `exclude_patterns` use the conversion described below.

```sh
# Rewrite legacy rule ids in place
falcon migrate --input falcon.json --write
```

## Migrating from the legacy flat schema

The old flat schema (`rules`, `exclude_patterns`, `severity_override` at the top
level) is no longer accepted by the loader. `falcon migrate --input falcon.json`
translates these keys using the mapping below and each rule's declared group and
section. Unmapped legacy rules or settings fail migration without writing,
rather than discarding the old configuration.

Legacy flat configurations enabled every rule unless explicitly disabled. The
current schema has no all-rules preset, so migration keeps the recommended
preset and explicitly listed rules, and warns how many unlisted rules no longer
run. Enable desired rules explicitly under `linter.rules` or `cross-file.rules`.

Severity overrides are case-insensitive; `warning` maps to `warn` and `note`
maps to `info`. They change the level without replacing rule options. Explicitly
disabled rules remain disabled, and their supported options are preserved.

| Legacy                                             | New                                                            |
|----------------------------------------------------|----------------------------------------------------------------|
| `"rules": { "x": { "enabled": false } }`           | `"linter": { "rules": { "<group>": { "x": "off" } } }`         |
| `"severity_override": { "x": "error" }`            | `"linter": { "rules": { "<group>": { "x": "error" } } }`       |
| `"rules": { "x": { "options": { ... } } }`         | `"linter": { "rules": { "<group>": { "x": { "level": "warn", "options": { ... } } } } }` |
| `"exclude_patterns": ["**/gen/**"]`                | `"files": { "includes": ["**", "!**/gen/**"] }`                |
| `"max_errors": 100`                                | `"max-errors": 100`                                            |

Legacy exclusions beginning with a single `*`, such as `*.g.dart`, become
recursive exclusions such as `!**/*.g.dart`. Patterns whose old matching cannot
be preserved, including a single `*` inside a path such as `lib/*.dart`, `?`,
or character classes such as `[ab]`, fail migration without writing.

The group for a rule is its category (`a11y`, `complexity`, `correctness`,
`performance`, `security`, `style`, `suspicious`); see the declarations in
`crates/falcon_rules/src/lint/<group>.rs` and `crates/falcon_rules/src/cross_file.rs`.
