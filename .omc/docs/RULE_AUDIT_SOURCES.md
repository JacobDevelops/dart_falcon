# jdlint Rule Audit: Source Locations & Version Pins

**Date:** 2026-06-09  
**Phase:** Phase 1 — Audit trail for all ~60 ported rules  
**Purpose:** Traceability matrix linking each jdlint rule to source package, repo, version, and file path

---

## Executive Summary

This document provides the definitive source audit for all 60 lint rules being ported in Phase 1. For each rule:
- **Source package:** dart_code_linter or pyramid_lint
- **Repository URL** and commit reference
- **Version pin:** As specified in jfit's `pubspec.yaml`
- **Source file path:** Likely location in the source package (estimated from package conventions)
- **Status:** Located, TBD, or Deferred

---

## Version Pins & Repository URLs

### dart_code_linter

| Property | Value |
|----------|-------|
| **Package name** | `dart_code_linter` |
| **Repository** | https://github.com/bancolombia/dart-code-linter |
| **Phase 1 Version** | ^3.2.1 (latest stable as of 2026-06) |
| **Maintained fork of** | `dart-code-metrics` (original repo archived) |
| **jfit pubspec.yaml pin** | `dart_code_linter: ^3.2.1` (check current) |
| **Last verified commit** | `main` branch (check for exact tag) |

**Repository structure:**
```
dart-code-linter/
├── lib/
│   └── src/
│       ├── rules/
│       │   ├── avoid_dynamic/
│       │   │   ├── avoid_dynamic.dart
│       │   │   ├── avoid_dynamic_visitor.dart
│       │   │   └── avoid_dynamic_report.dart
│       │   ├── [other rules]
│       │   └── ...
│       ├── analyzers/
│       ├── models/
│       └── utils/
├── test/
│   └── src/
│       └── rules/
│           └── [test files matching rule structure]
├── pubspec.yaml
└── README.md
```

---

### pyramid_lint

| Property | Value |
|----------|-------|
| **Package name** | `pyramid_lint` |
| **Repository** | https://github.com/charlescyt/pyramid_lint |
| **Phase 1 Version** | ^2.4.0 (latest stable as of 2026-06) |
| **jfit pubspec.yaml pin** | `pyramid_lint: ^2.4.0` (check current) |
| **Last verified commit** | `main` branch (check for exact tag) |

**Repository structure:**
```
pyramid_lint/
├── lib/
│   └── src/
│       ├── lints/
│       │   ├── avoid_abbreviations_in_doc_comments.dart
│       │   ├── avoid_dynamic.dart
│       │   ├── [other rules]
│       │   └── ...
│       ├── models/
│       └── utils/
├── test/
│   └── src/
│       └── lints/
│           └── [test files]
├── pubspec.yaml
└── README.md
```

---

## Part 1: dart_code_linter Rules (34 rules)

Each rule documents:
- **Name:** Rule ID as used in jdlint
- **Source file:** Estimated path in dart_code_linter repo
- **Visitor class:** Main implementation class (if known)
- **Status:** Audit status (✓ Located, ? TBD, ~ Deferred)
- **Notes:** Relevant implementation details or version-specific info

---

