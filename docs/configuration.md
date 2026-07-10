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
