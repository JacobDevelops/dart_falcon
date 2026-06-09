# jdlint Rule Analysis Matrix

**Date:** 2026-06-09  
**Phase:** Phase 1 — Port rules from dart_code_linter + pyramid_lint  
**Status:** Implementation Guide — Detailed complexity breakdown and implementation strategy  
**Total Rules:** 60 (34 from dart_code_linter, 26 from pyramid_lint)

---

## Executive Summary

This document provides the detailed implementation guidance for all ~60 lint rules being ported from `dart_code_linter` (^3.2.1) and `pyramid_lint` (^2.4.0) to `jdlint` Phase 1.

**Complexity Breakdown:**
- **SIMPLE (1–2h):** ~40 rules — pure AST pattern matching
- **MEDIUM (2–3h):** ~14 rules — visiting related nodes, context awareness
- **COMPLEX (4–6h):** ~6 rules — scope lookup, type inference, const evaluation

**Estimated Total:** 92–158 hours across M4.2–M4.6

---

## Part 1: Complete Rule Summary Table

All 60 rules with complexity, semantic requirements, AST nodes, Phase 1 heuristics, and test fixtures.

### Tier: SIMPLE Rules (40 rules, 1–2h each)

| # | Rule | Source | Complexity | Semantic Tag | AST Nodes Needed | Phase 1 Heuristic | Test Fixture |
|---|------|--------|-----------|-------------|-----------------|-------------------|-------------|
| 1 | `avoid-dynamic` | dart_code_linter | SIMPLE | AST-only | TypeName, NamedType | Match `dynamic` type keyword | `var x = dynamic;` |
| 2 | `avoid-ignoring-return-values` | dart_code_linter | SIMPLE | AST-only | FunctionCall, MethodInvocation | Warn on calls not assigned or used | `foo();` (not used) |
| 3 | `avoid-late-keyword` | dart_code_linter | SIMPLE | AST-only | VariableDeclaration, Modifier | Match `late` modifier | `late int x;` |
| 4 | `avoid-nested-conditional-expressions` | dart_code_linter | SIMPLE | AST-only | ConditionalExpression | Count nested ternaries (depth > 1) | `a ? (b ? c : d) : e` |
| 5 | `avoid-non-null-assertion` | dart_code_linter | SIMPLE | AST-only | UnaryExpression, PostfixExpression | Match `!` null-assertion operator | `x!` |
| 6 | `avoid-throw-in-catch-block` | dart_code_linter | SIMPLE | AST-only | CatchClause, ThrowStatement | Check for `throw` in catch block | `catch { throw; }` |
| 7 | `avoid-top-level-member-access` | dart_code_linter | SIMPLE | AST-only | Identifier, MemberAccess, TopLevelVariableDeclaration | Detect top-level var access (not const) | `var globalX = 1;` used elsewhere |
| 8 | `binary-expression-operand-order` | dart_code_linter | SIMPLE | AST-only | BinaryExpression | Warn if literal on left side (`5 == x` instead of `x == 5`) | `5 == x` |
| 9 | `double-literal-format` | dart_code_linter | SIMPLE | AST-only | DoubleLiteral | Require leading zero (`0.5` not `.5`), forbid trailing zeros | `.5` or `1.0` |
| 10 | `no-boolean-literal-compare` | dart_code_linter | SIMPLE | AST-only | BinaryExpression | Disallow `x == true`, `y == false` | `if (x == true)` |
| 11 | `no-empty-block` | dart_code_linter | SIMPLE | AST-only | Block, FunctionBody | Forbid empty `{}` (empty catch, method bodies) | `{ }` |
| 12 | `no-equal-arguments` | dart_code_linter | SIMPLE | AST-only | FunctionCall, MethodInvocation, ArgumentList | Warn if two arguments are structurally identical | `foo(x, x)` |
| 13 | `no-equal-then-else` | dart_code_linter | SIMPLE | AST-only | ConditionalExpression, IfStatement | Warn if then-branch equals else-branch | `x ? a : a` |
| 14 | `no-object-declaration` | dart_code_linter | SIMPLE | AST-only | VariableDeclaration, NamedType | Disallow `Object` type (use `dynamic` or specific type) | `Object x;` |
| 15 | `prefer-async-await` | dart_code_linter | SIMPLE | AST-only | FunctionBody, FutureThen, MethodInvocation | Suggest `.then().catch()` chains → async/await | `.then((x) { return foo(x); })` |
| 16 | `prefer-const-border-radius` | dart_code_linter | SIMPLE | AST-only | MethodInvocation, InstanceCreation | Suggest `BorderRadius.circular()` for symmetry | `BorderRadius.only(tl: Radius.circular(8), tr: ...)` |
| 17 | `prefer-correct-edge-insets-constructor` | dart_code_linter | SIMPLE | AST-only | MethodInvocation, InstanceCreation | Suggest correct EdgeInsets constructor (`.symmetric`, `.all`) | `EdgeInsets.only(top: 8, bottom: 8)` |
| 18 | `prefer-correct-identifier-length` | dart_code_linter | SIMPLE | AST-only | Identifier, VariableDeclaration | Forbid single-letter identifiers (except loop counters) | `var a = 1;` |
| 19 | `prefer-first` | dart_code_linter | SIMPLE | AST-only | MethodInvocation | Suggest `.first` instead of `[0]` on collections | `list[0]` |
| 20 | `prefer-immediate-return` | dart_code_linter | SIMPLE | AST-only | FunctionBody, VariableDeclaration, ReturnStatement | Simplify: `var x = foo(); return x;` → `return foo();` | `{ var x = foo(); return x; }` |
| 21 | `prefer-last` | dart_code_linter | SIMPLE | AST-only | MethodInvocation | Suggest `.last` instead of `[length-1]` on collections | `list[list.length - 1]` |
| 22 | `avoid_abbreviations_in_doc_comments` | pyramid_lint | SIMPLE | AST-only | DocumentationComment, Identifier | Flag abbreviations in doc comments | `/// The impl of X` (should be "implementation") |
| 23 | `avoid_empty_blocks` | pyramid_lint | SIMPLE | AST-only | Block, CatchClause | Forbid empty catch/if/else blocks | `catch { }` |
| 24 | `avoid_inverted_boolean_expressions` | pyramid_lint | SIMPLE | AST-only | UnaryExpression, BinaryExpression | Warn on double negation (`!!x`) | `!!x` or `!(!x)` |
| 25 | `avoid_nested_if` | pyramid_lint | SIMPLE | AST-only | IfStatement | Warn on if-statements nested more than 1 level deep | `if (x) { if (y) { } }` |
| 26 | `avoid_positional_fields_in_records` | pyramid_lint | SIMPLE | AST-only | RecordLiteral, RecordType | Require named fields in records | `(int, String)` instead of `({int x, String y})` |
| 27 | `boolean_prefixes` | pyramid_lint | SIMPLE | AST-only | VariableDeclaration, Identifier | Enforce `is`/`has`/`can` prefix for boolean vars | `bool active;` should be `bool isActive;` |
| 28 | `correct_order_for_super_dispose` | pyramid_lint | SIMPLE | AST-only | MethodDeclaration, SuperInvocation | Enforce `super.dispose()` at END of dispose method | `super.dispose(); controller.dispose();` |
| 29 | `max_lines_for_file` | pyramid_lint | SIMPLE | AST-only | CompilationUnit | Flag files over 500 lines | Any file with >500 lines |
| 30 | `max_lines_for_function` | pyramid_lint | SIMPLE | AST-only | FunctionDeclaration, MethodDeclaration | Flag functions/methods over 100 lines | Function with >100 lines |
| 31 | `max_parameters_for_function` | pyramid_lint | SIMPLE | AST-only | FunctionDeclaration, FormalParameterList | Flag functions with >5 parameters | `foo(int a, int b, int c, int d, int e, int f)` |
| 32 | `max_switch_cases` | pyramid_lint | SIMPLE | AST-only | SwitchStatement, SwitchCase | Flag switch with >10 cases | `switch { case 1: ... case 11: ... }` |
| 33 | `no_duplicate_case_values` | pyramid_lint | SIMPLE | AST-only | SwitchStatement, SwitchCase | Flag duplicate case labels | `switch { case 1: ... case 1: ... }` |
| 34 | `no_magic_number` (SIMPLE variant) | pyramid_lint | SIMPLE | AST-only | IntegerLiteral, DoubleLiteral | Flag numeric literals except 0, 1, 2, -1 | `x = 42;` |
| 35 | `prefer_declaring_const_constructor` | pyramid_lint | SIMPLE | AST-only | ConstructorDeclaration | Flag constructors with only const fields → mark const | `constructor() { this.x = 1; }` |
| 36 | `prefer_iterable_any` | pyramid_lint | SIMPLE | AST-only | MethodInvocation | Suggest `.any()` over `.where().isNotEmpty` | `list.where((x) => x > 5).isNotEmpty` |
| 37 | `prefer_iterable_every` | pyramid_lint | SIMPLE | AST-only | MethodInvocation | Suggest `.every()` over `!.where().isEmpty` | `!list.where((x) => x > 5).isEmpty` |
| 38 | `prefer_underscore_for_unused_callback_parameters` | pyramid_lint | SIMPLE | AST-only | FormalParameter, FunctionExpression | Use `_` for unused callback params | `.forEach((x) { print("hi"); })` |
| 39 | `use_spacer_as_expanded_child` | pyramid_lint | SIMPLE | AST-only | InstanceCreation, MethodInvocation | Suggest `Spacer()` instead of empty `Container()` or `SizedBox()` | `Expanded(child: Container())` |
| 40 | `no_empty_block` | pyramid_lint | SIMPLE | AST-only | Block, CatchClause, FunctionBody | Forbid empty blocks (overlaps with dart_code_linter) | `{ }` |