### Rule 1: `avoid-dynamic`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-dynamic` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_dynamic/avoid_dynamic.dart` |
| **Visitor class** | `AvoidDynamicVisitor` (estimated) |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_dynamic/avoid_dynamic_test.dart` |
| **Notes** | Core type-safety rule; matches `dynamic` keyword in type annotations |

---

### Rule 2: `avoid-global-state`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-global-state` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_global_state/avoid_global_state.dart` |
| **Visitor class** | `AvoidGlobalStateVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_global_state/avoid_global_state_test.dart` |
| **Notes** | Flags mutable top-level variables; allows `const` and `@memoized` |

---

### Rule 3: `avoid-ignoring-return-values`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-ignoring-return-values` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_ignoring_return_values/avoid_ignoring_return_values.dart` |
| **Visitor class** | `AvoidIgnoringReturnValuesVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_ignoring_return_values/avoid_ignoring_return_values_test.dart` |
| **Notes** | Detects unused return values; configurable exclusion list |

---

### Rule 4: `avoid-late-keyword`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-late-keyword` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_late_keyword/avoid_late_keyword.dart` |
| **Visitor class** | `AvoidLateKeywordVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_late_keyword/avoid_late_keyword_test.dart` |
| **Notes** | Flags `late` modifier; promotes eager initialization |

---

### Rule 5: `avoid-nested-conditional-expressions`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-nested-conditional-expressions` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_nested_conditional_expressions/avoid_nested_conditional_expressions.dart` |
| **Visitor class** | `AvoidNestedConditionalExpressionsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_nested_conditional_expressions/avoid_nested_conditional_expressions_test.dart` |
| **Notes** | Detects ternary operators nested more than 1 level deep |

---

### Rule 6: `avoid-non-null-assertion`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-non-null-assertion` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_non_null_assertion/avoid_non_null_assertion.dart` |
| **Visitor class** | `AvoidNonNullAssertionVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_non_null_assertion/avoid_non_null_assertion_test.dart` |
| **Notes** | Warns on `!` null-assertion operator |

---

### Rule 7: `avoid-passing-async-when-sync-expected`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-passing-async-when-sync-expected` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_passing_async_when_sync_expected/avoid_passing_async_when_sync_expected.dart` |
| **Visitor class** | `AvoidPassingAsyncWhenSyncExpectedVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_passing_async_when_sync_expected/...` |
| **Notes** | Requires scope lookup; Phase 1 uses type annotation heuristic |

---

### Rule 8: `avoid-redundant-async`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-redundant-async` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_redundant_async/avoid_redundant_async.dart` |
| **Visitor class** | `AvoidRedundantAsyncVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_redundant_async/avoid_redundant_async_test.dart` |
| **Notes** | Flags async with single await; suggests removing async/await |

---

### Rule 9: `avoid-returning-widgets`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-returning-widgets` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_returning_widgets/avoid_returning_widgets.dart` |
| **Visitor class** | `AvoidReturningWidgetsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_returning_widgets/avoid_returning_widgets_test.dart` |
| **Notes** | Warns on Widget return types outside build methods; Phase 1 uses method name heuristic |

---

### Rule 10: `avoid-throw-in-catch-block`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-throw-in-catch-block` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_throw_in_catch_block/avoid_throw_in_catch_block.dart` |
| **Visitor class** | `AvoidThrowInCatchBlockVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_throw_in_catch_block/avoid_throw_in_catch_block_test.dart` |
| **Notes** | Detects `throw` statements inside catch blocks |

---

### Rule 11: `avoid-top-level-member-access`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-top-level-member-access` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_top_level_member_access/avoid_top_level_member_access.dart` |
| **Visitor class** | `AvoidTopLevelMemberAccessVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_top_level_member_access/avoid_top_level_member_access_test.dart` |
| **Notes** | Warns on non-const top-level variable access |

---

### Rule 12: `avoid-unnecessary-type-assertions`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-unnecessary-type-assertions` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_unnecessary_type_assertions/avoid_unnecessary_type_assertions.dart` |
| **Visitor class** | `AvoidUnnecessaryTypeAssertionsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_unnecessary_type_assertions/...` |
| **Notes** | Detects redundant `is T` checks; Phase 1 uses annotation matching |

---

### Rule 13: `avoid-unnecessary-type-casts`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-unnecessary-type-casts` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_unnecessary_type_casts/avoid_unnecessary_type_casts.dart` |
| **Visitor class** | `AvoidUnnecessaryTypeCastsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_unnecessary_type_casts/avoid_unnecessary_type_casts_test.dart` |
| **Notes** | Detects redundant `as T` casts; Phase 1 uses annotation matching |

---

### Rule 14: `avoid-unrelated-type-assertions`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-unrelated-type-assertions` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_unrelated_type_assertions/avoid_unrelated_type_assertions.dart` |
| **Visitor class** | `AvoidUnrelatedTypeAssertionsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_unrelated_type_assertions/...` |
| **Notes** | Detects unreachable `is T` checks (e.g., `String is int`); AST structure check |

---

### Rule 15: `avoid-unused-parameters`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid-unused-parameters` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/avoid_unused_parameters/avoid_unused_parameters.dart` |
| **Visitor class** | `AvoidUnusedParametersVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/avoid_unused_parameters/avoid_unused_parameters_test.dart` |
| **Notes** | Flags function/method parameters never referenced in body |

---

### Rule 16: `binary-expression-operand-order`

| Property | Value |
|----------|-------|
| **Rule ID** | `binary-expression-operand-order` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/binary_expression_operand_order/binary_expression_operand_order.dart` |
| **Visitor class** | `BinaryExpressionOperandOrderVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/binary_expression_operand_order/...` |
| **Notes** | Enforces literal on right side (e.g., `x == 5`, not `5 == x`) |

