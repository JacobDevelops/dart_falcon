# Configuring falcon (`falcon.json`)

falcon reads a biome 2.x-shaped `falcon.json`. Discovery order (first match wins):

1. `falcon.json` in the current directory
2. `falcon.json` at the enclosing git root
3. `~/.falcon.json`

If no config is found, falcon runs with defaults: every rule enabled at its
default severity (warning).

A config that **is** found but fails to load is a hard **error** — falcon prints
a message and exits non-zero rather than silently falling back to defaults. This
covers invalid JSON, wrong-typed values, `max-errors: 0` (see
[`max-errors`](#max-errors)), and the [legacy flat
schema](#migrating-from-the-legacy-flat-schema). Discovery and explicit
`--config` behave identically here, so a typo can never quietly re-enable every
rule.

### Unknown and legacy keys

Unknown top-level keys are **warned about by name** and then ignored, so a
mistyped section (`linterr` for `linter`, `cross_file` for `cross-file`) never
vanishes silently. The deprecated spellings `project` and `cross_file` (for
`cross-file`) and `max_errors` (for `max-errors`) still load, but each earns a
deprecation warning pointing at `falcon migrate`. The published JSON schema
validates only the canonical kebab-case keys — an editor will flag a legacy
spelling even though falcon still accepts it, which is your cue to migrate.

## Editor autocomplete (`$schema`)

Point the top-level `$schema` at the published JSON schema to get rule-name
autocomplete and validation in editors:

```json
"$schema": "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/main/schema/falcon.schema.json"
```

The schema is generated from falcon's rule metadata (`cargo xtask schema`) and
committed at [`schema/falcon.schema.json`](../schema/falcon.schema.json), so it
always lists the current rule set. The VS Code extension wires this up
automatically for any file named `falcon.json`.

## Full example

```json
{
  "$schema": "https://raw.githubusercontent.com/JacobDevelops/dart_falcon/main/schema/falcon.schema.json",
  "files": {
    "includes": ["**", "!.dart_tool/**", "!build/**", "!**/*.g.dart", "!**/*.freezed.dart"]
  },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "complexity": {
        "max-lines-for-file": "off"
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

- Plain entries are **positive includes** — only files matching at least one are
  linted. A list containing a bare catch-all (`**` or `**/*`), or an empty list,
  means "no positive filtering": every file is a candidate.
- Entries prefixed with `!` are **exclusions**, applied on top of the includes.

Exclusions and any CLI `--exclude` patterns are unioned.

Positive globs match paths **as walked from the CLI argument**, not as absolute
paths. Running `falcon check .` from the project root walks paths like
`lib/foo.dart`, so `lib/**` matches; passing an absolute path to `falcon check`
walks absolute paths, so a positive glob would need to be absolute to match.
A bare `**` (or `**/*`) entry disables positive filtering entirely.

## `linter`

- `enabled` (default `true`): when `false`, no rule runs — zero diagnostics.
- `domains`: per-domain gating. Keys are domain names (currently `flutter`);
  values are `all`, `recommended`, or `none`.
- `rules`: the recommended preset plus per-group rule levels.

### `linter.rules`

- `recommended` (default `true`): whether the recommended preset is active.
- Every other key is a **group** — one of `complexity`, `correctness`,
  `performance`, `style`, `suspicious` — mapping rule names to a configuration.

> **Rule ids are kebab-case.** Every rule id uses dashes (e.g.
> `max-lines-for-file`, `no-magic-number`). The pre-1.0 `snake_case` ids still
> resolve as aliases — an old id in `falcon.json` or a `// falcon-ignore` comment
> keeps working — but they are deprecated. Run `falcon migrate --input
> falcon.json` to rewrite them to the canonical ids (see
> [Migrating](#migrating-from-dart_code_linter--pyramid_lint)).

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
3. Otherwise, if the rule has domains: enabled if **any** of its domains resolves
   enabled. A missing domain key defaults to `recommended`. `all` → enabled;
   `recommended` → enabled iff the recommended preset is active; `none` →
   disabled.
4. Otherwise (no domains): enabled iff the recommended preset is active.

Net effect: with no config file, every rule is on at warning.

## `max-errors`

Optional cap on the number of reported diagnostics (`null` = unlimited). Must be
**at least 1**: `0` is rejected at load (it would suppress every diagnostic and
pass a run green with violations present). A CLI `--max-errors` flag overrides
the config value. The legacy `max_errors` (underscore) spelling still loads with
a deprecation warning.

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

### Metric thresholds (group `complexity`)

| Rule | Option | Default | Meaning |
|------|--------|---------|---------|
| `max-lines-for-file` | `max_lines` | `200` | Flag files with more than this many lines. The message states the actual configured threshold. |
| `max-lines-for-function` | `max_lines` | `100` | Flag functions/methods longer than this many lines. |
| `max-parameters-for-function` | `max_parameters` | `5` | Flag functions/methods with more than this many parameters. Constructors are not counted (matching dart_code_linter's number-of-parameters metric), and `copyWith` methods are exempt. |
| `max-switch-cases` | `max_cases` | `10` | Flag switch statements with more than this many non-default cases. |
| `cyclomatic-complexity` | `max_complexity` | `20` | Flag functions whose cyclomatic complexity (1 + decision points: `if`, ternary, `&&`, `\|\|`, `??`, loops, `catch`, non-default `case`, pattern `when` guards) exceeds this value. |
| `maximum-nesting-level` | `max_nesting` | `5` | Flag functions whose deepest nesting of control-flow blocks (`if`/`for`/`while`/`do`/`switch`/`try`) exceeds this value. |

### Identifier and naming rules

| Rule (group) | Option | Default | Meaning |
|--------------|--------|---------|---------|
| `prefer-correct-identifier-length` (`style`) | `min_length` | `3` | Flag identifiers shorter than this. Matches dart_code_linter's default. Scope is limited to variable/field declarations, getter/setter names and enum constants — parameters, catch clauses, for-each variables and plain function/method names are never checked. A single leading underscore is stripped before the length and exception checks. |
| | `max_length` | `300` | Flag identifiers longer than this. |
| | `exceptions` | `[]` | Names always allowed regardless of length (there is no built-in list). |
| `boolean-prefixes` (`style`) | `valid_prefixes` | `["is","are","was","were","has","have","had","can","should","will","do","does","did"]` | Accepted boolean-name prefixes (dart-pyramid-lint defaults). User entries **extend** the defaults. Only variable/field declarations with a boolean-*literal* initializer, and bool-returning methods/getters/functions, are checked; parameters and uninitialized fields are not. Names are matched with a single leading underscore stripped, and `@override` methods are exempt. |

### `prefer-moving-to-variable` (`complexity`)

| Option | Default | Meaning |
|--------|---------|---------|
| `allowed_duplicated_chains` | `2` | The occurrence index at which a repeated expression is flagged. `2` flags the 2nd and later duplicates; `3` flags the 3rd and later. Values below `2` are clamped to `2`. |

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
| `widgets_order` | — | For a class that `extends State`, orders lifecycle members by this list. Supported tokens: `constructor`, `init-state`, `did-change-dependencies`, `did-update-widget`, `dispose`, `build`, `overridden-methods`. |

> **`widgets_order` is best-effort.** falcon has no type resolution, so State
> detection keys on the syntactic `extends State`/`State<T>` clause, and only the
> relative order of the recognized lifecycle members is checked;
> `overridden-methods` matches any other `@override` method.

`class-members-ordering` (`style`) accepts the same `order` option with the same
category tokens; it differs from `member-ordering` only in its default sequence.

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

A partial rule block for file rules (`linter`) or cross-file rules (`cross-file`;
the legacy keys `project` and `cross_file` are still accepted), respectively. Both have the same
shape as `linter.rules`: rule levels, per-rule
`options`, and an optional `enabled` master switch are honored — but no
`domains`, no nested `overrides`, no `files`. An override may carry either or
both sections; each patches the correspondingly-named base block.

- Each explicit rule entry (under its group) **replaces** the base resolution
  for that rule on matching files: `off` disables it; `on`/`info`/`warn`/`error`
  enables it at that severity — even turning on a rule the base config disabled.
- A rule entry's `options` block **replaces** the base rule's options on matching
  files (options are not deep-merged). An override that sets only a level leaves
  the base options intact.
- `"enabled": false` disables every rule in that section for matching files (a
  later override may re-enable a specific one).

### Ordering

For a given file, every override whose `includes` match applies **in order**:
later overrides win over earlier ones, and all win over the base config. A rule
is registered (and run) if it is enabled for *any* path — base or override — so
an override can re-enable a rule the base config turned off.

### Path-matching caveat

Overrides match the file path **as walked**, exactly like `files.includes`.
Running `falcon check .` from the project root walks paths like `test/foo.dart`,
so `test/**` matches; passing an absolute path (or LSP, which resolves document
URIs to absolute paths) walks absolute paths, so a glob must be absolute or use
a leading `**/` (e.g. `**/theme.dart`) to match.

### Per-path options

An override may carry rule `options`, letting a rule run with different options on
different paths — for example a stricter `max-lines-for-file` under `test/`:

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
<reason>` comment (the reason is mandatory) — or `// falcon-ignore-all` for a
whole file. The full grammar, placement rules, stacking, and error cases are
documented in **[suppressions.md](./suppressions.md)**.

```dart
dynamic payload = decode(bytes); // falcon-ignore lint/suspicious/avoid-dynamic: interop boundary
```

## `cross-file` — cross-file rules

Most rules analyze one file at a time. A small set of **cross-file rules** instead
reason across the whole analyzed file set — they need to see every file to decide
whether something is referenced anywhere. They are a **separate feature** from the
linter and live under their own top-level `cross-file` block, *not* under
`linter`. The pre-1.0 spellings `project` and `cross_file` (underscore) are still
accepted as deprecated aliases (each warns on load — run `falcon migrate`):

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
- `rules`: the recommended preset plus per-group rule levels — the **same shape**
  as `linter.rules` (level strings or `{ "level", "options" }` objects), but with
  **no `domains`** gating, since cross-file rules are not domain-scoped.

Cross-file rules are grouped under their category (all three are `correctness`),
share the same metadata table as file rules, and are suppressible with
`// falcon-ignore cross-file/<group>/<rule>: <reason>` comments (or the
`// falcon-ignore-all` variant). Configuring a cross-file rule under
`linter.rules` (or a file rule under `cross-file.rules`) is a mistake — falcon
warns and steers you to the right section, and the misplaced entry does not take
effect.

| Rule                     | Group        | Recommended | Replaces (dart_code_linter)   |
|--------------------------|--------------|-------------|-------------------------------|
| `unused-files`           | correctness  | yes         | `check-unused-files`          |
| `unused-code`            | correctness  | yes         | `check-unused-code`           |
| `unnecessary-nullable`   | correctness  | yes         | `check-unnecessary-nullable`  |

Notes:

- **Where they run.** Cross-file rules run in the `falcon check` pipeline's
  cross-file pass, after the per-file pass, over every collected file. The **LSP
  server also runs them**: it walks the workspace (open buffers overlaid on the
  on-disk files) on didOpen/didSave/config-reload — not on every keystroke — and
  republishes the merged per-file plus cross-file diagnostics for open documents.
- **Scope.** `unused-files` and `unused-code` only flag files/declarations under
  the package `lib/` directory (resolved from the nearest `pubspec.yaml`), while
  counting references from every analyzed file (including `test/`). Exclude
  generated code (`lib/gen/**`, `*.pb*.dart`, etc.) via `files.includes` the same
  way you would for any rule.
- **`unnecessary-nullable` is resolver-backed** and on by the recommended preset.
  It still restricts itself to `_`-prefixed private declarations (so every call
  site is visible in-project), but falcon's type-resolution layer now decides
  whether each argument can be null: local type inference recognizes structurally
  non-null forms (literals, `new`, arithmetic), and a cross-file return-type index
  resolves a callee's or getter's declared return type. An argument proven
  non-null no longer counts as "passes null", which removed the false positives
  that had kept the rule opt-in. It remains a cross-file rule (cross-file
  pass). Per-file exclusions (the old `--exclude` flags) are expressible with
  `overrides`.

## Migrating from dart_code_linter / pyramid_lint

falcon replaces the `dart_code_linter` and `pyramid_lint` linters, and can
generate a `falcon.json` from an existing `analysis_options.yaml` — the same idea
as `biome migrate eslint/prettier`:

```sh
# Print the generated config to stdout
falcon migrate --input analysis_options.yaml

# Write it to ./falcon.json
falcon migrate --write
```

Rules under the `dart_code_linter:` block are matched against their upstream
dart_code_linter ids; rules under `custom_lint:` (how pyramid_lint is configured)
are matched against their pyramid_lint ids. Each mapped rule is emitted under its
falcon group as `"warn"`; disabled entries (`- rule: false`) become `"off"`, and
entries with options become `{ "level": "warn", "options": { ... } }`.

The migration is **explicit**: `recommended` is set to `false` in the output so
only the rules present in your `analysis_options.yaml` are active (like biome).
Notes:

- Option **key names are passed through verbatim** and may need manual review —
  falcon's option schema is not guaranteed to match the upstream linter's.
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
level) is no longer accepted — falcon reports an error rather than silently
ignoring it. Migrate as follows:

| Legacy                                             | New                                                            |
|----------------------------------------------------|----------------------------------------------------------------|
| `"rules": { "x": { "enabled": false } }`           | `"linter": { "rules": { "<group>": { "x": "off" } } }`         |
| `"severity_override": { "x": "error" }`            | `"linter": { "rules": { "<group>": { "x": "error" } } }`       |
| `"rules": { "x": { "options": { ... } } }`         | `"linter": { "rules": { "<group>": { "x": { "level": "warn", "options": { ... } } } } }` |
| `"exclude_patterns": ["**/gen/**"]`                | `"files": { "includes": ["**", "!**/gen/**"] }`                |
| `"max_errors": 100`                                | `"max-errors": 100`                                            |

The group for a rule is its category (`complexity`, `correctness`,
`performance`, `style`, `suspicious`); see `crates/falcon_rules/src/meta.rs`.
