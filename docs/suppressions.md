# Suppressions

`falcon.json` turns whole rules on or off across your project. To silence a
**single occurrence** instead, annotate the code with a `// falcon-ignore`
comment. The shape is modelled on Biome's `// biome-ignore lint/<group>/<rule>:
<reason>`, and — like Biome — **the reason is mandatory**.

For turning rules off wholesale, per group, or per path, see
[configuration.md](./configuration.md); for the rule ids and groups you name in a
directive, see the [rules index](/linter/rules).

## Syntax

```
// falcon-ignore <section>/<group>/<rule>: <reason>
// falcon-ignore-all <section>/<group>/<rule>: <reason>
```

Each directive names **exactly one rule** by its full path and ends with a
non-empty reason after the colon:

- **`<section>`** — `lint` for a normal per-file rule, `cross-file` for a
  whole-project rule. The pre-1.0 spelling `project` is still accepted as a
  deprecated alias for `cross-file`.
- **`<group>`** — the rule's category: `complexity`, `correctness`,
  `performance`, `style`, or `suspicious`.
- **`<rule>`** — the rule id, kebab-case (e.g. `avoid-dynamic`). Legacy
  `snake_case` ids still resolve as aliases.

```dart
// falcon-ignore lint/suspicious/avoid-dynamic: interop boundary
// falcon-ignore cross-file/correctness/unused-files: generated entrypoint
```

> **Note:** falcon does **not** read Dart's own `// ignore:` /
> `// ignore_for_file:` comments — those still control the Dart analyzer's lints
> and have no effect on falcon. Suppress a falcon diagnostic only with
> `// falcon-ignore`.

Both `//` line comments and `///` doc comments carry directives. A directive
inside a `/* block comment */` or a string literal is **not** treated as a
suppression:

```dart
var note = '// falcon-ignore lint/suspicious/avoid-dynamic: x'; // string — ignored
/* falcon-ignore lint/suspicious/avoid-dynamic: x */            // block — ignored
```

## Inline suppression (one line)

A `// falcon-ignore` directive suppresses a single line. Placement decides
**which** line:

**Trailing** — a comment after code on the same line suppresses that line:

```dart
dynamic payload = decode(bytes); // falcon-ignore lint/suspicious/avoid-dynamic: interop boundary
```

**Leading** — a comment alone on its own line suppresses the next line of code:

```dart
// falcon-ignore lint/suspicious/avoid-dynamic: interop boundary
dynamic payload = decode(bytes);
```

## Stacking multiple rules

One comment carries **one** rule. To suppress several rules on the same code
line, stack the leading comments directly above it — consecutive
suppression-only lines all apply to the next line of code:

```dart
// falcon-ignore lint/suspicious/avoid-dynamic: interop boundary
// falcon-ignore lint/style/prefer-const-constructors: perf-tested
final widget = build(dynamicValue);
```

## File-level suppression (`falcon-ignore-all`)

`// falcon-ignore-all` suppresses a rule **everywhere in the file**. It may sit
anywhere, but conventionally goes at the top:

```dart
// falcon-ignore-all lint/suspicious/avoid-dynamic: generated file

dynamic a = 1;
dynamic b = 2;   // both suppressed
```

This is the inline equivalent of turning the rule off for one file via an
[`overrides` entry](./configuration.md#overrides) in `falcon.json` — reach for
`overrides` when you want the same exclusion across many files by path.

Cross-file rules are suppressed the same way, with the `cross-file` section:

```dart
// falcon-ignore-all cross-file/correctness/unused-code: public API surface
```

## The reason is required

Every directive must end with a non-empty reason after the colon. A directive
with no reason **does not suppress**; instead falcon reports a
`malformed-suppression` warning pointing at the comment:

```dart
dynamic x = 1; // falcon-ignore lint/suspicious/avoid-dynamic
//             ^ no reason → not suppressed, malformed-suppression reported
```

The same warning is raised — so a typo never silently fails to suppress — when:

- the **path is malformed** (not `<section>/<group>/<rule>`, or an unknown
  section):

  ```dart
  // falcon-ignore avoid-dynamic: x   → malformed path
  ```

- the **rule is named under the wrong group or section**; the message tells you
  the correct path:

  ```dart
  // falcon-ignore lint/style/avoid-dynamic: x
  //   avoid-dynamic is in `suspicious`, so the message says:
  //   "suppression path is lint/suspicious/avoid-dynamic"

  // falcon-ignore-all lint/correctness/unused-files: x
  //   unused-files is a cross-file rule, so the message says:
  //   "suppression path is cross-file/correctness/unused-files"
  ```

- the **rule name is unknown** (catching typos):

  ```dart
  // falcon-ignore lint/suspicious/not-a-rule: x   → unknown rule 'not-a-rule'
  ```

> **Caution:** `malformed-suppression` is an internal diagnostic. It is **not** a
> configurable rule — it does not appear in `falcon.json`, and it cannot itself
> be suppressed. Fix the directive instead.

## Details

- Rule paths are validated against falcon's registered rules; a legacy alias id
  is normalized to its canonical id, so an old id still matches the diagnostic's
  current rule name.
- Suppression comments are read from a real lex pass, not a line scan — which is
  why directives inside strings and block comments are correctly ignored.

## See also

- [Configuration](./configuration.md) — turn rules off by group, path, or
  project-wide.
- [Rules index](/linter/rules) — rule ids, groups, and examples.
- [Getting Started](./getting-started.md) — install, configure, and run falcon.