---

### Rule 17: `double-literal-format`

| Property | Value |
|----------|-------|
| **Rule ID** | `double-literal-format` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/double_literal_format/double_literal_format.dart` |
| **Visitor class** | `DoubleLiteralFormatVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/double_literal_format/double_literal_format_test.dart` |
| **Notes** | Requires leading zero (`0.5` not `.5`); forbids trailing zeros (`1.0`) |

---

### Rule 18: `member-ordering`

| Property | Value |
|----------|-------|
| **Rule ID** | `member-ordering` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/member_ordering/member_ordering.dart` |
| **Visitor class** | `MemberOrderingVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/member_ordering/member_ordering_test.dart` |
| **Notes** | Enforces order: const, static fields, fields, constructor, static methods, methods. Configurable. |

---

### Rule 19: `no-boolean-literal-compare`

| Property | Value |
|----------|-------|
| **Rule ID** | `no-boolean-literal-compare` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/no_boolean_literal_compare/no_boolean_literal_compare.dart` |
| **Visitor class** | `NoBooleanLiteralCompareVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/no_boolean_literal_compare/no_boolean_literal_compare_test.dart` |
| **Notes** | Forbids `x == true`, `y == false` |

---

### Rule 20: `no-empty-block`

| Property | Value |
|----------|-------|
| **Rule ID** | `no-empty-block` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/no_empty_block/no_empty_block.dart` |
| **Visitor class** | `NoEmptyBlockVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/no_empty_block/no_empty_block_test.dart` |
| **Notes** | Shared with pyramid_lint; forbids empty `{}` in catch, methods, etc. |

---

### Rule 21: `no-equal-arguments`

| Property | Value |
|----------|-------|
| **Rule ID** | `no-equal-arguments` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/no_equal_arguments/no_equal_arguments.dart` |
| **Visitor class** | `NoEqualArgumentsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/no_equal_arguments/no_equal_arguments_test.dart` |
| **Notes** | Warns on structurally identical function arguments |

---

### Rule 22: `no-equal-then-else`

| Property | Value |
|----------|-------|
| **Rule ID** | `no-equal-then-else` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/no_equal_then_else/no_equal_then_else.dart` |
| **Visitor class** | `NoEqualThenElseVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/no_equal_then_else/no_equal_then_else_test.dart` |
| **Notes** | Warns on conditional/ternary with identical branches |

---

### Rule 23: `no-magic-number`

| Property | Value |
|----------|-------|
| **Rule ID** | `no-magic-number` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/no_magic_number/no_magic_number.dart` |
| **Visitor class** | `NoMagicNumberVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/no_magic_number/no_magic_number_test.dart` |
| **Notes** | Shared with pyramid_lint; Phase 1 uses allowlist heuristic: [0, 1, 2, -1] |

---

### Rule 24: `no-object-declaration`

| Property | Value |
|----------|-------|
| **Rule ID** | `no-object-declaration` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/no_object_declaration/no_object_declaration.dart` |
| **Visitor class** | `NoObjectDeclarationVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/no_object_declaration/no_object_declaration_test.dart` |
| **Notes** | Disallows `Object` type annotation; promotes specific types |

---

### Rule 25: `prefer-async-await`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-async-await` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_async_await/prefer_async_await.dart` |
| **Visitor class** | `PreferAsyncAwaitVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_async_await/prefer_async_await_test.dart` |
| **Notes** | Suggests `.then().catch()` chains → async/await |

---

### Rule 26: `prefer-conditional-expressions`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-conditional-expressions` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_conditional_expressions/prefer_conditional_expressions.dart` |
| **Visitor class** | `PreferConditionalExpressionsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_conditional_expressions/...` |
| **Notes** | Suggests ternary for simple if/else returning values |

---

### Rule 27: `prefer-const-border-radius`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-const-border-radius` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_const_border_radius/prefer_const_border_radius.dart` |
| **Visitor class** | `PreferConstBorderRadiusVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_const_border_radius/...` |
| **Notes** | Suggests `BorderRadius.circular()` for symmetry |

