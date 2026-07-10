# Configuring falcon (`falcon.json`)

falcon reads a biome 2.x-shaped `falcon.json`. Discovery order (first match wins):

1. `falcon.json` in the current directory
2. `falcon.json` at the enclosing git root
3. `~/.falcon.json`

If no config is found, falcon runs with defaults: every rule enabled at its
default severity (warning).

## Full example

```json
{
  "$schema": "https://example.invalid/falcon-schema.json",
  "files": {
    "includes": ["**", "!.dart_tool/**", "!build/**", "!**/*.g.dart", "!**/*.freezed.dart"]
  },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "complexity": {
        "max_lines_for_file": "off"
      },
      "style": {
        "prefer-trailing-comma": { "level": "error", "options": {} }
      }
    },
    "domains": { "flutter": "recommended" }
  },
  "project": {
    "enabled": true,
    "rules": {
      "correctness": {
        "unused-files": "warn",
        "unused-code": "warn",
        "unnecessary-nullable": "off"
      }
    }
  },
  "max_errors": null
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

## `max_errors`

Optional cap on the number of reported diagnostics (`null` = unlimited). A CLI
`--max-errors` flag overrides the config value.

## Rule options

Some rules accept an `options` object under the `{ "level": ..., "options": {…} }`
form. Options are read leniently: a missing or malformed value falls back to the
rule's default, and a bad option never aborts a run.

```json
{
  "linter": {
    "rules": {
      "complexity": {
        "max_lines_for_file": { "level": "warn", "options": { "max_lines": 400 } },
        "cyclomatic_complexity": { "level": "warn", "options": { "max_complexity": 15 } }
      }
    }
  }
}
```

Option names use `snake_case` inside the `options` object.

### Metric thresholds (group `complexity`)

| Rule | Option | Default | Meaning |
|------|--------|---------|---------|
| `max_lines_for_file` | `max_lines` | `200` | Flag files with more than this many lines. The message states the actual configured threshold. |
| `max_lines_for_function` | `max_lines` | `100` | Flag functions/methods longer than this many lines. |
| `max_parameters_for_function` | `max_parameters` | `5` | Flag functions/methods with more than this many parameters. Constructors are not counted (matching dart_code_linter's number-of-parameters metric), and `copyWith` methods are exempt. |
| `max_switch_cases` | `max_cases` | `10` | Flag switch statements with more than this many non-default cases. |
| `cyclomatic_complexity` | `max_complexity` | `20` | Flag functions whose cyclomatic complexity (1 + decision points: `if`, ternary, `&&`, `\|\|`, `??`, loops, `catch`, non-default `case`, pattern `when` guards) exceeds this value. |
| `maximum_nesting_level` | `max_nesting` | `5` | Flag functions whose deepest nesting of control-flow blocks (`if`/`for`/`while`/`do`/`switch`/`try`) exceeds this value. |

### Identifier and naming rules

| Rule (group) | Option | Default | Meaning |
|--------------|--------|---------|---------|
| `prefer-correct-identifier-length` (`style`) | `min_length` | `3` | Flag identifiers shorter than this. Matches dart_code_linter's default. Scope is limited to variable/field declarations, getter/setter names and enum constants — parameters, catch clauses, for-each variables and plain function/method names are never checked. A single leading underscore is stripped before the length and exception checks. |
| | `max_length` | `300` | Flag identifiers longer than this. |
| | `exceptions` | `[]` | Names always allowed regardless of length (there is no built-in list). |
| `boolean_prefixes` (`style`) | `valid_prefixes` | `["is","are","was","were","has","have","had","can","should","will","do","does","did"]` | Accepted boolean-name prefixes (dart-pyramid-lint defaults). User entries **extend** the defaults. Only variable/field declarations with a boolean-*literal* initializer, and bool-returning methods/getters/functions, are checked; parameters and uninitialized fields are not. Names are matched with a single leading underscore stripped, and `@override` methods are exempt. |

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
          "complexity": { "max_lines_for_function": "off" }
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

### `overrides[].linter` and `overrides[].project`

A partial rule block for file rules (`linter`) or project rules (`project`),
respectively. Both have the same shape: only rule levels and an optional
`enabled` master switch are honored — overrides are **rule-level only** (no
`domains`, no nested `overrides`, no `files`). An override may carry either or
both sections; each patches the correspondingly-named base block.

- Each explicit rule entry (under its group) **replaces** the base resolution
  for that rule on matching files: `off` disables it; `on`/`info`/`warn`/`error`
  enables it at that severity — even turning on a rule the base config disabled.
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

### Options limitation

Rule **options** in an override are **not yet supported** and are rejected at
load with a clear error. Options remain global (configured under `linter.rules`);
an override may re-scope a rule's level (on/off/severity) per path but not its
options. Path-scoped options may be added later.

## Suppressing diagnostics

`falcon.json` turns whole rules on or off. To silence a single occurrence
instead, use an inline comment — falcon honors the same `// ignore:` syntax as
the Dart analyzer, so existing comments carry over unchanged.

