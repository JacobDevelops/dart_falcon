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

Unknown top-level keys are **warned about by name** and then ignored, so a
mistyped section (`linterr` for `linter`, `crossfile` for `cross-file`) never
vanishes silently. The deprecated spellings `project` and `cross_file` (for
`cross-file`) and `max_errors` (for `max-errors`) still load, but each earns a
deprecation warning pointing at `falcon migrate`, including when the legacy
spelling appears inside an `overrides` entry. The published JSON schema
validates only the canonical kebab-case keys, so an editor will flag a legacy
spelling even though falcon still accepts it, which is your cue to migrate.

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
  means "no positive filtering": every file is a candidate.
- Entries prefixed with `!` are **exclusions**, applied on top of the includes.

Exclusions and any CLI `--exclude` patterns are unioned.

Invalid glob patterns produce a warning and are skipped consistently in CLI
file selection, LSP selection, and overrides. If no valid positive patterns
remain, positive filtering is unrestricted; valid exclusions still win.

Positive globs match paths **as walked from the CLI argument**, not as absolute
paths. Running `falcon check .` from the project root walks paths like
`lib/foo.dart`, so `lib/**` matches; passing an absolute path to `falcon check`
walks absolute paths, so a positive glob would need to be absolute to match.
A bare `**` (or `**/*`) entry disables positive filtering entirely.

## `linter`

- `enabled` (default `true`): when `false`, no file rule runs. Cross-file checks are configured separately.
- `domains`: per-domain gating. Keys are domain names (`flutter`, `test`,
  `bloc`, `riverpod`, `provider`, `flutter_hooks`, `equatable`); values are
  `all`, `recommended`, or `none`.
- `rules`: falcon's recommended set plus per-group rule levels.

### Language server reloads

The language server reloads configuration and mutable dependencies when the editor sends watched-file notifications. This includes path and git dependencies, Flutter SDK packages, and newly generated Dart files.

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

Optional cap on reported diagnostics (`null` = unlimited). In `falcon.json`, the value must be **at least 1**; `0` is rejected at load. The CLI `--max-errors` flag overrides the config value. Use a positive value: the CLI currently accepts `--max-errors 0`, which hides every diagnostic and can return a clean exit code even when violations exist. The legacy config key `max_errors` still loads with a deprecation warning.

## Rule options

Some rules accept an `options` object under the `{ "level": ..., "options": {…} }`
form. Options are read leniently: a missing or malformed value falls back to the
rule's default, and a bad option never aborts a run.

```json
{
  "linter": {
    "rules": {
      "complexity": {
        "max-lines-for-file": { "level": "warn", "options": { "max_lines": 400 } },
        "cyclomatic-complexity": { "level": "warn", "options": { "max_complexity": 15 } }
      }
    }
  }
}
```

Option names use `snake_case` inside the `options` object.

### Banned declarations and API usage (group `suspicious`)

`avoid-banned-names`, `avoid-banned-types`, `avoid-banned-annotations`,
`avoid-banned-exports` and `banned-usage` do nothing until configured.
Like `banned-imports`, each entry in `options.rules` contains a `deny` match
list and a nonblank `message`. Matches are case-sensitive globs; malformed
entries are ignored.

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

### Suspicious expression rules

| Rule | Option | Default | Meaning |
|------|--------|---------|---------|
| `avoid-nested-assignments` | `ignore_null_aware_assignments` | `false` | Exempt nested `??=` assignments. |
| `avoid-negated-conditions` | `ignore_null_checks` | `false` | Exempt negated null checks. |
| `no-magic-string` | `allow_only_once` | `false` | Allow a string value that occurs only once in the file. |
| `no-magic-string` | `ignored_invocations` | `["print", "debugPrint"]` | Invocation names whose direct string arguments are exempt. |
| `avoid-referencing-subclasses` | `ignored_annotations` | `["freezed", "Freezed", "unfreezed"]` | Replaces the defaults; exempt classes by exact, case-sensitive final annotation name segment. |

### Equatable selection