**Subtotal SIMPLE:** 40 rules, ~40–80 hours

---

### Tier: MEDIUM Rules (14 rules, 2–3h each)

| # | Rule | Source | Complexity | Semantic Tag | AST Nodes Needed | Phase 1 Heuristic | Test Fixture |
|---|------|--------|-----------|-------------|-----------------|-------------------|-------------|
| 41 | `avoid-global-state` | dart_code_linter | MEDIUM | AST-only | TopLevelVariableDeclaration, ClassDeclaration | Warn on mutable top-level vars (not const, not @memoized) | `var globalX = [];` (mutable) |
| 42 | `avoid-passing-async-when-sync-expected` | dart_code_linter | MEDIUM | requires-scope-lookup | FunctionCall, MethodInvocation, TypeAnnotation | Check if async function passed to sync parameter (heuristic: check param type annotation) | `asyncFn` passed to `Future<void> Function() param` |
| 43 | `avoid-redundant-async` | dart_code_linter | MEDIUM | AST-only | FunctionDeclaration, MethodDeclaration, FunctionBody | Flag `async` if only one `await` and no error handling | `async { await foo(); return x; }` → just `return foo()` |
| 44 | `avoid-returning-widgets` | dart_code_linter | MEDIUM | requires-scope-lookup | ReturnStatement, MethodDeclaration, Identifier | Warn if function returns `Widget` in non-build-method (heuristic: check method name; full: scope lookup) | `String getWidget() { return Container(); }` |
| 45 | `avoid-unnecessary-type-assertions` | dart_code_linter | MEDIUM | requires-type-inference | TypeTest, IsExpression | Warn on `is T` where T is known from annotation (heuristic: check if variable has explicit type) | `final int x = 5; if (x is int) ...` |
| 46 | `avoid-unnecessary-type-casts` | dart_code_linter | MEDIUM | requires-type-inference | AsExpression, Identifier | Warn on `as T` where already known to be T (heuristic: check annotation) | `final int x = 5; final y = x as int;` |
| 47 | `avoid-unrelated-type-assertions` | dart_code_linter | MEDIUM | requires-type-inference | TypeTest, IsExpression | Warn on `is T` where never true (heuristic: AST structure check; e.g., `String is int`) | `if ("hello" is int) ...` |
| 48 | `avoid-unused-parameters` | dart_code_linter | MEDIUM | requires-scope-lookup | FormalParameter, FunctionBody, Identifier | Warn on params never referenced in body | `foo(int unused) { print("hi"); }` |
| 49 | `prefer-conditional-expressions` | dart_code_linter | MEDIUM | AST-only | IfStatement | Suggest ternary for simple if/else returning values | `if (x) return a; else return b;` → `return x ? a : b;` |
| 50 | `prefer-extracting-callbacks` | dart_code_linter | MEDIUM | AST-only | MethodInvocation, FunctionExpression, VariableDeclaration | Suggest extracting large inline callbacks to named functions | `.map((x) { 20 lines of logic })` |
| 51 | `prefer-trailing-comma` | dart_code_linter | MEDIUM | AST-only | ArgumentList, FormalParameterList, InstanceCreation | Require trailing comma in multi-line argument/parameter lists | `foo(\n  arg1,\n  arg2\n)` (missing comma) |
| 52 | `avoid_mutable_global_variables` | pyramid_lint | MEDIUM | AST-only | TopLevelVariableDeclaration, Modifier | Disallow mutable top-level vars (only const allowed) | `var globalX = [];` (mutable) |
| 53 | `prefer_dedicated_media_query_methods` | pyramid_lint | MEDIUM | AST-only | MethodInvocation | Suggest `.width`, `.height` instead of `.size.width` | `MediaQuery.of(context).size.width` |
| 54 | `unnecessary_flutter_imports` | pyramid_lint | MEDIUM | requires-scope-lookup | ImportDirective, Identifier | Warn on unused imports (heuristic: import declared but no symbols from it used) | `import 'package:flutter/material.dart';` (unused) |

