# Corpus Fixture Format

This document describes the standardized format for test fixtures in the falcon corpus, used by the `validate-rules` harness to verify diagnostic accuracy across all rule implementations.

## Directory Structure

All test fixtures are organized under `crates/falcon_rules/tests/corpus/` with the following layout:

```
corpus/
├── {rule-name}/
│   ├── bad.dart          # Dart code containing violations (annotated)
│   ├── good.dart         # Dart code with no violations (no annotations)
│   ├── bad_2.dart        # (Optional) Additional bad fixture variants
│   ├── good_2.dart       # (Optional) Additional good fixture variants
│   └── config.json       # (Optional) Per-rule config used when validating this dir
├── {another-rule-name}/
│   ├── bad.dart
│   └── good.dart
└── ...
```

### Rules
- Each rule gets its own directory named after the rule identifier (e.g., `avoid-dynamic`, `camel-case-types`)
- The rule name must match the rule ID defined in the rule implementation
- At minimum, each rule directory must contain `bad.dart` and `good.dart`
- Additional variants (e.g., `bad_2.dart`, `good_2.dart`) are permitted for complex rules with multiple violation patterns
- Coverage target (plan §5.7): ≥5 positive and ≥5 negative examples per rule; rules that emit at most one diagnostic per file (e.g., `max_lines_for_file`) approximate this with multiple fixture file variants

### Per-Rule Config (`config.json`)

Config-gated or threshold-based rules may ship a `config.json` next to their fixtures.
The file uses the full `falcon.json` shape and is applied by both harnesses when
validating that directory (the xtask harness passes it to the binary via `--config`;
the in-process harness loads it with `falcon_config::load_config`):

```json
{
  "linter": {
    "rules": {
      "style": {
        "use-design-system-item": {
          "level": "warn",
          "options": {
            "items": [{ "class_name": "Container", "use_instead": "AppContainer" }]
          }
        }
      }
    }
  }
}
```

Rules live under `linter.rules.<group>` (group is the rule's category —
`complexity`, `correctness`, `performance`, `style`, or `suspicious`). A rule
value is either a level string (`"off"`, `"on"`, `"info"`, `"warn"`, `"error"`)
or an object `{ "level": ..., "options": { ... } }`.

Without a `config.json`, fixtures are validated with the default config (every rule
enabled at its default severity, no options).

## Annotation Format

### Basic Violation Annotation

Violations are marked with an inline comment at the end of the line where the violation occurs:

```dart
void example() {
  dynamic value = 42; /* expect: avoid-dynamic */
}
```

The format is: `/* expect: {rule-name} */`

### Message Validation Annotation

To validate the diagnostic message content, include an optional message annotation:

```dart
void example() {
  var a = 1, b = 2; /* expect: avoid-var-pattern, msg: "Avoid using 'var' for non-obvious types" */
}
```

The format is: `/* expect: {rule-name}, msg: "{expected-message-text}" */`

### Multiple Violations on Same Line

If multiple violations occur on the same line, use separate annotations:

```dart
class MyClass { /* expect: camel-case-types */ /* expect: another-rule */
```

## Good Files

Files named `good.dart` (and variants like `good_2.dart`) must:
- Contain valid Dart code that **does not trigger the rule**
- Have **no `/* expect: */` annotations** whatsoever
- Demonstrate correct patterns as a positive reference
- Be valid, compilable Dart code

Example `good.dart`:
```dart
void example() {
  final int value = 42;  // Good: no dynamic, clear type
  const int answer = 42; // Good: const with explicit type
}
```

## Bad Files

Files named `bad.dart` (and variants like `bad_2.dart`) must:
- Contain Dart code that **does trigger the rule**
- Have `/* expect: */` annotations **at every violation point**
- Each annotation must be on the same line as the violation
- Be valid, compilable Dart code (syntax-wise)

Example `bad.dart`:
```dart
void example() {
  dynamic value = 42; /* expect: avoid-dynamic */
  var x = 10;         /* expect: avoid-var-pattern */
}
```

## Mapping from Upstream Linters

falcon aims for semantic compatibility with upstream Dart linters. The following mappings apply when porting rules:

### From dart_code_linter / pyramid_lint

These linters use a different annotation style. When migrating fixtures, convert as follows:

**Old format (dart_code_linter/pyramid_lint):**
```dart
void example() {
  dynamic value = 42; // lint: avoid-dynamic
}
```

**New format (falcon):**
```dart
void example() {
  dynamic value = 42; /* expect: avoid-dynamic */
}
```

If the original fixture includes expected messages, preserve them in the new annotation:

**Old:**
```dart
dynamic value = 42; // lint: avoid-dynamic, Expected message here
```

**New:**
```dart
dynamic value = 42; /* expect: avoid-dynamic, msg: "Expected message here" */
```

## Validate-Rules Harness

The `cargo xtask validate-rules` harness processes corpus fixtures to verify that rule implementations produce correct diagnostics.

### How It Works

1. **Parse Annotations**: Reads `/* expect: */` comments from all bad.dart files
2. **Run Analysis**: Executes falcon on each fixture file, collecting actual diagnostics
3. **Match Diagnostics**: For each expected violation, validates:
   - **Rule ID**: Exact match with `expect:` rule name
   - **Line Number**: Exact match (line of violation)
   - **Message**: Fuzzy match against expected message (if provided) using Jaro-Winkler similarity
   - **Severity**: Exact match (default: ERROR)
4. **Report Results**: Outputs diagnostic accuracy per rule and overall corpus health
5. **Validate Good Files**: Ensures `good.dart` files produce zero diagnostics for the rule

### Running Validation

```bash
# Validate all rules
cargo xtask validate-rules

# Validate a specific rule
cargo xtask validate-rules avoid-dynamic

# Verbose output
cargo xtask validate-rules --verbose
```

### Exit Codes

- `0`: All rules pass validation
- `1`: One or more rules fail validation (mismatched diagnostics)
- `2`: Harness error (missing fixtures, invalid Dart syntax, etc.)

## Example: avoid-dynamic

### Directory Layout
```
corpus/avoid-dynamic/
├── bad.dart
└── good.dart
```

### bad.dart
```dart
// Bad: explicit dynamic type
void processValue(dynamic value) { /* expect: avoid-dynamic */
  print(value);
}

// Bad: var with dynamic inference
dynamic getData() { /* expect: avoid-dynamic */
  return null;
}

// Bad: dynamic in generics
List<dynamic> items = []; /* expect: avoid-dynamic */
```

### good.dart
```dart
// Good: specific type parameter
void processValue(Object value) {
  print(value);
}

// Good: explicit return type
String getData() {
  return "";
}

// Good: specific type in generics
List<String> items = [];
```

## Best Practices

1. **One Rule, One Directory**: Keep fixtures for different rules separate
2. **Minimal Examples**: Use short, focused examples that isolate the violation
3. **Valid Syntax**: Fixtures must be syntactically valid Dart (even bad.dart)
4. **Consistent Formatting**: Use consistent indentation and spacing
5. **Meaningful Comments**: Add explanatory comments in bad.dart showing what's wrong
6. **Message Specificity**: Include message annotations for rules with parametric messages
7. **Coverage**: Include multiple violation patterns for rules with complex heuristics
8. **Regression Tests**: Add fixtures for reported bugs before fixing them

## Troubleshooting

### Annotation Not Recognized
- Ensure the comment is on the same line as the violation
- Use exact format: `/* expect: rule-name */` (no extra spaces or characters)
- Verify the rule name matches the rule ID in the implementation

### Message Mismatch
- Check the message threshold (default: 0.85 Jaro-Winkler similarity)
- Use `msg: "..."` annotation to specify expected message
- Run with `--verbose` to see actual vs. expected messages

### Good File Showing Violations
- Remove any `/* expect: */` annotations from good.dart
- Ensure the code genuinely does not trigger the rule
- Check for rule-specific edge cases in the rule implementation

## Version History

- **M4.0**: Initial corpus format specification
- **M4.1**: Added message annotation support, validation harness
- **M4.2**: Planned threshold calibration and rule acceptance policy
- **M4.8**: Per-rule `config.json` support; ≥5 positive / ≥5 negative coverage target