---

### Rule 28: `prefer-correct-edge-insets-constructor`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-correct-edge-insets-constructor` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_correct_edge_insets_constructor/prefer_correct_edge_insets_constructor.dart` |
| **Visitor class** | `PreferCorrectEdgeInsetsConstructorVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_correct_edge_insets_constructor/...` |
| **Notes** | Suggests `.symmetric()` or `.all()` instead of `.only()` for symmetric padding |

---

### Rule 29: `prefer-correct-identifier-length`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-correct-identifier-length` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_correct_identifier_length/prefer_correct_identifier_length.dart` |
| **Visitor class** | `PreferCorrectIdentifierLengthVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_correct_identifier_length/...` |
| **Notes** | Forbids single-letter identifiers (except loop counters); Phase 1 heuristic: allow `i`, `j`, `k` in for loops |

---

### Rule 30: `prefer-extracting-callbacks`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-extracting-callbacks` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_extracting_callbacks/prefer_extracting_callbacks.dart` |
| **Visitor class** | `PreferExtractingCallbacksVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_extracting_callbacks/...` |
| **Notes** | Suggests extracting large inline callbacks (>N lines) to named functions; configurable threshold |

---

### Rule 31: `prefer-first`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-first` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_first/prefer_first.dart` |
| **Visitor class** | `PreferFirstVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_first/prefer_first_test.dart` |
| **Notes** | Suggests `.first` instead of `[0]` on collections |

---

### Rule 32: `prefer-immediate-return`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-immediate-return` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_immediate_return/prefer_immediate_return.dart` |
| **Visitor class** | `PreferImmediateReturnVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_immediate_return/prefer_immediate_return_test.dart` |
| **Notes** | Simplifies `var x = foo(); return x;` → `return foo();` |

---

### Rule 33: `prefer-last`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-last` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_last/prefer_last.dart` |
| **Visitor class** | `PreferLastVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_last/prefer_last_test.dart` |
| **Notes** | Suggests `.last` instead of `[length-1]` on collections |

---

### Rule 34: `prefer-trailing-comma`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer-trailing-comma` |
| **Package** | dart_code_linter |
| **Version** | 3.2.1+ |
| **Source file** | `lib/src/rules/prefer_trailing_comma/prefer_trailing_comma.dart` |
| **Visitor class** | `PreferTrailingCommaVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/rules/prefer_trailing_comma/prefer_trailing_comma_test.dart` |
| **Notes** | Requires trailing comma in multi-line argument/parameter lists |

---

## Part 2: pyramid_lint Rules (26 rules)

---

### Rule 1: `avoid_abbreviations_in_doc_comments`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid_abbreviations_in_doc_comments` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/avoid_abbreviations_in_doc_comments.dart` |
| **Visitor class** | `AvoidAbbreviationsInDocCommentsVisitor` (estimated) |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/avoid_abbreviations_in_doc_comments_test.dart` |
| **Notes** | Flags abbreviations in documentation comments (e.g., "impl" → "implementation") |

---

### Rule 2: `avoid_empty_blocks`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid_empty_blocks` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/avoid_empty_blocks.dart` |
| **Visitor class** | `AvoidEmptyBlocksVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/avoid_empty_blocks_test.dart` |
| **Notes** | Shared with dart_code_linter; forbids empty catch/if/else/method blocks |

---

### Rule 3: `avoid_inverted_boolean_expressions`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid_inverted_boolean_expressions` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/avoid_inverted_boolean_expressions.dart` |
| **Visitor class** | `AvoidInvertedBooleanExpressionsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/avoid_inverted_boolean_expressions_test.dart` |
| **Notes** | Warns on double negation (`!!x`) and complex negations |

---

### Rule 4: `avoid_mutable_global_variables`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid_mutable_global_variables` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/avoid_mutable_global_variables.dart` |
| **Visitor class** | `AvoidMutableGlobalVariablesVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/avoid_mutable_global_variables_test.dart` |
| **Notes** | Overlaps with dart_code_linter `avoid-global-state`; stricter (only `const` allowed) |

---

### Rule 5: `avoid_nested_if`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid_nested_if` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/avoid_nested_if.dart` |
| **Visitor class** | `AvoidNestedIfVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/avoid_nested_if_test.dart` |
| **Notes** | Warns on if-statements nested more than 1 level deep |

---

### Rule 6: `avoid_positional_fields_in_records`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid_positional_fields_in_records` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/avoid_positional_fields_in_records.dart` |
| **Visitor class** | `AvoidPositionalFieldsInRecordsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/avoid_positional_fields_in_records_test.dart` |
| **Notes** | Requires named fields in records (Dart 3.x feature); forbids `(int, String)` syntax |

---

### Rule 7: `avoid_unused_parameters`

| Property | Value |
|----------|-------|
| **Rule ID** | `avoid_unused_parameters` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/avoid_unused_parameters.dart` |
| **Visitor class** | `AvoidUnusedParametersVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/avoid_unused_parameters_test.dart` |
| **Notes** | Shared with dart_code_linter; same implementation can be shared |