`extend-equatable` (`style`, `equatable` domain) does nothing until configured.
Set `class_name_pattern` and/or `file_name_pattern` to regular expressions selecting
value classes that should extend Equatable. Either matching pattern selects a
class; file patterns match the file basename, not its full path. Missing or invalid
patterns select nothing. For example:

```json
"extend-equatable": {
  "level": "warn",
  "options": { "class_name_pattern": "State$" }
}
```

### Metric thresholds (group `complexity`)

| Rule | Option | Default | Meaning |
|------|--------|---------|---------|
| `max-lines-for-file` | `max_lines` | `200` | Flag files with more than this many lines. The message states the actual configured threshold. |
| `max-lines-for-function` | `max_lines` | `100` | Flag functions/methods longer than this many lines. |
| `max-parameters-for-function` | `max_parameters` | `5` | Flag functions/methods with more than this many parameters. Constructors are not counted (matching dart_code_linter's number-of-parameters metric), and `copyWith` methods are exempt. |
| `max-switch-cases` | `max_cases` | `10` | Flag switch statements with more than this many non-default cases. |
| `cyclomatic-complexity` | `max_complexity` | `20` | Flag functions whose cyclomatic complexity (1 + decision points: `if`, ternary, `&&`, `\|\|`, `??`, loops, `catch`, non-default `case`, pattern `when` guards) exceeds this value. |
| `maximum-nesting-level` | `max_nesting` | `5` | Flag functions whose deepest nesting of control-flow blocks (`if`/`for`/`while`/`do`/`switch`/`try`) exceeds this value. |
| `avoid-long-records` | `max_fields` | `6` | Limit positional and named fields together in record values and types. |
| `avoid-complex-arithmetic-expressions` | `max_terms` | `6` | Limit connected numeric arithmetic terms; calls are opaque and parentheses do not reset the count. |
| `avoid-if-with-many-branches` | `max_branches` | `3` | Limit branches in an if/else-if chain, including a final else. |
| `avoid-nested-switches` | `max_nesting` | `1` | Limit nested switch statements; nested callable bodies restart the count. |
| `avoid-nested-switch-expressions` | `max_nesting` | `1` | Limit nested switch expressions; nested callable bodies restart the count. |
| `avoid-nested-try-statements` | `max_nesting` | `2` | Limit nested try statements; nested callable bodies restart the count. |
| `max-statements` | `max_statements` | `20` | Limit statements per function body, excluding blocks, labels, empty statements, and local function declarations; nested bodies are independent. |
| `cognitive-complexity` | `max_complexity` | `15` | Limit nesting-weighted decisions, catches, alternatives, and logical operator sequences; nested callable bodies are independent. |
| `widget-tree-depth` | `max_nesting` | `8` | Limit resolved Flutter widget construction depth, counting the root as one and excluding SizedBox. Lists, parentheses, and conditional branches add no levels. Helper calls, callbacks, and extracted references establish independent trees. |
| `avoid-complex-conditions` | `threshold` | `10` | Flag a connected `&&`/`\|\|` expression with more operands than this. Parentheses and `!` keep the count; any other expression is one operand. |
| `avoid-complex-loop-conditions` | `threshold` | `1` | Same counting for `while`, `do`-`while` and C-style `for` conditions, including collection `for`. For-in iterables, initializers and updaters are not counted. |

### Test registration helpers

`avoid-empty-test-groups` (`correctness`) and `prefer-unique-test-names` (`suspicious`) accept `include_methods` (array of strings, default `[]`). List custom test registration helper names, for example `"include_methods": ["testCase"]`, to count these calls as tests in addition to canonical test-framework registrations.

### Additional test assertions

`missing-test-assertion` (`correctness`, `test` domain) accepts
`include_assertions` (array of strings, default `[]`). These names extend the
built-in framework expectations and mock verification calls with trusted project
assertion helpers, including local functions. Names match their exact call-site
spelling; include the prefix for prefixed calls.

```json
"missing-test-assertion": {
  "level": "warn",
  "options": { "include_assertions": ["assertResult", "helpers.verifyState"] }
}
```

Only `_test.dart` files importing `test` or `flutter_test` are checked, and the
test registration must resolve to a framework package. Assertions in nested
tests or unused local closures and functions do not satisfy the enclosing test.

### Identifier and naming rules

| Rule (group) | Option | Default | Meaning |
|--------------|--------|---------|---------|
| `prefer-correct-identifier-length` (`style`) | `min_length` | `3` | Flag identifiers shorter than this. Checks variable/field declarations, getter/setter names and enum constants, not parameters, catch clauses, for-each variables or plain function/method names. A single leading underscore is stripped before the length and exception checks. |
| | `max_length` | `300` | Flag identifiers longer than this. |
| | `exceptions` | `[]` | Names always allowed regardless of length (there is no built-in list). |
| `boolean-prefixes` (`style`) | `valid_prefixes` | `["is","are","was","were","has","have","had","can","should","will","do","does","did"]` | Accepted boolean-name prefixes (dart-pyramid-lint defaults). User entries **extend** the defaults. Only variable/field declarations with a boolean-*literal* initializer, and bool-returning methods/getters/functions, are checked; parameters and uninitialized fields are not. Names are matched with a single leading underscore stripped, and `@override` methods are exempt. |
| `prefer-correct-error-name` (`style`) | `allowed_names` | unset | Accepted exception parameter names in catch clauses and resolved SDK Future/Stream error callbacks. Inactive until a nonempty list is configured. |
| | `ignored_names` | `[]` | Additional error parameter names to ignore. |
| `prefer-correct-handler-name` (`style`) | `name_pattern` | unset | Regular expression required for callback handler method names, ignoring a privacy underscore. Inactive until a valid nonempty pattern is configured; inherited contracts and annotated methods are excluded. |
| | `ignore_public` | `false` | Exclude public handler methods. |
| `prefer-correct-callback-field-name` (`style`) | `name_pattern` | unset | Regular expression identifying callback fields. Function fields must match and non-function fields must not match. Inactive until a valid nonempty pattern is configured; `_` is ignored. |

### `prefer-named-parameters` (`style`)

| Option | Default | Meaning |
|--------|---------|---------|
| `max_number` | `1` | Maximum positional parameters before named parameters are preferred. Inherited method contracts, extension types, closures, setters and operators are excluded. |

The `prefer-boolean-prefixes` rule id is an alias for `boolean-prefixes` and uses the same options and declaration scope. Like the other legacy ids, the JSON schema only lists the canonical `boolean-prefixes`, so prefer that id in new configs.

### `prefer-commenting-analyzer-ignores` (`style`)

| Option | Default | Meaning |
|--------|---------|---------|
| `ignore_global` | `true` | Exclude `ignore_for_file:` directives. Set to `false` to require explanations for both file-wide and line-specific analyzer ignores. |

### `move-variable-closer-to-its-usage` (`performance`)

| Option | Default | Meaning |
|--------|---------|---------|
| `ignore_const` | `false` | Exclude const declarations. Only immutable scalar-literal declarations are considered. |
| `ignore_after_if` | `true` | Skip declarations used only after an intervening if statement. Set to `false` to recommend moving these declarations as well as declarations used in one branch. |

### `prefer-moving-to-variable` (`complexity`)

| Option | Default | Meaning |
|--------|---------|---------|
| `allowed_duplicated_chains` | `2` | The occurrence index at which a repeated expression is flagged. `2` flags the 2nd and later duplicates; `3` flags the 3rd and later. Values below `2` are clamped to `2`. |

### `prefer-container` (`style`, `flutter`)

| Option | Default | Meaning |
|--------|---------|---------|
| `min_sequence` | `3` | How many directly nested Container-compatible widgets (e.g. `Padding` → `Align` → `DecoratedBox`) are needed before suggesting one `Container`. Values below `3` are raised to `3`. Only the outermost widget of a run is reported. |

### `format-comment` (`style`)

Checks that comments read like sentences (start upper-case, end with `.`/`!`/`?`/`:`).
Consecutive comment lines are treated as one block: a multi-line block is joined
and split into sentences, so a sentence that wraps across lines is judged as a
whole and continuation lines are never flagged on their own. Both `//` line
comments and `///` doc comments are checked unless `only_doc_comments` is set.

| Option | Default | Meaning |
|--------|---------|---------|
| `only_doc_comments` | `false` | When `true`, only `///` doc comments are checked. |
| `ignored_patterns` | `[]` | List of regular expressions; a comment matching any of them is skipped. Invalid patterns are ignored. |

### `no-magic-number` (`style`)

Flags numeric literals that are not extracted to a named constant. Literals are
exempt when they are in the allow-list, inside a variable/field initializer, a
collection literal, a const map or const constructor, a `DateTime` constructor,
or used directly as an index.

| Option | Default | Meaning |
|--------|---------|---------|
| `allowed` | `[-1, 0, 1]` | Numeric values that are never considered magic. |

### `prefer-extracting-callbacks` (`complexity`, `flutter`)

Flags block-body function-expression callbacks passed as arguments to widget
constructors inside a `Widget`/`State` subclass. Arrow (`=>`) callbacks, empty
blocks and Flutter builders (first parameter `BuildContext`) are never flagged.

| Option | Default | Meaning |
|--------|---------|---------|
| `allowed_line_count` | _none_ | When set, only callbacks spanning more than this many lines are flagged; unset flags every qualifying callback. |
| `ignored_named_arguments` | `[]` | Named-argument labels whose callbacks are ignored. |

### `member-ordering` (`style`)

Without options, the built-in order applies (static const → static fields →
instance fields → constructors → static methods → instance methods).

| Option | Default | Meaning |
|--------|---------|---------|
| `order` | built-in | List of category tokens giving the required member sequence. Supported tokens: `public-fields`, `private-fields`, `fields`, `static-fields`, `constructors`, `named-constructors`, `static-methods`, `private-methods`, `public-methods`, `methods`, `getters`, `setters`. A member is ranked by the earliest token in the list it qualifies for; members matching no token are ignored. |
| `widgets_order` | none | For a class that `extends State`, orders lifecycle members by this list. Supported tokens: `constructor`, `init-state`, `did-change-dependencies`, `did-update-widget`, `dispose`, `build`, `overridden-methods`. |

> `widgets_order` detects `State` from the written `extends State`/`State<T>` clause. Only recognized lifecycle members are ordered; `overridden-methods` matches any other `@override` method.

`class-members-ordering` (`style`) accepts the same `order` option with the same
category tokens; it differs from `member-ordering` only in its default sequence.

### `avoid-unnecessary-getter` (`style`)

| Option | Default | Meaning |
|--------|---------|---------|
| `ignore_upcast` | `true` | Preserve getters whose return type differs from their private final backing field. Set to `false` to report these getters too. |

### Name-ordering rules (`style`)

`enum-constants-ordering`, `map-keys-ordering`, `parameters-ordering`,
`pattern-fields-ordering`, and `record-fields-ordering` share the following option.
Unlike DCM's alphabetical defaults, these config-driven falcon rules do nothing
unless `order` is configured.

| Option | Default | Meaning |
|--------|---------|---------|
| `order` | none | `"ascending"` or `"descending"` lexicographic, case-sensitive name order. |

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

Each block accepts an optional `enabled` switch and a `rules` object shaped like `linter.rules`, with grouped rule levels and options. Neither block accepts `domains`, nested `overrides`, or `files`. An override may carry either or both blocks. The legacy cross-file keys `project` and `cross_file` still load.

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

### Path-matching caveat

Overrides match the file path **as walked**, exactly like `files.includes`.
Running `falcon check .` from the project root walks paths like `test/foo.dart`,
so `test/**` matches; passing an absolute path (or LSP, which resolves document
URIs to absolute paths) walks absolute paths, so a glob must be absolute or use
a leading `**/` (e.g. `**/theme.dart`) to match.

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
          "max-lines-for-file": { "level": "warn", "options": { "max_lines": 1000 } }
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

Cross-file rules are grouped under their category (`correctness`, `style`, or `suspicious`),
share the same metadata table as file rules, and are suppressible with
`// falcon-ignore cross-file/<group>/<rule>: <reason>` comments (or the
`// falcon-ignore-all` variant). Configuring a cross-file rule under
`linter.rules` (or a file rule under `cross-file.rules`) is a mistake: falcon
warns and steers you to the right section, and the misplaced entry does not take
effect.

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

  Patterns are case-sensitive whole-string globs: `*` stays within one directory, `**` crosses directories. `files` matches paths relative to the nearest `pubspec.yaml` (including `lib/`). `deny` matches the import URI as written and, for relative or same-package imports, the normalized package-relative target. Conditional import alternatives are checked too; exports and parts are not. Both lists must be non-empty and `message` non-blank, otherwise the entry is ignored. Each matching entry reports once per import URI, so a matching conditional alternative gets its own report. Per-path overrides replace the base options.

## Migrating from dart_code_linter / pyramid_lint

falcon replaces the `dart_code_linter` and `pyramid_lint` linters, and can
generate a `falcon.json` from an existing `analysis_options.yaml`:

```sh
# Print the generated config to stdout
falcon migrate --input analysis_options.yaml

# Write it to ./falcon.json
falcon migrate --write
```

Migration reads explicit rules from `dart_code_linter.rules`, legacy `custom_lint.rules`, and the analyzer's top-level `linter.rules` (list or map form). It doesn't expand `include:` presets or read `plugins:` configuration. Each mapped rule is emitted under its falcon group as `"warn"`; disabled entries become `"off"`, and options are preserved.

The generated `linter.rules` sets `recommended: false`. If cross-file rules are migrated, their block also sets `recommended: false`; otherwise the block is omitted and falcon's recommended cross-file rules remain active. Add `"cross-file": { "enabled": false }` if you don't want those checks.

Notes:

- Option **key names are passed through verbatim** and may need manual review. falcon's option schema is not guaranteed to match the upstream linter's.
- Upstream rules with no falcon equivalent are reported as warnings on stderr and
  omitted from the output.
- The former twin rules (`no_empty_block`/`avoid_empty_blocks`, `no_magic_number`,
  `avoid_unused_parameters`) were unified into a single canonical rule each
  (`no-empty-block`, `no-magic-number`, `avoid-unused-parameters`); their upstream
  ids all still map to the surviving rule.

### Upgrading an existing falcon.json

`migrate` also upgrades a `falcon.json` written for an older falcon: pass it as
the input and any legacy `snake_case` rule ids (and the removed twin ids) are
rewritten to their canonical kebab-case form, preserving levels and options.
Duplicate twin entries collapse into the surviving rule, keeping the more severe
level. The input kind is auto-detected.

```sh
# Rewrite legacy rule ids in place
falcon migrate --input falcon.json --write
```

## Migrating from the legacy flat schema

The old flat schema (`rules`, `exclude_patterns`, `severity_override` at the top
level) is no longer accepted. falcon reports an error rather than silently
ignoring it. Migrate as follows:

| Legacy                                             | New                                                            |
|----------------------------------------------------|----------------------------------------------------------------|
| `"rules": { "x": { "enabled": false } }`           | `"linter": { "rules": { "<group>": { "x": "off" } } }`         |
| `"severity_override": { "x": "error" }`            | `"linter": { "rules": { "<group>": { "x": "error" } } }`       |
| `"rules": { "x": { "options": { ... } } }`         | `"linter": { "rules": { "<group>": { "x": { "level": "warn", "options": { ... } } } } }` |
| `"exclude_patterns": ["**/gen/**"]`                | `"files": { "includes": ["**", "!**/gen/**"] }`                |
| `"max_errors": 100`                                | `"max-errors": 100`                                            |

The group for a rule is its category (`a11y`, `complexity`, `correctness`,
`performance`, `security`, `style`, `suspicious`); see `crates/falcon_rules/src/meta.rs`.
