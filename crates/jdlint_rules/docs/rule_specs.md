# jdlint Rule Specification Document

**Version:** M4.0  
**Date:** 2026-06-10  
**Scope:** Complete specification for all 60 lint rules (M4.2–M4.6)  
**Audience:** Rule implementors, testers, and integrators

---

## Table of Contents

- [Part 1: SIMPLE Rules (40 rules)](#part-1-simple-rules)
- [Part 2: MEDIUM Rules (14 rules)](#part-2-medium-rules)
- [Part 3: COMPLEX Rules (6 rules)](#part-3-complex-rules)
- [Deduplication & Overlaps](#deduplication--overlaps)

---

# PART 1: SIMPLE RULES

## 1. avoid-dynamic

**Rule ID:** `avoid-dynamic`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** error  

### Description

Flags the use of the `dynamic` type annotation. Dynamic typing bypasses type safety and makes code harder to reason about. Use `Object`, specific types, or `var` with type inference instead.

### Phase 1 Heuristic

Match the `dynamic` keyword in type name position (variable declaration, function return type, parameter type).

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
dynamic x = 5;
void foo(dynamic arg) {}
dynamic getResult() => compute();
final dynamic result = getData();
```

**Good (does NOT trigger):**
```dart
int x = 5;
void foo(Object arg) {}
Future<int> getResult() => compute();
final result = getData(); // type inferred
```

**Edge case:**
```dart
// Comments mentioning 'dynamic' should NOT trigger
var x = 5; // dynamic type is inferred
```

### Diagnostic Message

```
Avoid using the dynamic type annotation. Use a specific type or Object instead.
```

### Acceptance Criteria (M4.8)

- Detects `dynamic` in variable declarations, function returns, and parameters
- Does NOT flag comments or string literals containing "dynamic"
- Ignores `dynamic` as a symbol name (e.g., method named `dynamic()`)
- All test cases pass

---

## 2. avoid-ignoring-return-values

**Rule ID:** `avoid-ignoring-return-values`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags function calls whose return values are not used. Ignoring return values often indicates logic errors or forgotten error handling.

### Phase 1 Heuristic

A function/method call is an expression statement with a FunctionCall or MethodInvocation node. Report if not assigned, passed as argument, or returned.

### Configuration

Optional allowlist of function names to ignore (e.g., `print`, `logger.debug`).

### Examples

**Bad (triggers rule):**
```dart
foo();  // return value ignored
list.add(5);  // return value (bool) ignored
await asyncFn();  // Future return ignored
```

**Good (does NOT trigger):**
```dart
var x = foo();  // assigned
list.add(5) ? print("ok") : print("fail");  // used in expr
return asyncFn();  // returned
foo(bar());  // used as argument
```

**Edge case:**
```dart
void foo() {}
foo();  // OK: void return is expected to be ignored
```

### Diagnostic Message

```
The return value of 'foo' is ignored. Did you forget to use it?
```

### Acceptance Criteria (M4.8)

- Flags all non-void function calls with unused return values
- Allows void functions (no warning)
- Respects allowlist configuration
- Does NOT flag calls in void contexts (e.g., statement body of `void foo()`)

---

## 3. avoid-late-keyword

**Rule ID:** `avoid-late-keyword`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags the use of the `late` keyword. Late initialization makes code harder to reason about and can mask initialization bugs.

### Phase 1 Heuristic

Match the `late` modifier in variable declarations.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
late int value;
late final String name = computeName();
class Foo {
  late List items;
}
```

**Good (does NOT trigger):**
```dart
int? value;
int value = 0;
final String name = computeName();
class Foo {
  final List items;
}
```

### Diagnostic Message

```
Avoid using the 'late' keyword. Use nullable types or initialize fields in constructor.
```

### Acceptance Criteria (M4.8)

- Detects `late` modifier in all variable declarations
- Works in class fields, local vars, and top-level vars
- All test cases pass

---

## 4. avoid-nested-conditional-expressions

**Rule ID:** `avoid-nested-conditional-expressions`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags ternary expressions nested more than one level deep. Nested ternaries are hard to read; extract to named variables or if/else.

### Phase 1 Heuristic

Count depth of ConditionalExpression nesting. Flag if depth > 1.

### Configuration

Optional `maxNestingLevel` (default 1).

### Examples

**Bad (triggers rule):**
```dart
var result = a ? (b ? c : d) : e;  // depth 2
var x = a ? b : (c ? d : e);  // depth 2
var y = a ? (b ? c : (d ? e : f)) : g;  // depth 3
```

**Good (does NOT trigger):**
```dart
var result = a ? b : c;  // depth 1
// Extract to intermediate:
var temp = b ? c : d;
var result = a ? temp : e;
```

### Diagnostic Message

```
Avoid nested conditional expressions. Refactor to if/else or intermediate variables.
```

### Acceptance Criteria (M4.8)

- Correctly counts nesting depth
- Respects maxNestingLevel config
- Does NOT flag single-level ternaries
- All edge cases (parenthesized, mixed with if/else) handled

---

## 5. avoid-non-null-assertion

**Rule ID:** `avoid-non-null-assertion`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags the use of the `!` null-assertion operator. Null assertions bypass null safety and can cause runtime crashes. Use proper null checks instead.

### Phase 1 Heuristic

Match PostfixExpression with `!` operator.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
String name = data!.name;
int x = getValue()!;
list.first!.doSomething();
```

**Good (does NOT trigger):**
```dart
String? name = data?.name;
int? x = getValue();
if (data != null) { x = data.name; }
```

### Diagnostic Message

```
Avoid using the non-null assertion operator (!). Use null checks or nullable types.
```

### Acceptance Criteria (M4.8)

- Detects all `!` null assertions
- Does NOT flag `!=` (inequality operator)
- All test cases pass

---

## 6. avoid-throw-in-catch-block

**Rule ID:** `avoid-throw-in-catch-block`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags re-throwing or throwing new exceptions in catch blocks. Catch blocks should log/handle errors or rethrow without modification (use `rethrow` instead).

### Phase 1 Heuristic

Check if CatchClause body contains ThrowStatement.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
try {
  foo();
} catch (e) {
  throw Exception("Failed: $e");
}
try {
  bar();
} catch (e) {
  throw e;  // modify stack trace
}
```

**Good (does NOT trigger):**
```dart
try {
  foo();
} catch (e) {
  logger.error(e);
  rethrow;
}
try {
  bar();
} catch (e) {
  logger.error(e);
  // handle gracefully
}
```

### Diagnostic Message

```
Avoid throwing in catch blocks. Use 'rethrow' to preserve the stack trace, or handle the error.
```

### Acceptance Criteria (M4.8)

- Detects throw statements in catch blocks
- Does NOT flag logging or other calls
- Rethrow is explicitly allowed
- All test cases pass

---

## 7. avoid-top-level-member-access

**Rule ID:** `avoid-top-level-member-access`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags access to non-const top-level variables from other locations. Top-level mutable state is a code smell and hinders testing.

### Phase 1 Heuristic

Track all top-level variable declarations (non-const). Report uses that reference them from within functions.

### Configuration

Optional allowlist of variable names (e.g., `logger`, `config`).

### Examples

**Bad (triggers rule):**
```dart
int globalCounter = 0;  // top-level mutable var
void increment() {
  globalCounter++;  // ✗ accessing mutable top-level var
}

var serviceInstance;
void useService() {
  serviceInstance.call();  // ✗ accessing mutable top-level var
}
```

**Good (does NOT trigger):**
```dart
const int VERSION = 1;
void printVersion() {
  print(VERSION);  // ✓ const is OK
}

class Service {
  int counter = 0;  // member var, not top-level
}
```

### Diagnostic Message

```
Avoid using mutable top-level variables. Use class members or dependency injection.
```

### Acceptance Criteria (M4.8)

- Detects references to non-const top-level vars
- Does NOT flag const variables
- Works with allowlist
- All test cases pass

---

## 8. binary-expression-operand-order

**Rule ID:** `binary-expression-operand-order`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags comparison operators with literal values on the left side. Write `x == 5` not `5 == x` for better readability.

### Phase 1 Heuristic

Check BinaryExpression for comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) where left operand is a literal and right is a variable/expression.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if (5 == x) {}
if (true == isValid) {}
if ("hello" == name) {}
if (null != value) {}
```

**Good (does NOT trigger):**
```dart
if (x == 5) {}
if (isValid == true) {}  // can flip conditionally
if (name == "hello") {}
if (value != null) {}
```

### Diagnostic Message

```
Put literal values on the right side of comparisons. Write 'x == 5' instead of '5 == x'.
```

### Acceptance Criteria (M4.8)

- Detects literal-left-side comparisons
- Works with all comparison operators
- Does NOT flag arithmetic operators
- All test cases pass

---

## 9. double-literal-format

**Rule ID:** `double-literal-format`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Enforces consistent double literal formatting: require leading zero (0.5 not .5), forbid trailing zeros (0.5 not 1.0).

### Phase 1 Heuristic

Inspect DoubleLiteral AST nodes. Check for missing leading zero and trailing zeros.

### Configuration

Optional `allowLeadingDot` (default false), `allowTrailingZeros` (default false).

### Examples

**Bad (triggers rule):**
```dart
double x = .5;  // ✗ missing leading zero
double y = 1.0;  // ✗ trailing zero
double z = .0;
```

**Good (does NOT trigger):**
```dart
double x = 0.5;
double y = 1;  // integer literal instead
double z = 2.5;
```

### Diagnostic Message

```
Double literals should have a leading zero (0.5 not .5) and no trailing zeros.
```

### Acceptance Criteria (M4.8)

- Detects missing leading zero
- Detects trailing zeros
- Respects configuration
- Does NOT flag integer literals
- All test cases pass

---

## 10. no-boolean-literal-compare

**Rule ID:** `no-boolean-literal-compare`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags redundant comparisons with boolean literals. `x == true` is redundant; just write `x`. Similarly, `x == false` should be `!x`.

### Phase 1 Heuristic

Match BinaryExpression with `==` or `!=` where one operand is `true` or `false` literal.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if (isValid == true) {}
if (isEmpty == false) {}
if (x != true) {}
if (y != false) {}
bool result = status == true;
```

**Good (does NOT trigger):**
```dart
if (isValid) {}
if (!isEmpty) {}
if (!x) {}
if (y) {}
bool result = status;
```

### Diagnostic Message

```
Avoid comparing boolean variables with boolean literals. Use the variable directly or apply negation.
```

### Acceptance Criteria (M4.8)

- Detects all boolean literal comparisons
- Works with both `==` and `!=`
- All test cases pass

---

## 11. no-empty-block

**Rule IDs:** `no-empty-block` (dart_code_linter), `no_empty_block` (pyramid_lint)  
**Source:** dart_code_linter + pyramid_lint (shared implementation)  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags empty blocks in function bodies, catch clauses, if/else statements, and loops. Empty blocks indicate incomplete code.

### Phase 1 Heuristic

Check if Block node has no statements (empty statement list).

### Configuration

Optional allowlist of contexts (e.g., allow empty catch blocks).

### Examples

**Bad (triggers rule):**
```dart
void foo() { }  // empty method
try { } catch (e) { }  // empty catch
if (x) { } else { }
for (int i = 0; i < 10; i++) { }
```

**Good (does NOT trigger):**
```dart
void foo() {
  // TODO: implement
}
try { } catch (e) {
  logger.error(e);
  rethrow;
}
if (x) {
  doSomething();
}
```

### Diagnostic Message

```
Avoid empty blocks. Add implementation or a TODO comment.
```

### Acceptance Criteria (M4.8)

- Detects empty blocks in all contexts
- Optional allowlist for specific contexts
- Ignores blocks with comments
- All test cases pass

---

## 12. no-equal-arguments

**Rule ID:** `no-equal-arguments`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags function calls with duplicate arguments. Passing the same value twice is likely a bug.

### Phase 1 Heuristic

Collect all arguments in ArgumentList. Compare AST structure for equality. Report duplicates.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
foo(x, x);  // same argument twice
bar(a, b, a);  // first and third are equal
Container(width: 10, height: 10);  // if width == height value
```

**Good (does NOT trigger):**
```dart
foo(x, y);
bar(a, b, c);
Container(width: 10, height: 20);
```

### Diagnostic Message

```
Avoid passing the same argument multiple times. The argument 'x' is passed twice.
```

### Acceptance Criteria (M4.8)

- Detects structurally equal arguments
- Works with positional and named args
- Reports which arguments are duplicates
- All test cases pass

---

## 13. no-equal-then-else

**Rule ID:** `no-equal-then-else`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags if/else or ternary statements where the then-branch and else-branch produce identical results. This code is always redundant.

### Phase 1 Heuristic

Compare AST structure of then-block and else-block. If identical, report.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if (x) { return a; } else { return a; }  // both return same value
var result = x ? value : value;  // same value in both branches
if (condition) { doX(); } else { doX(); }  // same call in both
```

**Good (does NOT trigger):**
```dart
if (x) { return a; } else { return b; }
var result = x ? aValue : bValue;
return x ? foo() : bar();
```

### Diagnostic Message

```
The then-branch and else-branch produce identical results. Simplify to remove the condition.
```

### Acceptance Criteria (M4.8)

- Detects equal branches in if/else and ternary
- Works with nested blocks
- All test cases pass

---

## 14. no-object-declaration

**Rule ID:** `no-object-declaration`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags use of `Object` type annotation in variable declarations. Use specific types or `dynamic` instead (though `dynamic` is also discouraged).

### Phase 1 Heuristic

Match `Object` type name in variable declaration context.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
Object value = 5;
void foo(Object arg) {}
final Object result = getData();
```

**Good (does NOT trigger):**
```dart
int value = 5;
void foo(dynamic arg) {}
final result = getData();  // inferred type
```

### Diagnostic Message

```
Avoid using the Object type. Use a specific type or dynamic instead.
```

### Acceptance Criteria (M4.8)

- Detects `Object` in variable declarations and parameters
- Does NOT flag `Object` as a class base
- All test cases pass

---

## 15. prefer-async-await

**Rule ID:** `prefer-async-await`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests replacing `.then().catch()` chains with async/await syntax. Async/await is more readable and less error-prone.

### Phase 1 Heuristic

Detect MethodInvocation chain with `.then()` or `.catch()` methods on a Future. Suggest async/await if equivalent.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
Future<int> foo() => getData().then((x) => x * 2);
foo().then((x) { print(x); }).catch((e) { print(e); });
```

**Good (does NOT trigger):**
```dart
Future<int> foo() async => (await getData()) * 2;
async { 
  try { print(await foo()); } 
  catch (e) { print(e); } 
}
```

### Diagnostic Message

```
Use async/await instead of .then().catch() chains. It's more readable and easier to debug.
```

### Acceptance Criteria (M4.8)

- Detects .then() and .catch() chains
- Only suggests when refactoring is straightforward
- All test cases pass

---

## 16. prefer-const-border-radius

**Rule ID:** `prefer-const-border-radius`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests `BorderRadius.circular()` instead of redundant `.all()` or symmetry shortcuts when all corners have the same radius.

### Phase 1 Heuristic

Detect InstanceCreation of BorderRadius with all-equal values. Suggest `.circular()`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
BorderRadius.only(
  topLeft: Radius.circular(8),
  topRight: Radius.circular(8),
  bottomLeft: Radius.circular(8),
  bottomRight: Radius.circular(8),
)
BorderRadius.all(Radius.circular(8))
```

**Good (does NOT trigger):**
```dart
BorderRadius.circular(8)
BorderRadius.only(topLeft: Radius.circular(8), topRight: Radius.circular(4))
```

### Diagnostic Message

```
Use BorderRadius.circular(radius) instead of specifying all corners. It's more concise.
```

### Acceptance Criteria (M4.8)

- Detects redundant BorderRadius declarations
- Suggests appropriate constructors
- All test cases pass

---

## 17. prefer-correct-edge-insets-constructor

**Rule ID:** `prefer-correct-edge-insets-constructor`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests using the most appropriate `EdgeInsets` constructor (`.symmetric()`, `.only()`, `.all()`) for the given values.

### Phase 1 Heuristic

Detect EdgeInsets creation. Check if values form a symmetric or all-equal pattern. Suggest the simplest constructor.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
EdgeInsets.only(top: 8, bottom: 8)  // should be .symmetric(vertical: 8)
EdgeInsets.only(left: 4, right: 4, top: 4, bottom: 4)  // should be .all(4)
```

**Good (does NOT trigger):**
```dart
EdgeInsets.symmetric(vertical: 8)
EdgeInsets.all(4)
EdgeInsets.only(left: 4, right: 8, top: 2, bottom: 6)
```

### Diagnostic Message

```
Use the most concise EdgeInsets constructor. Consider .symmetric() or .all() for common patterns.
```

### Acceptance Criteria (M4.8)

- Detects symmetry patterns
- Suggests correct constructors
- All test cases pass

---

## 18. prefer-correct-identifier-length

**Rule ID:** `prefer-correct-identifier-length`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags overly short identifier names (single letters), except for loop counters (i, j, k) and universally understood abbreviations (x, y for coords).

### Phase 1 Heuristic

Check all identifiers in variable/parameter declarations. Flag if length == 1 and not in allowlist/scope context.

### Configuration

Optional allowlist (default: `["i", "j", "k", "x", "y", "z"]`).

### Examples

**Bad (triggers rule):**
```dart
var a = 5;  // too short
void foo(String b) {}
final c = getData();
```

**Good (does NOT trigger):**
```dart
var count = 5;
void foo(String name) {}
final result = getData();
for (int i = 0; i < 10; i++) {}  // loop counter OK
```

### Diagnostic Message

```
Identifier name is too short. Use a more descriptive name.
```

### Acceptance Criteria (M4.8)

- Detects short identifiers
- Allows loop counters and coords
- Respects configuration
- All test cases pass

---

## 19. prefer-first

**Rule ID:** `prefer-first`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests using `.first` property instead of `[0]` to access the first element of collections.

### Phase 1 Heuristic

Detect IndexAccess with literal `0` index on a collection/iterable. Suggest `.first`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
var first = list[0];
final x = items[0];
String firstChar = "hello"[0];  // String is iterable
```

**Good (does NOT trigger):**
```dart
var first = list.first;
final x = items.first;
String firstChar = "hello".codeUnitAt(0);  // more appropriate
```

### Diagnostic Message

```
Use .first property instead of [0] to access the first element.
```

### Acceptance Criteria (M4.8)

- Detects [0] access on collections
- Works with all iterable types
- All test cases pass

---

## 20. prefer-immediate-return

**Rule ID:** `prefer-immediate-return`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Simplifies `var x = foo(); return x;` to `return foo();`. Removes unnecessary intermediate variables.

### Phase 1 Heuristic

Detect pattern: variable declaration assigned a function call, followed immediately by return of that variable with no other uses.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
var x = foo();
return x;

final result = getData();
return result;
```

**Good (does NOT trigger):**
```dart
return foo();

var x = foo();
doSomething(x);  // x is used elsewhere
return x;
```

### Diagnostic Message

```
Return the result directly instead of assigning to an intermediate variable.
```

### Acceptance Criteria (M4.8)

- Detects unnecessary intermediate variables
- Only flags if variable used only in return
- All test cases pass

---

## 21. prefer-last

**Rule ID:** `prefer-last`  
**Source:** dart_code_linter  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests using `.last` property instead of `[length - 1]` to access the last element of collections.

### Phase 1 Heuristic

Detect IndexAccess with `[length - 1]` or `[.length - 1]` pattern. Suggest `.last`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
var last = list[list.length - 1];
final x = items[items.length - 1];
```

**Good (does NOT trigger):**
```dart
var last = list.last;
final x = items.last;
```

### Diagnostic Message

```
Use .last property instead of [length - 1] to access the last element.
```

### Acceptance Criteria (M4.8)

- Detects [length - 1] patterns
- Works with all iterable types
- All test cases pass

---

## 22. avoid_abbreviations_in_doc_comments

**Rule ID:** `avoid_abbreviations_in_doc_comments`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags abbreviations in documentation comments. Use full words for clarity (e.g., "implementation" not "impl", "argument" not "arg").

### Phase 1 Heuristic

Scan all documentation comments. Match common abbreviations against a dictionary.

### Configuration

Optional allowlist of acceptable abbreviations (default: common ones like "i.e.", "e.g.").

### Examples

**Bad (triggers rule):**
```dart
/// Impl of the core logic
/// Returns the approx value
/// Params: x, y
void compute(int x, int y) {}
```

**Good (does NOT trigger):**
```dart
/// Implementation of the core logic
/// Returns the approximate value
/// Parameters: x, y
void compute(int x, int y) {}
```

### Diagnostic Message

```
Avoid abbreviations in documentation comments. Use 'implementation' instead of 'impl'.
```

### Acceptance Criteria (M4.8)

- Detects common abbreviations
- Respects allowlist
- Works in all doc comment types
- All test cases pass

---

## 23. avoid_empty_blocks

**Rule ID:** `avoid_empty_blocks`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags empty blocks (same as `no-empty-block`). See rule #11 for details.

### Diagnostic Message

```
Avoid empty blocks. Add implementation or a TODO comment.
```

### Acceptance Criteria (M4.8)

Refer to rule #11 acceptance criteria.

---

## 24. avoid_inverted_boolean_expressions

**Rule ID:** `avoid_inverted_boolean_expressions`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags double negations (`!!x`) and inverted expressions that can be simplified. Write `x` instead of `!!x`.

### Phase 1 Heuristic

Detect UnaryExpression with `!` where operand is itself a UnaryExpression with `!`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if (!!x) {}  // double negation
bool result = !!value;
if (!(!isValid)) {}
```

**Good (does NOT trigger):**
```dart
if (x) {}
bool result = value;
if (isValid) {}
```

### Diagnostic Message

```
Avoid double negation. Use the value directly or apply a single negation.
```

### Acceptance Criteria (M4.8)

- Detects double negations
- Simplifies complex inverted expressions
- All test cases pass

---

## 25. avoid_nested_if

**Rule ID:** `avoid_nested_if`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags if-statements nested more than a configurable depth. Excessive nesting reduces readability.

### Phase 1 Heuristic

Count nesting depth of IfStatement nodes. Flag if depth exceeds threshold.

### Configuration

Optional `maxNestingLevel` (default 2).

### Examples

**Bad (triggers rule):**
```dart
if (x) {
  if (y) {
    if (z) {  // depth 3
      doSomething();
    }
  }
}
```

**Good (does NOT trigger):**
```dart
if (x && y && z) {
  doSomething();
}
if (x) {
  if (y) {
    doSomething();  // depth 2, OK
  }
}
```

### Diagnostic Message

```
Avoid deeply nested if-statements. Consider combining conditions or extracting to a helper method.
```

### Acceptance Criteria (M4.8)

- Counts nesting depth correctly
- Respects maxNestingLevel configuration
- All test cases pass

---

## 26. avoid_positional_fields_in_records

**Rule ID:** `avoid_positional_fields_in_records`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags records with positional fields. Use named fields for clarity (e.g., `({int x, String y})` instead of `(int, String)`).

### Phase 1 Heuristic

Detect RecordLiteral or RecordType. Check if any fields are positional (not named).

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
var record = (1, "hello");  // positional fields
final (int, String) result = getData();
```

**Good (does NOT trigger):**
```dart
var record = (id: 1, name: "hello");
final ({int id, String name}) result = getData();
```

### Diagnostic Message

```
Use named fields in records for better clarity. Write ({int x, String y}) instead of (int, String).
```

### Acceptance Criteria (M4.8)

- Detects positional record fields
- Works with both literals and types
- All test cases pass

---

## 27. boolean_prefixes

**Rule ID:** `boolean_prefixes`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Enforces boolean variable names to start with `is`, `has`, or `can` prefix. Makes boolean semantics clear.

### Phase 1 Heuristic

Detect VariableDeclaration with `bool` type. Check if identifier starts with `is`, `has`, or `can`.

### Configuration

Optional allowed prefixes (default: `["is", "has", "can"]`).

### Examples

**Bad (triggers rule):**
```dart
bool active = true;
bool visible;
bool ready = false;
```

**Good (does NOT trigger):**
```dart
bool isActive = true;
bool isVisible;
bool isReady = false;
bool hasData = false;
bool canEdit = true;
```

### Diagnostic Message

```
Boolean variables should use a prefix like 'is', 'has', or 'can'. Use 'isActive' instead of 'active'.
```

### Acceptance Criteria (M4.8)

- Detects boolean variables without prefix
- Works with all declaration contexts
- Respects configuration
- All test cases pass

---

## 28. correct_order_for_super_dispose

**Rule ID:** `correct_order_for_super_dispose`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Enforces that `super.dispose()` is called at the END of a dispose method. Other cleanup should happen first.

### Phase 1 Heuristic

Detect MethodDeclaration with name `dispose`. Check if it contains SuperInvocation to `dispose()`. Flag if not the last statement.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
@override
void dispose() {
  super.dispose();  // ✗ should be at end
  controller.dispose();
}
```

**Good (does NOT trigger):**
```dart
@override
void dispose() {
  controller.dispose();  // cleanup first
  super.dispose();  // ✓ at end
}
```

### Diagnostic Message

```
Call super.dispose() at the end of the dispose method, after cleaning up resources.
```

### Acceptance Criteria (M4.8)

- Detects dispose() method
- Checks for super.dispose() at end
- Only flags Flutter/Dart dispose patterns
- All test cases pass

---

## 29. max_lines_for_file

**Rule ID:** `max_lines_for_file`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags files exceeding a maximum line count. Large files are harder to understand and maintain.

### Phase 1 Heuristic

Count total lines in CompilationUnit. Compare against threshold.

### Configuration

Required `threshold` (default 500).

### Examples

**Bad (triggers rule):**
```
file_with_2000_lines.dart  // exceeds 500
```

**Good (does NOT trigger):**
```
file_with_300_lines.dart
```

### Diagnostic Message

```
File exceeds the maximum line count of 500 lines. Consider splitting into multiple files.
```

### Acceptance Criteria (M4.8)

- Counts lines accurately
- Respects threshold configuration
- Works with all file types
- All test cases pass

---

## 30. max_lines_for_function

**Rule ID:** `max_lines_for_function`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags functions/methods exceeding a maximum line count. Large functions are hard to test and understand.

### Phase 1 Heuristic

For each FunctionDeclaration or MethodDeclaration, count lines from start to end. Compare against threshold.

### Configuration

Required `threshold` (default 100).

### Examples

**Bad (triggers rule):**
```dart
void complexFunction() {  // 200+ lines
  ...
}
```

**Good (does NOT trigger):**
```dart
void simpleFunction() {  // 50 lines
  ...
}
```

### Diagnostic Message

```
Function exceeds the maximum line count of 100 lines. Consider extracting logic.
```

### Acceptance Criteria (M4.8)

- Counts function lines accurately
- Respects threshold configuration
- Works with all function types
- All test cases pass

---

## 31. max_parameters_for_function

**Rule ID:** `max_parameters_for_function`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags functions with too many parameters. Many parameters are a code smell; use objects or records instead.

### Phase 1 Heuristic

Count FormalParameter nodes in FormalParameterList. Compare against threshold.

### Configuration

Required `threshold` (default 5).

### Examples

**Bad (triggers rule):**
```dart
void process(int a, int b, int c, int d, int e, int f) {}
```

**Good (does NOT trigger):**
```dart
void process(ProcessConfig config) {}
class ProcessConfig {
  int a, b, c, d, e, f;
}
```

### Diagnostic Message

```
Function has too many parameters (6 > 5). Consider using a class or record.
```

### Acceptance Criteria (M4.8)

- Counts parameters accurately
- Respects threshold configuration
- Works with named and positional params
- All test cases pass

---

## 32. max_switch_cases

**Rule ID:** `max_switch_cases`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** warning  

### Description

Flags switch statements with too many cases. Many cases indicate complex logic that should be refactored.

### Phase 1 Heuristic

Count SwitchCase nodes in SwitchStatement. Compare against threshold.

### Configuration

Required `threshold` (default 10).

### Examples

**Bad (triggers rule):**
```dart
switch (x) {
  case 1: ...
  case 2: ...
  // ... 15 more cases
}
```

**Good (does NOT trigger):**
```dart
switch (x) {
  case 1: ...
  case 2: ...
  // ... 8 cases total
}
```

### Diagnostic Message

```
Switch statement has too many cases (15 > 10). Consider using a map or pattern matching.
```

### Acceptance Criteria (M4.8)

- Counts switch cases accurately
- Respects threshold configuration
- Works with all case types
- All test cases pass

---

## 33. no_duplicate_case_values

**Rule ID:** `no_duplicate_case_values`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** error  

### Description

Flags duplicate case labels in switch statements. Duplicate cases are unreachable and likely bugs.

### Phase 1 Heuristic

Collect all case values in SwitchStatement. Report duplicates.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
switch (x) {
  case 1: ...
  case 2: ...
  case 1: ...  // ✗ duplicate
}
```

**Good (does NOT trigger):**
```dart
switch (x) {
  case 1: ...
  case 2: ...
  default: ...
}
```

### Diagnostic Message

```
Duplicate case value 1. The second case is unreachable.
```

### Acceptance Criteria (M4.8)

- Detects duplicate case values
- Works with all case types
- Reports which value is duplicated
- All test cases pass

---

## 34. no_magic_number (SIMPLE variant)

**Rule IDs:** `no-magic-number` (dart_code_linter, COMPLEX), `no_magic_number` (pyramid_lint, SIMPLE variant)  
**Source:** dart_code_linter + pyramid_lint (shared implementation)  
**Complexity:** SIMPLE variant (1–2h) / COMPLEX variant (4–5h)  
**Default Severity:** warning  

### Description

Flags numeric literals except those in an allowlist. Magic numbers reduce code clarity; use named constants instead.

### Phase 1 Heuristic (SIMPLE variant)

Traverse all IntegerLiteral and DoubleLiteral nodes. Flag if not in allowlist (default: `[0, 1, 2, -1]`).

### Configuration

Required/Optional `allowlist` (default `[0, 1, 2, -1]`).

### Examples

**Bad (triggers rule):**
```dart
int x = 42;
double y = 3.14;
for (int i = 0; i < 100; i++) {}  // 100 is flagged
```

**Good (does NOT trigger):**
```dart
const int MAGIC_VALUE = 42;
int x = MAGIC_VALUE;
for (int i = 0; i < 100; i++) {}  // with config allowlist: [0, 1, 2, -1, 100]
```

### Diagnostic Message

```
Avoid magic numbers. Use a named constant instead. For example: const int THRESHOLD = 42.
```

### Acceptance Criteria (M4.8)

- Detects numeric literals not in allowlist
- Works with integers and doubles
- Respects configuration
- All test cases pass

---

## 35. prefer_declaring_const_constructor

**Rule ID:** `prefer_declaring_const_constructor`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Flags constructors that could be marked `const` because all fields are assigned const/immutable values.

### Phase 1 Heuristic

Detect ConstructorDeclaration. Check if all field assignments are to const expressions. If so, suggest `const` modifier.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
class Foo {
  final int x;
  final String y;
  Foo(this.x, this.y);  // can be const
}
```

**Good (does NOT trigger):**
```dart
class Foo {
  final int x;
  final String y;
  const Foo(this.x, this.y);
}
```

### Diagnostic Message

```
This constructor can be const. Mark it with 'const' for better optimization.
```

### Acceptance Criteria (M4.8)

- Detects constructors with only const field assignments
- Works with all constructor types
- All test cases pass

---

## 36. prefer_iterable_any

**Rule ID:** `prefer_iterable_any`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests using `.any()` instead of `.where().isNotEmpty`. More idiomatic and efficient.

### Phase 1 Heuristic

Detect MethodInvocation chain: `.where(predicate).isNotEmpty`. Suggest `.any(predicate)`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if (list.where((x) => x > 5).isNotEmpty) {}
var has = items.where((i) => i.valid).isNotEmpty;
```

**Good (does NOT trigger):**
```dart
if (list.any((x) => x > 5)) {}
var has = items.any((i) => i.valid);
```

### Diagnostic Message

```
Use .any() instead of .where().isNotEmpty for better readability.
```

### Acceptance Criteria (M4.8)

- Detects .where().isNotEmpty pattern
- Suggests .any() alternative
- All test cases pass

---

## 37. prefer_iterable_every

**Rule ID:** `prefer_iterable_every`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests using `.every()` instead of negated `.where().isEmpty`. More idiomatic and efficient.

### Phase 1 Heuristic

Detect UnaryExpression with `!` on MethodInvocation: `!.where(predicate).isEmpty`. Suggest `.every(predicate)`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if (!list.where((x) => x > 5).isEmpty) {}
var all = !items.where((i) => i.valid).isEmpty;
```

**Good (does NOT trigger):**
```dart
if (list.every((x) => x > 5)) {}
var all = items.every((i) => i.valid);
```

### Diagnostic Message

```
Use .every() instead of !.where().isEmpty for better readability.
```

### Acceptance Criteria (M4.8)

- Detects !.where().isEmpty pattern
- Suggests .every() alternative
- All test cases pass

---

## 38. prefer_underscore_for_unused_callback_parameters

**Rule ID:** `prefer_underscore_for_unused_callback_parameters`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests using `_` for unused callback parameters. Signals to readers that the parameter is intentionally unused.

### Phase 1 Heuristic

Detect FormalParameter in closure/callback context. Check if parameter is never referenced in function body. Suggest renaming to `_`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
list.forEach((item) { print("hi"); });  // item unused
map.entries.forEach((entry) { count++; });  // entry unused
```

**Good (does NOT trigger):**
```dart
list.forEach((_) { print("hi"); });
map.entries.forEach((_) { count++; });
```

### Diagnostic Message

```
Use underscore (_) for unused callback parameters.
```

### Acceptance Criteria (M4.8)

- Detects unused callback parameters
- Works in all callback contexts
- All test cases pass

---

## 39. use_spacer_as_expanded_child

**Rule ID:** `use_spacer_as_expanded_child`  
**Source:** pyramid_lint  
**Complexity:** SIMPLE (1–2h)  
**Default Severity:** suggestion  

### Description

Suggests using `Spacer()` widget instead of empty `Container()` or `SizedBox()` as child of `Expanded`. Spacer is more explicit about intent.

### Phase 1 Heuristic

Detect InstanceCreation of `Expanded` with empty `Container()` or `SizedBox()` as child. Suggest `Spacer()`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
Expanded(child: Container())
Expanded(child: SizedBox())
```

**Good (does NOT trigger):**
```dart
Expanded(child: Spacer())
Expanded(child: Text("hello"))
```

### Diagnostic Message

```
Use Spacer() instead of an empty Container() as an Expanded child. It's more explicit.
```

### Acceptance Criteria (M4.8)

- Detects empty Container/SizedBox in Expanded
- Suggests Spacer() alternative
- All test cases pass

---

# PART 2: MEDIUM RULES

## 40. avoid-global-state

**Rule ID:** `avoid-global-state`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Flags mutable top-level variables. Mutable global state is a code smell that hinders testing and introduces hidden dependencies. Use dependency injection or class members instead. Allows `const` or `@memoized` vars.

### Phase 1 Heuristic

Detect TopLevelVariableDeclaration that is NOT `const`. Check for `@memoized` annotation. Report if mutable.

### Configuration

Optional allowlist of variable names or patterns (default: `["logger", "config"]`).

### Examples

**Bad (triggers rule):**
```dart
int globalCounter = 0;  // mutable, no const
final List items = [];  // final but mutable collection
var cache = <String, int>{};
```

**Good (does NOT trigger):**
```dart
const int VERSION = 1;
const List<int> PRIMES = [2, 3, 5];
@memoized final int getMemoizedValue => computeOnce();
```

### Diagnostic Message

```
Avoid using mutable global state. Use dependency injection or class members instead.
```

### Acceptance Criteria (M4.8)

- Detects mutable top-level variables
- Allows `const` and `@memoized`
- Respects allowlist configuration
- All test cases pass

---

## 41. avoid-passing-async-when-sync-expected

**Rule ID:** `avoid-passing-async-when-sync-expected`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Flags passing async functions to parameters expecting sync functions. Type mismatch; the async function won't be awaited.

### Phase 1 Heuristic

Check function call/method invocation. For each argument, compare:
- Argument: Is it a reference to an async function? Check if function has `async` modifier.
- Parameter: What type annotation? Is it `Future<...>` or non-Future callback?
If argument is async and parameter expects sync, flag.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
Future<void> asyncFn() async {}

void callback(void Function() fn) {
  fn();  // expects sync, but gets async
}

callback(asyncFn);  // ✗ async passed to sync param
```

**Good (does NOT trigger):**
```dart
callback(() async { await asyncFn(); });  // ✓ correct signature
callback(() { asyncFn(); });  // ✓ correct (ignores return)
```

### Diagnostic Message

```
Avoid passing async functions to synchronous callbacks. The async function won't be awaited.
```

### Acceptance Criteria (M4.8)

- Detects async function passed to sync parameter
- Works with function references and lambdas
- Heuristic based on type annotations
- All test cases pass

---

## 42. avoid-redundant-async

**Rule ID:** `avoid-redundant-async`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** suggestion  

### Description

Flags `async` keyword on functions with a single `await` and no error handling. Can be simplified to `return await fn()` or just `return fn()`.

### Phase 1 Heuristic

Detect FunctionDeclaration/MethodDeclaration with `async` modifier. Count `await` expressions in body. If count == 1 and no try/catch, flag.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
Future<int> getValue() async {
  return await compute();
}

Future<void> foo() async {
  await bar();
}
```

**Good (does NOT trigger):**
```dart
Future<int> getValue() => compute();

Future<void> foo() async {
  await bar();
  await baz();  // multiple awaits
}

Future<void> foo() async {
  try { await bar(); }
  catch (e) { print(e); }  // has error handling
}
```

### Diagnostic Message

```
Remove redundant async. This function has only one await and no error handling.
```

### Acceptance Criteria (M4.8)

- Counts await statements correctly
- Allows multiple awaits or error handling
- All test cases pass

---

## 43. avoid-returning-widgets

**Rule ID:** `avoid-returning-widgets`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Flags returning Widget from non-build methods. Widgets should only be constructed in build methods. Other methods should return data.

### Phase 1 Heuristic

Detect ReturnStatement in MethodDeclaration. Check if:
1. Method name is NOT `build`, `buildX`, or `_buildX` (build method pattern).
2. Return type or expression is a Widget subclass (heuristic: check type annotation or InstanceCreation of known Widget classes).
Report if both conditions met.

### Configuration

Optional allowlist of method names (default: `["build"]`).

### Examples

**Bad (triggers rule):**
```dart
String getWidget() {
  return Container();  // returns Widget, not String
}

Widget helper() {
  return Text("hi");  // non-build method returning Widget
}
```

**Good (does NOT trigger):**
```dart
@override
Widget build(BuildContext context) {
  return Container();
}

Widget buildContent() {
  return Text("hi");  // build-method pattern OK
}

String getContent() {
  return "data";
}
```

### Diagnostic Message

```
Avoid returning widgets from non-build methods. Return data instead, and construct widgets in build methods.
```

### Acceptance Criteria (M4.8)

- Detects Widget returns in non-build methods
- Allows build-method patterns
- Respects configuration
- All test cases pass

---

## 44. avoid-unnecessary-type-assertions

**Rule ID:** `avoid-unnecessary-type-assertions`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Flags type assertions (`is T`) where the variable is already known to be type T via annotation. Redundant and misleading.

### Phase 1 Heuristic

Detect TypeTest (IsExpression) like `x is int`. Look up variable `x` in scope. If it has explicit type annotation `int`, flag.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
final int x = 5;
if (x is int) { }  // x already known to be int

String name = "hi";
if (name is String) { }  // redundant
```

**Good (does NOT trigger):**
```dart
dynamic x = 5;
if (x is int) { }  // necessary, x is dynamic

Object obj = 5;
if (obj is int) { }  // necessary, obj is Object
```

### Diagnostic Message

```
Unnecessary type assertion. Variable is already known to be of type int.
```

### Acceptance Criteria (M4.8)

- Detects unnecessary type checks
- Works with explicit type annotations
- All test cases pass

---

## 45. avoid-unnecessary-type-casts

**Rule ID:** `avoid-unnecessary-type-casts`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Flags type casts (`as T`) where the variable is already known to be type T. Redundant.

### Phase 1 Heuristic

Detect AsExpression like `x as int`. Look up variable `x` in scope. If it has explicit type annotation `int`, flag.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
final int x = 5;
int y = x as int;  // x already int

String name = "hi";
String upper = name as String;  // redundant
```

**Good (does NOT trigger):**
```dart
dynamic x = 5;
int y = x as int;  // necessary

Object obj = 5;
int y = obj as int;  // necessary
```

### Diagnostic Message

```
Unnecessary type cast. Variable is already known to be of type int.
```

### Acceptance Criteria (M4.8)

- Detects unnecessary casts
- Works with explicit type annotations
- All test cases pass

---

## 46. avoid-unrelated-type-assertions

**Rule ID:** `avoid-unrelated-type-assertions`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** error  

### Description

Flags type assertions (`is T`) that will never succeed. E.g., `if ("hello" is int)` is always false. Likely bugs.

### Phase 1 Heuristic

Detect TypeTest. Compare AST structure:
- If operand is literal `"hello"` (String) and test is `is int`, flag.
- If operand has explicit type annotation and test is unrelated type, flag.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if ("hello" is int) { }  // always false
final int x = 5;
if (x is String) { }  // x is int, not String
```

**Good (does NOT trigger):**
```dart
dynamic x = "hello";
if (x is int) { }  // x is dynamic, assertion is valid

Object obj = "hello";
if (obj is int) { }  // obj is Object, assertion is valid
```

### Diagnostic Message

```
This type assertion is always false. The variable can never be of type String.
```

### Acceptance Criteria (M4.8)

- Detects impossible type checks
- Based on AST structure and annotations
- All test cases pass

---

## 47. avoid-unused-parameters

**Rule IDs:** `avoid-unused-parameters` (dart_code_linter), `avoid_unused_parameters` (pyramid_lint)  
**Source:** dart_code_linter + pyramid_lint (shared implementation)  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Flags function parameters that are never referenced in the function body. Unused parameters are dead code or incomplete refactoring.

### Phase 1 Heuristic

For each FunctionDeclaration/MethodDeclaration, collect all FormalParameter names. Scan body for Identifier references. Report parameters not found in body.

### Configuration

Optional allowlist for callback params (default: allow `_`).

### Examples

**Bad (triggers rule):**
```dart
void foo(int unused) {
  print("hi");  // unused not referenced
}

int compute(int a, int b) {
  return a * 2;  // b never used
}
```

**Good (does NOT trigger):**
```dart
void foo(int value) {
  print(value);
}

int compute(int a, int b) {
  return a * b;
}

void onPressed(_) {
  doSomething();  // _ is OK for unused
}
```

### Diagnostic Message

```
Parameter 'unused' is never used. Remove it or use '_' to indicate it's intentionally unused.
```

### Acceptance Criteria (M4.8)

- Detects unused parameters
- Works with all parameter types
- All test cases pass

---

## 48. prefer-conditional-expressions

**Rule ID:** `prefer-conditional-expressions`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** suggestion  

### Description

Suggests replacing simple if/else blocks with ternary expressions. More concise for value returns.

### Phase 1 Heuristic

Detect IfStatement where both then-block and else-block contain a single ReturnStatement with expressions. Suggest ternary: `return condition ? thenVal : elseVal`.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
if (x) {
  return a;
} else {
  return b;
}

if (condition) return x; else return y;
```

**Good (does NOT trigger):**
```dart
return x ? a : b;

if (x) {
  doSomething();
  return a;
} else {
  doOtherThing();
  return b;
}
```

### Diagnostic Message

```
Use a conditional expression instead of an if/else block for simple value returns.
```

### Acceptance Criteria (M4.8)

- Detects simple if/else with returns
- Only suggests for value returns
- All test cases pass

---

## 49. prefer-extracting-callbacks

**Rule ID:** `prefer-extracting-callbacks`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** suggestion  

### Description

Suggests extracting large inline callbacks (lambdas/closures) to named functions. Improves readability and reusability.

### Phase 1 Heuristic

Detect FunctionExpression inside MethodInvocation (callback argument). Count lines in function body. If > threshold (e.g., 10 lines), suggest extracting.

### Configuration

Optional `lineThreshold` (default 10).

### Examples

**Bad (triggers rule):**
```dart
list.map((item) {
  var id = item.id;
  var name = item.name;
  var processed = name.toUpperCase();
  // ... 20+ lines of logic
  return processed;
}).toList();
```

**Good (does NOT trigger):**
```dart
list.map(_processItem).toList();

String _processItem(Item item) {
  var id = item.id;
  var name = item.name;
  var processed = name.toUpperCase();
  // ...
  return processed;
}
```

### Diagnostic Message

```
Extract this large callback to a named function for better readability.
```

### Acceptance Criteria (M4.8)

- Detects large inline callbacks
- Respects line threshold configuration
- All test cases pass

---

## 50. prefer-trailing-comma

**Rule ID:** `prefer-trailing-comma`  
**Source:** dart_code_linter  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** suggestion  

### Description

Requires trailing commas in multi-line argument and parameter lists. Helps with git diffs and auto-formatting.

### Phase 1 Heuristic

Detect ArgumentList, FormalParameterList, or InstanceCreation spanning multiple lines. Check if last argument/parameter has trailing comma. Flag if missing.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
foo(
  arg1,
  arg2
);  // ✗ missing trailing comma

foo(arg1, arg2);  // OK, single line
```

**Good (does NOT trigger):**
```dart
foo(
  arg1,
  arg2,  // ✓ trailing comma
);

foo(arg1, arg2);  // single line, OK
```

### Diagnostic Message

```
Add a trailing comma to multi-line argument lists for cleaner diffs.
```

### Acceptance Criteria (M4.8)

- Detects multi-line lists
- Checks for trailing comma
- Does NOT flag single-line lists
- All test cases pass

---

## 51. avoid_mutable_global_variables

**Rule ID:** `avoid_mutable_global_variables`  
**Source:** pyramid_lint  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Disallows mutable top-level variables. Only `const` variables allowed at top-level. See rule #40 for overlap.

### Phase 1 Heuristic

Detect TopLevelVariableDeclaration without `const` modifier. Flag.

### Configuration

Optional allowlist (default: `[]`).

### Examples

**Bad (triggers rule):**
```dart
int globalCounter = 0;
final List items = [];
var cache = <String, int>{};
```

**Good (does NOT trigger):**
```dart
const int VERSION = 1;
const List<int> PRIMES = [2, 3, 5];
```

### Diagnostic Message

```
Avoid mutable global variables. Use const for top-level declarations only.
```

### Acceptance Criteria (M4.8)

- Detects non-const top-level vars
- All test cases pass

---

## 52. prefer_dedicated_media_query_methods

**Rule ID:** `prefer_dedicated_media_query_methods`  
**Source:** pyramid_lint  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** suggestion  

### Description

Suggests using dedicated MediaQuery methods (`.width`, `.height`) instead of `.size.width`. More readable.

### Phase 1 Heuristic

Detect MethodInvocation chain: `MediaQuery.of(context).size.width` or `.size.height`. Suggest `MediaQuery.of(context).displayWidth` or equivalent.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
var w = MediaQuery.of(context).size.width;
var h = MediaQuery.of(context).size.height;
```

**Good (does NOT trigger):**
```dart
var w = MediaQuery.of(context).displayWidth;
var h = MediaQuery.of(context).displayHeight;
```

### Diagnostic Message

```
Use MediaQuery.of(context).displayWidth instead of .size.width for clarity.
```

### Acceptance Criteria (M4.8)

- Detects `.size.width` and `.size.height` patterns
- Suggests dedicated methods
- All test cases pass

---

## 53. unnecessary_flutter_imports

**Rule ID:** `unnecessary_flutter_imports`  
**Source:** pyramid_lint  
**Complexity:** MEDIUM (2–3h)  
**Default Severity:** warning  

### Description

Flags imports that are declared but never used in the file. Dead code.

### Phase 1 Heuristic

Collect all ImportDirective. For each, extract imported symbols. Scan file for references to those symbols. Report unused imports.

### Configuration

Optional allowlist of always-needed imports (default: `[]`).

### Examples

**Bad (triggers rule):**
```dart
import 'package:flutter/material.dart';  // never used

void main() {
  print("hello");
}
```

**Good (does NOT trigger):**
```dart
import 'package:flutter/material.dart';

void main() {
  runApp(MyApp());
}

class MyApp extends StatelessWidget { }
```

### Diagnostic Message

```
Unused import 'package:flutter/material.dart'. Remove it or use one of its symbols.
```

### Acceptance Criteria (M4.8)

- Detects unused imports
- Works with all import types
- Respects allowlist
- All test cases pass

---

# PART 3: COMPLEX RULES

## 54. member-ordering

**Rule ID:** `member-ordering`  
**Source:** dart_code_linter  
**Complexity:** COMPLEX (4–6h)  
**Default Severity:** warning  

### Description

Enforces a consistent order for class members: constants, static fields, instance fields, constructors, static methods, instance methods. Improves code readability.

### Phase 1 Heuristic

Collect all class members. Categorize by type (FieldDeclaration, ConstructorDeclaration, MethodDeclaration). Check for modifiers (static, const). Verify order matches config. Report violations.

### Configuration

Required `order` (default: `["const", "static_fields", "fields", "constructor", "static_methods", "methods"]`).

### Examples

**Bad (triggers rule):**
```dart
class Foo {
  void method() { }  // ✗ method before const
  static const int X = 1;
  int field;
  Foo();
}
```

**Good (does NOT trigger):**
```dart
class Foo {
  static const int X = 1;  // const first
  static int staticField;  // static fields
  int field;  // instance fields
  Foo();  // constructors
  static void staticMethod() { }  // static methods
  void method() { }  // instance methods
}
```

### Diagnostic Message

```
Member ordering incorrect. Should be: constants, static fields, instance fields, constructors, static methods, instance methods.
```

### Acceptance Criteria (M4.8)

- Categorizes all member types
- Enforces config order
- Reports each violation location
- All test cases pass

---

## 55. no-magic-number (COMPLEX variant)

**Rule IDs:** `no-magic-number` (dart_code_linter), `no_magic_number` (pyramid_lint)  
**Source:** dart_code_linter + pyramid_lint (shared, but COMPLEX variant for dart_code_linter)  
**Complexity:** COMPLEX (4–6h)  
**Default Severity:** warning  

### Description

Flags numeric literals except those in an allowlist. Magic numbers reduce code clarity. Phase 1 uses simple allowlist. Phase 2 will support const expression evaluation.

### Phase 1 Heuristic

See rule #34 for SIMPLE variant. COMPLEX variant adds support for named constant resolution (Phase 2 future).

### Configuration

Required/Optional `allowlist` (default `[0, 1, 2, -1]`).

### Examples

See rule #34 examples.

### Diagnostic Message

See rule #34 diagnostic.

### Acceptance Criteria (M4.8)

- Same as SIMPLE variant
- Ready for Phase 2 const evaluation extension
- All test cases pass

---

## 56. class_members_ordering

**Rule ID:** `class_members_ordering`  
**Source:** pyramid_lint  
**Complexity:** COMPLEX (4–6h)  
**Default Severity:** warning  

### Description

Enforces a consistent order for class members (same as `member-ordering`). Shared implementation with dart_code_linter variant.

### Phase 1 Heuristic

See rule #54.

### Configuration

Required `order` (default: `["const", "static_fields", "fields", "constructor", "static_methods", "methods"]`).

### Examples

See rule #54 examples.

### Diagnostic Message

See rule #54 diagnostic.

### Acceptance Criteria (M4.8)

- Same as member-ordering rule
- All test cases pass

---

## 57. use_once_constructors_once_provider

**Rule ID:** `use_once_constructors_once_provider`  
**Source:** pyramid_lint  
**Complexity:** COMPLEX (4–6h)  
**Default Severity:** warning  

### Description

Detects `OnceProvider` usage without the `.once()` wrapper. OnceProvider instances should be wrapped for proper lifecycle management.

### Phase 1 Heuristic

Detect InstanceCreation of `OnceProvider`. Check if it's wrapped in a call to `.once()`. If not, flag.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
final provider = OnceProvider(
  create: (ref) => MyService(),
);  // ✗ missing .once()
```

**Good (does NOT trigger):**
```dart
final provider = OnceProvider(
  create: (ref) => MyService(),
).once();  // ✓ wrapped in .once()
```

### Diagnostic Message

```
OnceProvider should be wrapped with .once(). Use OnceProvider(...).once() for proper lifecycle management.
```

### Acceptance Criteria (M4.8)

- Detects OnceProvider instances
- Checks for `.once()` wrapper
- All test cases pass

---

## 58. unnecessary_nullable_return_type

**Rule ID:** `unnecessary_nullable_return_type`  
**Source:** pyramid_lint  
**Complexity:** MEDIUM/COMPLEX (3–5h)  
**Default Severity:** warning  

### Description

Flags function return types as nullable (e.g., `Future<T?>`) when the function never returns null. Misleading to callers.

### Phase 1 Heuristic

Detect FunctionDeclaration with nullable return type (e.g., `Future<int?>`). Scan all ReturnStatement nodes in body. If none return null-literal or null-producing expressions, flag.

### Configuration

No configuration required.

### Examples

**Bad (triggers rule):**
```dart
Future<int?> getValue() {
  return Future.value(5);  // never null
}

String? getName() {
  return "John";  // never null
}
```

**Good (does NOT trigger):**
```dart
Future<int> getValue() {
  return Future.value(5);
}

String? getName() {
  return null;  // can be null
}
```

### Diagnostic Message

```
Function return type is unnecessarily nullable. Remove '?' from the return type.
```

### Acceptance Criteria (M4.8)

- Detects nullable return types
- Analyzes return statements
- All test cases pass

---

---

# DEDUPLICATION & OVERLAPS

## Exact Duplicates (Shared Implementation)

The following rules have identical semantics and share a single implementation, exposed via multiple rule IDs:

| Rule Name | dart_code_linter | pyramid_lint | Implementation |
|-----------|---|---|---|
| `no-empty-block` / `no_empty_block` | Rule #11 | Rule #23 | Single visitor + config |
| `avoid-unused-parameters` / `avoid_unused_parameters` | Rule #47 | Rule #60 | Single visitor + config |
| `no-magic-number` / `no_magic_number` | Rule #55 (COMPLEX) | Rule #34 (SIMPLE) | Single visitor, dual config profiles |

**Strategy:** Register once in rule registry, map both rule IDs to same implementation. Config schema handles differences.

---

## Semantic Overlaps (Parameterized Implementation)

| Rule | dart_code_linter | pyramid_lint | Difference | Handling |
|-----|---|---|---|---|
| Global state | Rule #40 (allow @memoized) | Rule #51 (const only) | Strictness differs | Single implementation, severity config |
| Member ordering | Rule #54 | Rule #56 | Identical semantics | Shared implementation |

---

## Configuration Schema (jdlint.json)

```json
{
  "rules": {
    "avoid-dynamic": { "enabled": true, "severity": "error" },
    "no-empty-block": { "enabled": true, "severity": "warning" },
    "no-magic-number": {
      "enabled": true,
      "severity": "warning",
      "allowlist": [0, 1, 2, -1]
    },
    "member-ordering": {
      "enabled": true,
      "severity": "warning",
      "order": ["const", "static_fields", "fields", "constructor", "static_methods", "methods"]
    },
    "max-lines-for-file": { "enabled": true, "threshold": 500 },
    "max-lines-for-function": { "enabled": true, "threshold": 100 },
    "max-parameters-for-function": { "enabled": true, "threshold": 5 },
    "max-switch-cases": { "enabled": true, "threshold": 10 },
    "prefer-correct-identifier-length": {
      "enabled": true,
      "allowlist": ["i", "j", "k", "x", "y", "z"]
    }
  }
}
```

---

## Implementation Priority & Batching

**Batch 1 (M4.2):** Rules #1–21, #22–39 — Pure AST pattern matching (40 rules, 8–12 engineers, 2 weeks)  
**Batch 2 (M4.3–M4.4):** Rules #40–53 — AST + context (14 rules, 6–8 engineers, 2 weeks)  
**Batch 3 (M4.5–M4.6):** Rules #54–58 — Complex (6 rules, 2–4 engineers, 2 weeks)

---

## Sign-Off Checklist (M4.8)

- [ ] All 60 rules implemented
- [ ] All test cases passing
- [ ] Zero duplicate diagnostics
- [ ] jdlint.json config working
- [ ] Full project lints in <1s
- [ ] LSP integration verified
- [ ] Rule docs generated
- [ ] No panics or unwrap() calls

---

**End of rule_specs.md**