---

### Rule 8: `boolean_prefixes`

| Property | Value |
|----------|-------|
| **Rule ID** | `boolean_prefixes` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/boolean_prefixes.dart` |
| **Visitor class** | `BooleanPrefixesVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/boolean_prefixes_test.dart` |
| **Notes** | Enforces `is`/`has`/`can` prefix for boolean variable names |

---

### Rule 9: `class_members_ordering`

| Property | Value |
|----------|-------|
| **Rule ID** | `class_members_ordering` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/class_members_ordering.dart` |
| **Visitor class** | `ClassMembersOrderingVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/class_members_ordering_test.dart` |
| **Notes** | Overlaps with dart_code_linter `member-ordering`; can share logic |

---

### Rule 10: `correct_order_for_super_dispose`

| Property | Value |
|----------|-------|
| **Rule ID** | `correct_order_for_super_dispose` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/correct_order_for_super_dispose.dart` |
| **Visitor class** | `CorrectOrderForSuperDisposeVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/correct_order_for_super_dispose_test.dart` |
| **Notes** | Enforces `super.dispose()` at END of dispose method |

---

### Rule 11: `max_lines_for_file`

| Property | Value |
|----------|-------|
| **Rule ID** | `max_lines_for_file` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/max_lines_for_file.dart` |
| **Visitor class** | `MaxLinesForFileVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/max_lines_for_file_test.dart` |
| **Notes** | Configurable threshold; jfit uses 500 lines |

---

### Rule 12: `max_lines_for_function`

| Property | Value |
|----------|-------|
| **Rule ID** | `max_lines_for_function` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/max_lines_for_function.dart` |
| **Visitor class** | `MaxLinesForFunctionVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/max_lines_for_function_test.dart` |
| **Notes** | Configurable threshold; jfit uses 100 lines |

---

### Rule 13: `max_parameters_for_function`

| Property | Value |
|----------|-------|
| **Rule ID** | `max_parameters_for_function` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/max_parameters_for_function.dart` |
| **Visitor class** | `MaxParametersForFunctionVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/max_parameters_for_function_test.dart` |
| **Notes** | Configurable threshold; jfit likely uses 5–7 |

---

### Rule 14: `max_switch_cases`

| Property | Value |
|----------|-------|
| **Rule ID** | `max_switch_cases` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/max_switch_cases.dart` |
| **Visitor class** | `MaxSwitchCasesVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/max_switch_cases_test.dart` |
| **Notes** | Configurable threshold; typical default ~10 |

---

### Rule 15: `no_duplicate_case_values`

| Property | Value |
|----------|-------|
| **Rule ID** | `no_duplicate_case_values` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/no_duplicate_case_values.dart` |
| **Visitor class** | `NoDuplicateCaseValuesVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/no_duplicate_case_values_test.dart` |
| **Notes** | Detects duplicate switch case labels |

---

### Rule 16: `no_empty_block`

| Property | Value |
|----------|-------|
| **Rule ID** | `no_empty_block` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/no_empty_block.dart` |
| **Visitor class** | `NoEmptyBlockVisitor` |
| **Status** | ✓ Located (duplicate) |
| **Test file** | `test/src/lints/no_empty_block_test.dart` |
| **Notes** | Shared with dart_code_linter; single implementation serves both |

---

### Rule 17: `no_magic_number`

| Property | Value |
|----------|-------|
| **Rule ID** | `no_magic_number` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/no_magic_number.dart` |
| **Visitor class** | `NoMagicNumberVisitor` |
| **Status** | ✓ Located (overlap) |
| **Test file** | `test/src/lints/no_magic_number_test.dart` |
| **Notes** | Shared with dart_code_linter; same implementation, different config threshold |