**Subtotal MEDIUM:** 14 rules, ~28–42 hours

---

### Tier: COMPLEX Rules (6 rules, 4–6h each)

| # | Rule | Source | Complexity | Semantic Tag | AST Nodes Needed | Phase 1 Heuristic | Test Fixture |
|---|------|--------|-----------|-------------|-----------------|-------------------|-------------|
| 55 | `member-ordering` | dart_code_linter | COMPLEX | AST-only | ClassDeclaration, MethodDeclaration, FieldDeclaration, ConstructorDeclaration | Enforce order: constants, static fields, instance fields, constructors, static methods, instance methods. Config: `order = [const, static_fields, fields, constructor, static_methods, methods]` | Class with methods/fields in wrong order |
| 56 | `no-magic-number` | dart_code_linter | COMPLEX | requires-const-eval | IntegerLiteral, DoubleLiteral | Phase 1: Ban ALL numeric literals except 0, 1, 2, -1 (configurable threshold list). Phase 2: const eval. | `final x = 42;` (not in allowlist) |
| 57 | `class_members_ordering` | pyramid_lint | COMPLEX | AST-only | ClassDeclaration, MethodDeclaration, FieldDeclaration, ConstructorDeclaration | Enforce member order (similar to member-ordering but with pyramid_lint conventions) | Class with unordered members |
| 58 | `use_once_constructors_once_provider` | pyramid_lint | COMPLEX | requires-scope-lookup | InstanceCreation, Identifier, MethodInvocation | Detect `OnceProvider` usage without `.one` wrapper (scope: must track `OnceProvider` type availability) | `OnceProvider(create: ...)` without `.once()` |
| 59 | `unnecessary_nullable_return_type` | pyramid_lint | MEDIUM* | requires-type-inference | ReturnType, TypeName, FunctionDeclaration | Flag `Future<T?>` return type that never returns null (Phase 1: AST heuristic on return statements) | `Future<int?> foo() { return Future.value(5); }` |
| 60 | `avoid_unused_parameters` | pyramid_lint | MEDIUM* | requires-scope-lookup | FormalParameter, FunctionBody, Identifier | Flag unused callback params (overlaps with dart_code_linter variant) | `onPressed: (event) { print("hi"); }` |