- **Same line** — a comment after code suppresses the listed rules on that line:

  ```dart
  dynamic x = 1; // ignore: avoid-dynamic
  ```

- **Next line** — a comment alone on its line suppresses the line below it:

  ```dart
  // ignore: avoid-dynamic
  dynamic x = 1;
  ```

- **Whole file** — `ignore_for_file` suppresses the listed rules anywhere in the
  file, wherever the comment appears (conventionally at the top):

  ```dart
  // ignore_for_file: avoid-dynamic, prefer-const-constructors
  ```

Details:

- List multiple rules separated by commas: `// ignore: rule-a, rule-b`.
- Rule names are matched **exactly** against falcon rule names as registered
  (e.g. `avoid-dynamic`); unknown names are ignored harmlessly.
- The no-space form `//ignore:` is accepted, as are extra slashes (`/// ignore:`).
- Only `//`-style line comments count; a directive inside a string literal or a
  `/* block comment */` is not treated as a suppression.

## `project` — project-level (cross-file) rules

Most rules analyze one file at a time. A small set of **project rules** instead
reason across the whole analyzed file set — they need to see every file to decide
whether something is referenced anywhere. They are a **separate feature** from the
linter and live under their own top-level `project` block, *not* under `linter`:

```json
"project": {
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

- `enabled` (default `true`): when `false`, no project rule runs.
- `rules`: the recommended preset plus per-group rule levels — the **same shape**
  as `linter.rules` (level strings or `{ "level", "options" }` objects), but with
  **no `domains`** gating, since project rules are not domain-scoped.

Project rules are grouped under their category (all three are `correctness`),
share the same metadata table as file rules, and are suppressible with the same
`// ignore:` / `// ignore_for_file:` comments. Configuring a project rule under
`linter.rules` (or a file rule under `project.rules`) is a mistake — falcon warns
and steers you to the right section, and the misplaced entry does not take effect.

| Rule                     | Group        | Recommended | Replaces (dart_code_linter)   |
|--------------------------|--------------|-------------|-------------------------------|
| `unused-files`           | correctness  | yes         | `check-unused-files`          |
| `unused-code`            | correctness  | yes         | `check-unused-code`           |
| `unnecessary-nullable`   | correctness  | no          | `check-unnecessary-nullable`  |

Notes:

- **CLI-only.** Project rules run in the `falcon check` pipeline's project pass,
  after the per-file pass, over every collected file. The **LSP server does not
  run them**: it analyzes a single open buffer and has no whole-project view, so
  a cross-file rule cannot be evaluated soundly there. This is by design — the
  editor keeps showing per-file diagnostics; run `falcon check` for project rules.
- **Scope.** `unused-files` and `unused-code` only flag files/declarations under
  the package `lib/` directory (resolved from the nearest `pubspec.yaml`), while
  counting references from every analyzed file (including `test/`). Exclude
  generated code (`lib/gen/**`, `*.pb*.dart`, etc.) via `files.includes` the same
  way you would for any rule.
- **`unnecessary-nullable` is heuristic** (hence off by the recommended preset).
  Without type resolution it can only see `null` *literals* at call sites, so a
  nullable value forwarded through a variable is not counted as "passes null".
  Enable it deliberately and review its findings. Per-file exclusions (the old
  `--exclude` flags) are expressible with `overrides`.

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
| `"max_errors": 100`                                | unchanged (`"max_errors": 100`)                                |

The group for a rule is its category (`complexity`, `correctness`,
`performance`, `style`, `suspicious`); see `crates/falcon_rules/src/meta.rs`.