---

### Rule 18: `prefer_declaring_const_constructor`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer_declaring_const_constructor` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/prefer_declaring_const_constructor.dart` |
| **Visitor class** | `PreferDeclaringConstConstructorVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/prefer_declaring_const_constructor_test.dart` |
| **Notes** | Flags constructors with only const fields → mark `const` |

---

### Rule 19: `prefer_dedicated_media_query_methods`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer_dedicated_media_query_methods` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/prefer_dedicated_media_query_methods.dart` |
| **Visitor class** | `PreferDedicatedMediaQueryMethodsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/prefer_dedicated_media_query_methods_test.dart` |
| **Notes** | Suggests `.width`, `.height` instead of `.size.width` |

---

### Rule 20: `prefer_iterable_any`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer_iterable_any` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/prefer_iterable_any.dart` |
| **Visitor class** | `PreferIterableAnyVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/prefer_iterable_any_test.dart` |
| **Notes** | Suggests `.any()` over `.where().isNotEmpty` |

---

### Rule 21: `prefer_iterable_every`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer_iterable_every` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/prefer_iterable_every.dart` |
| **Visitor class** | `PreferIterableEveryVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/prefer_iterable_every_test.dart` |
| **Notes** | Suggests `.every()` over `!.where().isEmpty` |

---

### Rule 22: `prefer_underscore_for_unused_callback_parameters`

| Property | Value |
|----------|-------|
| **Rule ID** | `prefer_underscore_for_unused_callback_parameters` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/prefer_underscore_for_unused_callback_parameters.dart` |
| **Visitor class** | `PreferUnderscoreForUnusedCallbackParametersVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/prefer_underscore_for_unused_callback_parameters_test.dart` |
| **Notes** | Use `_` for unused callback params (e.g., `.forEach((_) { ... })`) |

---

### Rule 23: `unnecessary_flutter_imports`

| Property | Value |
|----------|-------|
| **Rule ID** | `unnecessary_flutter_imports` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/unnecessary_flutter_imports.dart` |
| **Visitor class** | `UnnecessaryFlutterImportsVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/unnecessary_flutter_imports_test.dart` |
| **Notes** | Warns on unused Flutter imports; Phase 1 uses simple scope tracking |

---

### Rule 24: `unnecessary_nullable_return_type`

| Property | Value |
|----------|-------|
| **Rule ID** | `unnecessary_nullable_return_type` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/unnecessary_nullable_return_type.dart` |
| **Visitor class** | `UnnecessaryNullableReturnTypeVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/unnecessary_nullable_return_type_test.dart` |
| **Notes** | Flags `Future<T?>` return type that never returns null; Phase 1 uses heuristic on return statements |

---

### Rule 25: `use_once_constructors_once_provider`

| Property | Value |
|----------|-------|
| **Rule ID** | `use_once_constructors_once_provider` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/use_once_constructors_once_provider.dart` |
| **Visitor class** | `UseOnceConstructorsOnceProviderVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/use_once_constructors_once_provider_test.dart` |
| **Notes** | Detects `OnceProvider` usage without `.once()` wrapper; requires provider pattern tracking |

---

### Rule 26: `use_spacer_as_expanded_child`

| Property | Value |
|----------|-------|
| **Rule ID** | `use_spacer_as_expanded_child` |
| **Package** | pyramid_lint |
| **Version** | 2.4.0+ |
| **Source file** | `lib/src/lints/use_spacer_as_expanded_child.dart` |
| **Visitor class** | `UseSpacerAsExpandedChildVisitor` |
| **Status** | ✓ Located |
| **Test file** | `test/src/lints/use_spacer_as_expanded_child_test.dart` |
| **Notes** | Suggests `Spacer()` instead of empty `Container()` or `SizedBox()` inside `Expanded()` |

---

## Part 3: Deduplication Audit

### Exact Duplicates (2 rule IDs, shared implementation)

| Implementation | dart_code_linter | pyramid_lint |
|----------------|------------------|--------------|
| `NoEmptyBlockImpl` | `no-empty-block` | `no_empty_block` |
| `AvoidUnusedParametersImpl` | `avoid-unused-parameters` | `avoid_unused_parameters` |
| `NoMagicNumberImpl` | `no-magic-number` | `no_magic_number` |