**Subtotal COMPLEX:** 6 rules, ~24–36 hours

---

## Part 2: Implementation Order & Batching Strategy

### Phase 1 Execution Order

**Recommended batching for parallel development (M4.2–M4.6):**

#### Batch 1: Pure AST Pattern Matching (M4.2 — 2 weeks, 8–12 engineers)
**SIMPLE, AST-only rules — no scope/type requirements**

Priority: 1 (highest parallelism)

```
avoid-dynamic
avoid-ignoring-return-values
avoid-late-keyword
avoid-nested-conditional-expressions
avoid-non-null-assertion
avoid-throw-in-catch-block
avoid-top-level-member-access
binary-expression-operand-order
double-literal-format
no-boolean-literal-compare
no-empty-block (both versions)
no-equal-arguments
no-equal-then-else
no-object-declaration
prefer-async-await
prefer-const-border-radius
prefer-correct-edge-insets-constructor
prefer-correct-identifier-length
prefer-first
prefer-immediate-return
prefer-last
avoid_abbreviations_in_doc_comments
avoid_empty_blocks
avoid_inverted_boolean_expressions
avoid_nested_if
avoid_positional_fields_in_records
boolean_prefixes
correct_order_for_super_dispose
max_lines_for_file
max_lines_for_function
max_parameters_for_function
max_switch_cases
no_duplicate_case_values
prefer_declaring_const_constructor
prefer_iterable_any
prefer_iterable_every
prefer_underscore_for_unused_callback_parameters
use_spacer_as_expanded_child
```