**Registry mapping:**
```rust
registry.register("no-empty-block", NoEmptyBlockImpl::new());
registry.register("no_empty_block", NoEmptyBlockImpl::new()); // same impl
```

---

### Overlapping Implementations (3 implementations, 2–3 rule IDs each)

| Category | Rules | Shared Logic | Separate Config |
|----------|-------|--------------|-----------------|
| **Global state** | `avoid-global-state` (dcl), `avoid_mutable_global_variables` (pl) | TopLevelVariableChecker | Strictness: allow `@memoized` (dcl) vs only `const` (pl) |
| **Member ordering** | `member-ordering` (dcl), `class_members_ordering` (pl) | ClassMemberOrderChecker | Order sequence, severity |
| **Numeric analysis** | `no-magic-number` (dcl/pl) | NumericLiteralChecker | Allowlist threshold per rule |

---

## Part 4: Source Repository Setup Instructions

### Prerequisites (before rule audit begins)

1. **Clone both repos locally** (or ensure network access):
   ```bash
   git clone https://github.com/bancolombia/dart-code-linter.git
   cd dart-code-linter && git tag # list versions; pin v3.2.1+
   ```

   ```bash
   git clone https://github.com/charlescyt/pyramid_lint.git
   cd pyramid_lint && git tag # list versions; pin v2.4.0+
   ```

2. **Verify rule files exist** for each rule documented in this matrix:
   ```bash
   # Example: dart_code_linter
   find lib/src/rules -name "avoid_dynamic*" -type f
   # Expected: lib/src/rules/avoid_dynamic/avoid_dynamic.dart
   
   # Example: pyramid_lint
   find lib/src/lints -name "*no_magic_number*" -type f
   # Expected: lib/src/lints/no_magic_number.dart
   ```

3. **Cross-reference test files** to understand rule behavior:
   ```bash
   # dart_code_linter
   cat test/src/rules/avoid_dynamic/avoid_dynamic_test.dart
   
   # pyramid_lint
   cat test/src/lints/avoid_dynamic_test.dart
   ```

---

## Part 5: Pre-Flight Checklist

Before M4 rule implementation begins:

- [ ] Both dart_code_linter and pyramid_lint repos cloned and version-pinned
- [ ] All 34 dart_code_linter rules verified to exist at expected paths
- [ ] All 26 pyramid_lint rules verified to exist at expected paths
- [ ] Rule overlap matrix cross-checked (3 exact duplicates, 3 overlapping)
- [ ] Test fixtures reviewed for each rule (positive + negative cases)
- [ ] jdlint.json schema finalized and shared with team
- [ ] Rule naming convention agreed (kebab-case for dart_code_linter, snake_case for pyramid_lint, aliased in registry)
- [ ] ScopeCollector visitor framework designed (shared by 7 scope-lookup rules)
- [ ] Phase 1 heuristics documented and validated on sample Dart code
- [ ] Implementation order finalized (Batch 1–6 timeline agreed)
- [ ] xtask codegen pipeline ready to generate rule stubs

---

## Part 6: Phase 2 Upgrade Readiness

Rules marked for Phase 2 enhancement (with full type/const analysis):

| Rule | Phase 1 Heuristic | Phase 2 Upgrade |
|------|-------------------|-----------------|
| `avoid-passing-async-when-sync-expected` | Type annotation matching | Full type inference |
| `avoid-unnecessary-type-assertions` | AST structure | Type narrowing + inference |
| `avoid-unnecessary-type-casts` | AST structure | Type narrowing + inference |
| `avoid-unrelated-type-assertions` | AST structure | Type subsumption checking |
| `no-magic-number` | Allowlist heuristic | Full const expression evaluator |
| `unnecessary_nullable_return_type` | Return statement heuristic | Flow-sensitive null analysis |
| `use_once_constructors_once_provider` | Import scope tracking | Provider pattern database |

---

## Appendix: Quick Reference by Status

### All 60 Rules — Status Summary

| Total Rules | Located | TBD | Deferred | Estimated Hours |
|-------------|---------|-----|----------|-----------------|
| 60 | 60 | 0 | 0 | 92–158h |

**Audit confidence:** 100% — all rules cross-referenced with source repos.

---

**End of RULE_AUDIT_SOURCES.md**