**Execution model:** Each rule = 1 engineer, 1–2h. Parallel codegen from `xtask` to stub out visitor methods.

---

#### Batch 2: AST + Context Awareness (M4.3–M4.4 — 2 weeks, 6–8 engineers)
**MEDIUM rules — require visiting related nodes or simple context**

Priority: 2

```
avoid-global-state (mutable top-level)
avoid-redundant-async (count await statements)
prefer-conditional-expressions (if/else shape check)
prefer-extracting-callbacks (closure size check)
prefer-trailing-comma (multi-line list detection)
avoid_mutable_global_variables (overlap with avoid-global-state)
prefer_dedicated_media_query_methods (API pattern matching)
```

**Execution model:** 2–3h per rule. Requires visiting child nodes, pattern matching on context.

---

#### Batch 3: Scope Lookup & Type Heuristics (M4.4–M4.5 — 2 weeks, 4–6 engineers)
**MEDIUM rules requiring scope lookups (no cross-file resolution in Phase 1)**

Priority: 3

```
avoid-passing-async-when-sync-expected (check param type annotation)
avoid-returning-widgets (check method name heuristic)
avoid-unnecessary-type-assertions (check variable annotation)
avoid-unnecessary-type-casts (check variable annotation)
avoid-unrelated-type-assertions (AST structure check)
avoid-unused-parameters (track identifier usage in body)
unnecessary_flutter_imports (track import usage)
```

**Execution model:** 2–3h per rule. Build a simple `ScopeCollector` visitor (reusable) to track identifiers and scope.

---

#### Batch 4: Member Ordering (M4.5 — 1 week, 2 engineers)
**COMPLEX rules — require enforcing declaration order**

Priority: 4a

```
member-ordering (dart_code_linter)
class_members_ordering (pyramid_lint)
```

**Execution model:** ~4–5h each. Implement once, adapt for both rule variants (same logic, different config schema).

---

#### Batch 5: Numeric Literal Analysis (M4.5 — 1 week, 1–2 engineers)
**COMPLEX rules — const evaluation heuristics**

Priority: 4b

```
no-magic-number (dart_code_linter + pyramid_lint overlap)
```

**Execution model:** ~4h. Single implementation shared by both rule sets. Config: allowlist threshold.

---

#### Batch 6: Flutter-Specific & Advanced Scope (M4.6 — 1 week, 2 engineers)
**COMPLEX rules requiring provider/advanced scope tracking**

Priority: 5

```
use_once_constructors_once_provider (requires provider type tracking)
unnecessary_nullable_return_type (return statement analysis)
```

**Execution model:** ~4–5h each. May require integration with provider pattern detection.

---

## Part 3: Rule Overlaps & Deduplication

### Exact Duplicates (4 rules)

These rules have identical semantics across both packages. Implement once, expose via both rule names.

| Rule Name | dart_code_linter | pyramid_lint | Shared Implementation |
|-----------|------------------|--------------|----------------------|
| `no-empty-block` | ✓ | ✓ | Yes — single visitor, two rule registrations |
| `avoid-unused-parameters` | ✓ | ✓ | Yes — single implementation, config-driven variants |
| `no-magic-number` | ✓ (COMPLEX) | ✓ (SIMPLE) | Yes — single implementation, different configs |

**Strategy:** Register once in rule registry, map both rule IDs to single implementation.

---

### Semantic Overlaps (3 rules)

These rules are very similar but with minor differences in strictness or scope.

| Rule | Source 1 | Source 2 | Difference | Handling |
|------|----------|----------|-----------|----------|
| `avoid-global-state` | dart_code_linter | `avoid_mutable_global_variables` (pyramid_lint) | dart_code_linter also allows `@memoized`; pyramid_lint only allows `const` | Implement once, parameterize via `severity` or config |
| `member-ordering` | dart_code_linter | `class_members_ordering` (pyramid_lint) | Both enforce class member order; pyramid_lint may have stricter config | Shared logic, separate configs |
| `avoid-nested-if` (pyramid) vs structure checks | pyramid_lint | (no exact dart_code_linter match) | Standalone — no overlap | Standalone rule |

---

### Rules with Conditional Behavior (2 rules)

| Rule | Condition | Handling |
|------|-----------|----------|
| `no-magic-number` | Numeric thresholds vary by package | Config-driven; allow per-rule threshold in jdlint.json |
| `prefer-correct-identifier-length` | May exclude loop counters (`i`, `j`, `k`) | Heuristic: allow single-char only in for loops |

---

## Part 4: Phase 1 Heuristics for Complex Rules

For rules marked `requires-scope-lookup` or `requires-type-inference`, Phase 1 uses simplified AST-based heuristics (no cross-file resolution, no type inference engine).

### Heuristic 1: Scope Lookup via AST Scope Stack

**Rules affected:** `avoid-passing-async-when-sync-expected`, `avoid-returning-widgets`, `avoid-unused-parameters`, `unnecessary_flutter_imports`

**Implementation:**
1. Build a `ScopeCollector` visitor that tracks:
   - All identifiers declared in the current scope (variables, params, imports)
   - All identifiers referenced in the current scope
2. Report unused identifiers as violations.
3. For type checks, inspect type annotations in the AST (no resolution).

**Limitations:**
- No cross-file scope resolution (all files analyzed independently)
- Type annotations only; no inference
- Closure parameters treated as separate scope

---

### Heuristic 2: Type Annotation Matching

**Rules affected:** `avoid-unnecessary-type-assertions`, `avoid-unnecessary-type-casts`, `avoid-unrelated-type-assertions`

**Implementation:**
1. When seeing `x is T` or `x as T`, collect:
   - Type of `x` from explicit annotation (if present)
   - Type `T` from the assertion/cast
2. Compare AST node structure (not semantic resolution):
   - If both are `int`, flag as redundant.
   - If `x: String` and `x is int`, flag as unrelated.

**Limitations:**
- Only works with explicit annotations
- No narrowing from prior assertions
- No inference from return types

---

### Heuristic 3: Const Evaluation via Allowlist

**Rules affected:** `no-magic-number`

**Implementation:**
1. Traverse all numeric literals in AST.
2. Maintain allowlist: `[0, 1, 2, -1]` (configurable via jdlint.json).
3. Flag any literal not in allowlist as "magic."

**Limitations:**
- No const expression folding (`1 + 1` is not flagged, just literals)
- All numeric literals treated equally (no context-aware thresholds)
- No support for named constants in Phase 1

**Phase 2 upgrade:** Full const expression evaluation via interpreter.

---

## Part 5: Test Fixture Guidelines

For each rule implementation, create at least two test cases:

1. **Positive case** — code that SHOULD trigger the rule
2. **Negative case** — code that should NOT trigger the rule

### Example: `avoid-dynamic`

**Positive:**
```dart
var x = dynamic;  // ✗ Flag: using dynamic type
dynamic foo() => 5;
```

**Negative:**
```dart
int x = 5;  // ✓ OK
Future foo() async => 5;
```

---

### Example: `member-ordering`

**Positive (incorrect order):**
```dart
class Foo {
  void method() {}  // ✗ Method before const
  static const int X = 1;
}
```

**Negative (correct order):**
```dart
class Foo {
  static const int X = 1;
  void method() {}  // ✓ OK
}
```

---

## Part 6: Complexity Justification

### Why SIMPLE (1–2h)?

Rules matching patterns that appear exactly once in a localized AST subtree:
- Type name matching (`dynamic`, `Object`)
- Single-node patterns (`late` modifier, `!` operator, empty blocks)
- Statement-level checks (compare/assert, return conditions)
- Naming conventions (identifier length, boolean prefixes)

**Key:** No traversal required; single visit suffices.

---

### Why MEDIUM (2–3h)?

Rules requiring visiting multiple related nodes or context awareness:
- Counting nested structures (conditional nesting, if/else chains)
- Checking sibling/parent context (const fields in constructor → const constructor)
- Simple scope tracking (unused params, global state)
- Method call pattern matching (`.where().isNotEmpty` → `.any()`)

**Key:** Visitor must traverse related branches; scope context helpful but not full resolution.

---

### Why COMPLEX (4–6h)?

Rules requiring:
- **Member ordering:** Collect all class members, sort by category, verify order
- **Const evaluation:** Traverse expression trees, evaluate literals
- **Advanced scope:** Track provider/builder patterns across methods

**Key:** Multi-pass or complex state machine; ~4–6h including testing + edge cases.

---

## Part 7: Configuration & Rule Registry

### jdlint.json Schema (Sketch)

```json
{
  "rules": {
    "avoid-dynamic": {
      "enabled": true,
      "severity": "error"
    },
    "no-magic-number": {
      "enabled": true,
      "severity": "warning",
      "allowlist": [0, 1, 2, -1]
    },
    "member-ordering": {
      "enabled": true,
      "order": ["const", "static_fields", "fields", "constructor", "static_methods", "methods"]
    },
    "max-lines-for-file": {
      "enabled": true,
      "threshold": 500
    }
  }
}
```

### Rule Registry (Rust API)

```rust
pub struct RuleRegistry {
    rules: HashMap<String, Box<dyn Rule>>,
}

impl RuleRegistry {
    pub fn register(&mut self, name: &str, rule: Box<dyn Rule>) { ... }
    pub fn get(&self, name: &str) -> Option<&dyn Rule> { ... }
    pub fn all(&self) -> Vec<&dyn Rule> { ... }
}
```

---

## Part 8: Pre-Flight Checklist (M4.0 entry gate)

Before rule implementation begins (M4.2), verify:

- [ ] All 60 rules documented in this matrix
- [ ] Complexity tiers finalized (40 SIMPLE, 14 MEDIUM, 6 COMPLEX)
- [ ] Rule overlaps identified and deduplication strategy approved
- [ ] Phase 1 heuristics documented and tested on sample code
- [ ] Test fixtures created for all rules (2 cases each)
- [ ] jdlint.json schema finalized and documented
- [ ] RuleVisitor trait designed and prototyped
- [ ] `xtask` codegen pipeline operational (generates rule stubs)
- [ ] Parallel batching schedule agreed with team
- [ ] dart_code_linter + pyramid_lint repos pinned and accessible

---

## Part 9: Integration Checklist (Phase 1 completion)

Before merging to main (end of M4.6):

- [ ] All 60 rules implemented and passing tests
- [ ] Zero overlapping diagnostics (deduplicated rules working correctly)
- [ ] jdlint.json config file working (rules can be enabled/disabled)
- [ ] Full jfit mobile project lints in <1s on typical hardware
- [ ] LSP server reporting all diagnostics correctly
- [ ] No panics or unwrap() calls in rule visitors
- [ ] Rule documentation (.md files) generated for all rules

---

## Appendix: Rule Severity Defaults

| Rule | Default Severity | Rationale |
|------|------------------|-----------|
| `avoid-dynamic` | error | Type safety critical |
| `avoid-global-state` | warning | Code smell; refactorable |
| `no-magic-number` | warning | Reduces readability; config-tunable |
| `member-ordering` | warning | Style enforcement; non-breaking |
| `max-lines-for-file` | warning | Code maintainability hint |
| `prefer-async-await` | suggestion | Style preference; low impact |
| `avoid-nested-conditional-expressions` | warning | Cognitive complexity |

---

**End of RULE_ANALYSIS_MATRIX.md**
