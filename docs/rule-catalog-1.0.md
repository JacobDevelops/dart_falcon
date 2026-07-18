# Falcon 1.0 Rule Catalog (planned)

This is the planned rule catalog for falcon 1.0. It enumerates every rule
falcon does **not** yet ship, drawn from three sources — remaining official
Dart/Flutter lints, DCM-inspired rules, and cross-file (whole-project) rules —
and assigns each a falcon group, an analysis type, and a 1.0 priority.

**Baseline:** falcon currently ships **148 rules** (73 tagged
`RuleSource::Lints`, plus dart_code_linter / pyramid_lint ports). This document
lists only the *gaps*.

**Priorities**

- **must-have** — blocks 1.0. The rule is either an official preset member or a
  high-signal correctness/idiom check expected of a serious Dart/Flutter linter.
- **nice-to-have** — 1.0 stretch. Shipped if the workstream reaches it, but the
  release does not block on it.
- **post-1.0** — explicitly out of 1.0 scope (experimental, hyper-niche, or
  needs config/schema design that is itself post-1.0 work).

**Falcon groups** are `complexity | correctness | performance | style |
suspicious`.

**Analysis types** (what a rule needs from the engine):

- **syntax-only** — AST/token pass, no types.
- **needs-local-type-inference** — falcon's minimal resolver; local flow/types.
- **needs-full-type-resolution** — element model / SDK type hierarchies.
- **needs-cross-file/project** — whole-program or pubspec/asset analysis.

## Planned catalog size

| Source | must-have | nice-to-have | post-1.0 | total |
|---|---:|---:|---:|---:|
| Official lints (gaps) | 25 | 120 | 0 | 145 |
| DCM-inspired | 64 | 63 | 27 | 154 |
| Cross-file | 4 | 3 | 4 | 11 |
| **Total (new for 1.0)** | **93** | **186** | **31** | **310** |

Shipped baseline (not counted above): **148**. Full projected 1.0 registry if
every must-have + nice-to-have lands: **148 + 93 + 186 = 427**.

Six DCM rows were deduped against the official-lints and cross-file lists (see
[Deduplication notes](#deduplication-notes)); the counts above are post-dedupe.

---

# 1. Remaining official lints (145)

Every live official `dart-lang/linter` rule falcon does not implement. Falcon's
official universe is 216 live rules (227 total minus 6 removed and 5
deprecated); falcon covers 71 of them today, leaving these 145. Preset
membership is authoritative from `package:lints` (`core.yaml`,
`recommended.yaml`) and `package:flutter_lints` (`flutter.yaml`); `recommended`
includes `core`, `flutter` includes `recommended`.

Preset rules are **must-have** for 1.0. Non-preset (opt-in / Effective-Dart)
rules are **nice-to-have**. For non-preset rows, `falcon_group`, `domain`, and
`analysis` are heuristic; the 25 preset rows are hand-classified.

## 1a. Must-have — official preset gaps (25)

| rule | falcon group | domain | analysis | preset | official group | description |
|---|---|---|---|---|---|---|
| `avoid_types_as_parameter_names` | suspicious | none | needs-full-type-resolution | core | errors | Avoid types as parameter names. |
| `collection_methods_unrelated_type` | correctness | none | needs-full-type-resolution | core | errors | Invocation of collection methods with arguments of unrelated types. |
| `unrelated_type_equality_checks` | correctness | none | needs-full-type-resolution | core | errors | `==` invocation with references of unrelated types. |
| `use_build_context_synchronously` | correctness | flutter | needs-full-type-resolution | flutter | errors | Do not use BuildContexts across async gaps. |
| `use_key_in_widget_constructors` | correctness | flutter | needs-full-type-resolution | flutter | errors | Use key in widget constructors. |
| `depend_on_referenced_packages` | correctness | none | needs-cross-file/project | core | pub | Depend on referenced packages. |
| `package_names` | style | none | needs-cross-file/project | recommended | pub | Use `lowercase_with_underscores` for package names. |
| `secure_pubspec_urls` | suspicious | none | needs-cross-file/project | core | pub | Use secure urls in `pubspec.yaml`. |
| `annotate_overrides` | style | none | needs-full-type-resolution | recommended | style | Annotate overridden members. |
| `avoid_renaming_method_parameters` | style | none | needs-full-type-resolution | recommended | style | Don't rename parameters of overridden methods. |
| `await_only_futures` | correctness | none | needs-full-type-resolution | core | style | Await only futures. |
| `exhaustive_cases` | correctness | none | needs-full-type-resolution | recommended | style | Define case clauses for all constants in enum-like classes. |
| `file_names` | style | none | syntax-only | core | style | Name source files using `lowercase_with_underscores`. |
| `implicit_call_tearoffs` | style | none | needs-full-type-resolution | core | style | Explicitly tear-off `call` methods when using an object as a Function. |
| `library_annotations` | style | none | needs-local-type-inference | core | style | Attach library annotations to library directives. |
| `library_private_types_in_public_api` | correctness | none | needs-full-type-resolution | recommended | style | Avoid using private types in public APIs. |
| `null_check_on_nullable_type_parameter` | correctness | none | needs-full-type-resolution | core | style | Don't use null check on a potentially nullable type parameter. |
| `null_closures` | correctness | none | needs-full-type-resolution | recommended | style | Do not pass `null` as an argument where a closure is expected. |
| `overridden_fields` | suspicious | none | needs-full-type-resolution | recommended | style | Don't override fields. |
| `prefer_const_constructors_in_immutables` | style | flutter | needs-full-type-resolution | flutter | style | Prefer declaring const constructors on `@immutable` classes. |
| `prefer_contains` | performance | none | needs-full-type-resolution | recommended | style | Use `contains` for `List` and `String` instances. |
| `prefer_interpolation_to_compose_strings` | style | none | needs-local-type-inference | recommended | style | Use interpolation to compose strings and values. |
| `type_literal_in_constant_pattern` | correctness | none | needs-local-type-inference | core | style | Don't use constant patterns with type literals. |
| `void_checks` | correctness | none | needs-full-type-resolution | core | style | Don't assign to void. |
| `invalid_runtime_check_with_js_interop_types` | correctness | none | needs-full-type-resolution | recommended | errors | Avoid is/as runtime checks on JS interop types (unsound). |

Preset breakdown: core 12, recommended 10, flutter 3. By analysis type:
needs-full-type-resolution 18, needs-cross-file/project 3, needs-local-type-inference 3, syntax-only 1.

**Resolver-expansion implication:** all but `file_names` (syntax-only) need type
resolution. Three need only local inference (`library_annotations`,
`prefer_interpolation_to_compose_strings`, `type_literal_in_constant_pattern`);
three are pubspec-driven and can ship independently of the type resolver
(`depend_on_referenced_packages`, `secure_pubspec_urls`, `package_names`); the
remaining ~18 need full type resolution and drive the resolver-expansion
workstream.

## 1b. Nice-to-have — non-preset official gaps (120)

| rule | falcon group | domain | analysis | official group | description |
|---|---|---|---|---|---|
| `always_declare_return_types` | style | none | needs-local-type-inference | style | Declare method return types. |
| `always_put_control_body_on_new_line` | style | none | syntax-only | style | Separate the control structure expression from its statement. |
| `always_put_required_named_parameters_first` | style | none | syntax-only | style | Put required named parameters first. |
| `always_specify_types` | style | none | needs-local-type-inference | style | Specify type annotations. |
| `always_use_package_imports` | correctness | none | needs-cross-file/project | errors | Avoid relative imports for files in `lib/`. |
| `avoid_annotating_with_dynamic` | style | none | needs-local-type-inference | style | Avoid annotating with dynamic when not required. |
| `avoid_bool_literals_in_conditional_expressions` | style | none | syntax-only | style | Avoid bool literals in conditional expressions. |
| `avoid_catches_without_on_clauses` | style | none | needs-full-type-resolution | style | Avoid catches without on clauses. |
| `avoid_catching_errors` | style | none | needs-full-type-resolution | style | Don't explicitly catch Error or types that implement it. |
| `avoid_classes_with_only_static_members` | complexity | none | needs-full-type-resolution | style | Avoid defining a class that contains only static members. |
| `avoid_double_and_int_checks` | style | none | needs-full-type-resolution | style | Avoid double and int checks. |
| `avoid_dynamic_calls` | correctness | none | needs-full-type-resolution | errors | Avoid method calls or property accesses on a `dynamic` target. |
| `avoid_equals_and_hash_code_on_mutable_classes` | style | none | needs-full-type-resolution | style | Avoid overloading `==` and hashCode on classes not marked `@immutable`. |
| `avoid_escaping_inner_quotes` | style | none | syntax-only | style | Avoid escaping inner quotes by converting surrounding quotes. |
| `avoid_field_initializers_in_const_classes` | style | none | needs-full-type-resolution | style | Avoid field initializers in const classes. |
| `avoid_final_parameters` | style | none | syntax-only | style | Avoid final for parameter declarations. |
| `avoid_implementing_value_types` | style | none | needs-full-type-resolution | style | Don't implement classes that override `==`. |
| `avoid_js_rounded_ints` | style | none | needs-full-type-resolution | style | Avoid JavaScript rounded ints. |
| `avoid_multiple_declarations_per_line` | style | none | syntax-only | style | Don't declare multiple variables on a single line. |
| `avoid_null_checks_in_equality_operators` | style | none | needs-full-type-resolution | style | Don't check for null in custom `==` operators. |
| `avoid_positional_boolean_parameters` | style | none | syntax-only | style | Avoid positional boolean parameters. |
| `avoid_private_typedef_functions` | style | none | syntax-only | style | Avoid private typedef functions. |
| `avoid_redundant_argument_values` | style | none | needs-full-type-resolution | style | Avoid redundant argument values. |
| `avoid_returning_this` | style | none | needs-full-type-resolution | style | Avoid returning this from methods just to enable a fluent interface. |
| `avoid_setters_without_getters` | style | none | syntax-only | style | Avoid setters without getters. |
| `avoid_slow_async_io` | performance | none | needs-full-type-resolution | errors | Avoid slow async `dart:io` methods. |
| `avoid_type_to_string` | correctness | none | needs-full-type-resolution | errors | Avoid `<Type>.toString()` in production code since results may be minified. |
| `avoid_types_on_closure_parameters` | style | none | needs-local-type-inference | style | Avoid annotating types for function expression parameters. |
| `avoid_unused_constructor_parameters` | style | none | needs-full-type-resolution | style | Avoid defining unused parameters in constructors. |
| `avoid_void_async` | style | none | needs-full-type-resolution | style | Avoid async functions that return void. |
| `cancel_subscriptions` | correctness | none | needs-full-type-resolution | errors | Cancel instances of `dart.async.StreamSubscription`. |
| `cascade_invocations` | complexity | none | needs-full-type-resolution | style | Cascade consecutive method invocations on the same reference. |
| `cast_nullable_to_non_nullable` | style | none | needs-local-type-inference | style | Don't cast a nullable value to a non nullable type. |
| `close_sinks` | correctness | none | needs-full-type-resolution | errors | Close instances of `dart.core.Sink`. |
| `combinators_ordering` | style | none | syntax-only | style | Sort combinator names alphabetically. |
| `comment_references` | correctness | none | needs-full-type-resolution | errors | Only reference in scope identifiers in doc comments. |
| `conditional_uri_does_not_exist` | style | none | needs-cross-file/project | style | Missing conditional import. |
| `deprecated_consistency` | style | none | needs-full-type-resolution | style | Missing deprecated annotation. |
| `deprecated_member_use_from_same_package` | correctness | none | needs-cross-file/project | errors | Avoid using deprecated elements from within the declaring package. |
| `diagnostic_describe_all_properties` | correctness | flutter | needs-full-type-resolution | errors | DO reference all public properties in debug methods. |
| `directives_ordering` | style | none | syntax-only | style | Adhere to Effective Dart directives sorting conventions. |
| `discarded_futures` | correctness | none | needs-full-type-resolution | errors | Don't invoke asynchronous functions in non-async blocks. |
| `do_not_use_environment` | style | none | needs-cross-file/project | style | Do not use environment declared variables. |
| `eol_at_end_of_file` | style | none | syntax-only | style | Put a single newline at end of file. |
| `flutter_style_todos` | style | none | syntax-only | style | Use Flutter TODO format: `// TODO(username): message, https://URL`. |
| `implicit_reopen` | correctness | none | needs-full-type-resolution | errors | Don't implicitly reopen classes. _(experimental — defer)_ |
| `invalid_case_patterns` | correctness | none | needs-full-type-resolution | errors | Use case expressions that are valid in Dart 3.0. _(experimental — defer)_ |
| `join_return_with_assignment` | complexity | none | needs-full-type-resolution | style | Join return statement with assignment when possible. |
| `leading_newlines_in_multiline_strings` | style | none | syntax-only | style | Start multiline strings with a newline. |
| `library_names` | style | none | syntax-only | style | Name libraries using `lowercase_with_underscores`. |
| `lines_longer_than_80_chars` | style | none | syntax-only | style | Avoid lines longer than 80 characters. |
| `literal_only_boolean_expressions` | correctness | none | syntax-only | errors | Boolean expression composed only with literals. |
| `matching_super_parameters` | style | none | needs-full-type-resolution | style | Use matching super parameter names. |
| `missing_whitespace_between_adjacent_strings` | style | none | syntax-only | style | Missing whitespace between adjacent strings. |
| `no_adjacent_strings_in_list` | correctness | none | syntax-only | errors | Don't use adjacent strings in list. |
| `no_default_cases` | style | none | syntax-only | style | No default cases. _(experimental — defer)_ |
| `no_literal_bool_comparisons` | style | none | syntax-only | style | Don't compare booleans to boolean literals. |
| `no_runtimeType_toString` | style | none | needs-full-type-resolution | style | Avoid calling `toString()` on runtimeType. |
| `no_self_assignments` | correctness | none | needs-full-type-resolution | errors | Don't assign a variable to itself. |
| `noop_primitive_operations` | performance | none | needs-full-type-resolution | style | Noop primitive operations. |
| `omit_local_variable_types` | style | none | needs-local-type-inference | style | Omit type annotations for local variables. |
| `one_member_abstracts` | complexity | none | syntax-only | style | Avoid a one-member abstract class when a simple function will do. |
| `only_throw_errors` | style | none | needs-full-type-resolution | style | Only throw instances of classes extending Exception or Error. |
| `package_api_docs` | style | none | needs-cross-file/project | style | Provide doc comments for all public APIs. |
| `package_prefixed_library_names` | style | none | syntax-only | style | Prefix library names with the package name and a dot-separated path. |
| `parameter_assignments` | style | none | needs-full-type-resolution | style | Don't reassign references to parameters of functions or methods. |
| `prefer_asserts_in_initializer_lists` | style | none | syntax-only | style | Prefer putting asserts in initializer lists. |
| `prefer_asserts_with_message` | style | none | syntax-only | style | Prefer asserts with message. |
| `prefer_const_constructors` | style | none | needs-full-type-resolution | style | Prefer const with constant constructors. |
| `prefer_const_declarations` | style | none | needs-local-type-inference | style | Prefer const over final for declarations. |
| `prefer_const_literals_to_create_immutables` | style | none | needs-full-type-resolution | style | Prefer const literals as parameters of constructors on `@immutable` classes. |
| `prefer_constructors_over_static_methods` | style | none | needs-full-type-resolution | style | Prefer constructors instead of static methods to create instances. |
| `prefer_double_quotes` | style | none | syntax-only | style | Prefer double quotes where they won't require escape sequences. |
| `prefer_expression_function_bodies` | complexity | none | syntax-only | style | Use `=>` for short members whose body is a single return statement. |
| `prefer_final_in_for_each` | style | none | needs-local-type-inference | style | Prefer final in for-each loop variable if not reassigned. |
| `prefer_final_locals` | style | none | needs-local-type-inference | style | Prefer final for variable declarations if not reassigned. |
| `prefer_final_parameters` | style | none | needs-local-type-inference | style | Prefer final for parameter declarations if not reassigned. |
| `prefer_foreach` | performance | none | needs-full-type-resolution | style | Use `forEach` to only apply a function to all elements. |
| `prefer_if_elements_to_conditional_expressions` | complexity | none | needs-full-type-resolution | style | Prefer if elements to conditional expressions where possible. |
| `prefer_int_literals` | style | none | syntax-only | style | Prefer int literals over double literals. |
| `prefer_mixin` | style | none | needs-full-type-resolution | style | Prefer using mixins. |
| `prefer_null_aware_method_calls` | style | none | needs-full-type-resolution | style | Prefer null aware method calls. |
| `prefer_relative_imports` | correctness | none | needs-cross-file/project | errors | Prefer relative imports for files in `lib/`. |
| `prefer_single_quotes` | style | none | syntax-only | style | Only use double quotes for strings containing single quotes. |
| `prefer_void_to_null` | correctness | none | needs-full-type-resolution | errors | Don't use the Null type, unless you are positive you don't want void. |
| `public_member_api_docs` | style | none | needs-cross-file/project | style | Document all public members. |
| `require_trailing_commas` | style | none | syntax-only | style | Use trailing commas for all function calls and declarations. |
| `sized_box_shrink_expand` | style | flutter | needs-full-type-resolution | style | Use SizedBox shrink and expand named constructors. |
| `sort_constructors_first` | style | none | syntax-only | style | Sort constructor declarations before other members. |
| `sort_pub_dependencies` | correctness | none | syntax-only | pub | Sort pub dependencies alphabetically. |
| `sort_unnamed_constructors_first` | style | none | syntax-only | style | Sort unnamed constructor declarations first. |
| `test_types_in_equals` | correctness | none | needs-full-type-resolution | errors | Test type arguments in `operator ==(Object other)`. |
| `throw_in_finally` | correctness | none | needs-full-type-resolution | errors | Avoid `throw` in finally block. |
| `tighten_type_of_initializing_formals` | style | none | needs-full-type-resolution | style | Tighten type of initializing formal. |
| `type_annotate_public_apis` | style | none | needs-local-type-inference | style | Type annotate public APIs. |
| `unawaited_futures` | style | none | needs-full-type-resolution | style | `Future` results in `async` bodies must be awaited or marked `unawaited`. |
| `unnecessary_await_in_return` | style | none | needs-full-type-resolution | style | Unnecessary await keyword in return. |
| `unnecessary_breaks` | style | none | syntax-only | style | Don't use explicit `break`s when a break is implied. |
| `unnecessary_final` | style | none | needs-local-type-inference | style | Don't use `final` for local variables. |
| `unnecessary_lambdas` | complexity | none | needs-full-type-resolution | style | Don't create a lambda when a tear-off will do. |
| `unnecessary_library_directive` | style | none | syntax-only | style | Avoid library directives unless documented or annotated. |
| `unnecessary_null_aware_operator_on_extension_on_nullable` | style | none | needs-full-type-resolution | style | Unnecessary null aware operator on extension on a nullable type. |
| `unnecessary_null_checks` | style | none | needs-full-type-resolution | style | Unnecessary null checks. _(experimental — defer)_ |
| `unnecessary_parenthesis` | style | none | needs-full-type-resolution | style | Unnecessary parentheses can be removed. |
| `unnecessary_raw_strings` | style | none | syntax-only | style | Unnecessary raw string. |
| `unnecessary_statements` | correctness | none | needs-full-type-resolution | errors | Avoid using unnecessary statements. |
| `unreachable_from_main` | style | none | needs-cross-file/project | style | Unreachable top-level members in executable libraries. |
| `unsafe_html` | correctness | none | needs-full-type-resolution | errors | Avoid unsafe HTML APIs. |
| `use_colored_box` | style | flutter | needs-full-type-resolution | style | Use `ColoredBox`. |
| `use_decorated_box` | style | flutter | needs-full-type-resolution | style | Use `DecoratedBox`. |
| `use_enums` | style | none | needs-full-type-resolution | style | Use enums rather than classes that behave like enums. |
| `use_if_null_to_convert_nulls_to_bools` | style | none | needs-full-type-resolution | style | Use if-null operators to convert nulls to bools. |
| `use_is_even_rather_than_modulo` | performance | none | needs-full-type-resolution | style | Prefer `isOdd`/`isEven` over checking the result of `% 2`. |
| `use_late_for_private_fields_and_variables` | style | none | needs-full-type-resolution | style | Use late for private members with a non-nullable type. _(experimental — defer)_ |
| `use_named_constants` | style | none | needs-full-type-resolution | style | Use predefined named constants. |
| `use_raw_strings` | style | none | syntax-only | style | Use raw string to avoid escapes. |
| `use_setters_to_change_properties` | style | none | needs-full-type-resolution | style | Use a setter for operations that conceptually change a property. |
| `use_string_buffers` | performance | none | needs-full-type-resolution | style | Use string buffers to compose strings. |
| `use_test_throws_matchers` | style | none | needs-full-type-resolution | style | Use `throwsA` matcher instead of `fail()`. |
| `use_to_and_as_if_applicable` | style | none | needs-full-type-resolution | style | Start method names with to/_to or as/_as if applicable. |

Non-preset analysis-type breakdown (heuristic): needs-full-type-resolution 63,
syntax-only 37, needs-local-type-inference 12, needs-cross-file/project 8.
Experimental rows above (`implicit_reopen`, `invalid_case_patterns`,
`no_default_cases`, `unnecessary_null_checks`,
`use_late_for_private_fields_and_variables`) are deferred within the
nice-to-have bucket until upstream stabilizes them.

---

# 2. DCM-inspired rules (154)

Rules worth reimplementing as **open** falcon rules, surveyed from the DCM
(dcm.dev) catalog. DCM went commercial (~2023): its free CLI tier covers metrics
plus a small legacy `dart_code_metrics` (MIT, pre-1.0) subset — most of which
falcon already ports. Everything below is treated as **DCM-paid** unless marked
`legacy-free`; reimplementing paid rules as open falcon rules is the point.
`paid?` = DCM's per-rule Pro badge was not individually verified.

Analysis legend: **S** syntax-only, **LT** needs-local-type-inference, **FT**
needs-full-type-resolution, **X** needs-cross-file/project. Domain-rule falcon
groups (flutter/test/bloc/riverpod/provider/flutter_hooks/equatable) are
assigned here for the catalog and should be confirmed at implementation time.

## 2a. core — correctness

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-duplicate-map-keys` | correctness | S | paid? | Duplicate keys in a map literal silently drop entries. | must-have |
| `avoid-duplicate-collection-elements` | correctness | LT | paid | Duplicate elements in set/list literal. | must-have |
| `avoid-duplicate-switch-case-conditions` | correctness | S | paid | Two switch cases with equal constant conditions. | must-have |
| `avoid-collection-equality-checks` | correctness | FT | paid | `==` between collections never compares contents. | must-have |
| `avoid-unrelated-type-casts` | correctness | FT | paid | Cast between unrelated types always fails. | must-have |
| `avoid-unsafe-collection-methods` | correctness | FT | paid | `.first`/`.single`/`[i]` that can throw at runtime. | must-have |
| `avoid-missing-interpolation` | correctness | S | paid | `$name` written as literal text outside interpolation. | must-have |
| `avoid-contradictory-expressions` | correctness | LT | paid | `a && !a` style always-false/true conditions. | must-have |
| `avoid-unmodified-loop-condition` | correctness | LT | paid | Loop variable never changes → infinite loop. | must-have |
| `avoid-uncaught-future-errors` | correctness | FT | paid | Future error path with no catch. | must-have |
| `avoid-weak-cryptographic-algorithms` | correctness | FT | paid | MD5/SHA1/weak cipher usage. | must-have |
| `avoid-duplicate-cascades` | correctness | S | paid? | Same cascade section invoked twice. | nice-to-have |
| `avoid-duplicate-exports` | correctness | S | paid | Duplicate export directive. | nice-to-have |
| `avoid-duplicate-named-imports` | correctness | S | paid | Duplicate named import. | nice-to-have |
| `avoid-duplicate-mixins` | correctness | S | paid | Same mixin applied twice. | nice-to-have |
| `avoid-duplicate-factories` | correctness | S | paid | Duplicate factory declaration. | nice-to-have |
| `avoid-duplicate-initializers` | correctness | S | paid | Duplicate initializer entry. | nice-to-have |
| `avoid-unsafe-reduce` | correctness | FT | paid | `reduce` on possibly-empty iterable throws. | nice-to-have |
| `function-always-returns-null` | correctness | LT | paid | Function whose every return yields null. | nice-to-have |
| `function-always-returns-same-value` | correctness | LT | paid | Function returns one constant on all paths. | nice-to-have |
| `avoid-missing-enum-constant-in-map` | correctness | FT | paid | Map keyed by enum missing a constant. | nice-to-have |
| `avoid-constant-conditions` | correctness | LT | paid | Condition statically known constant. | nice-to-have |
| `avoid-constant-assert-conditions` | correctness | LT | paid | Assert condition statically known constant. | nice-to-have |
| `avoid-constant-switches` | correctness | LT | paid | Switch on a statically constant value. | nice-to-have |
| `avoid-self-compare` | correctness | S | paid | `x == x` (verify vs existing no-self-comparisons). | nice-to-have |
| `avoid-unused-after-null-check` | correctness | LT | paid | Value discarded after a null check. | nice-to-have |
| `avoid-unreachable-for-loop` | correctness | LT | paid | Loop body provably never runs. | nice-to-have |
| `avoid-unconditional-break` | correctness | S | paid | `break` that always fires on first iteration. | nice-to-have |
| `avoid-nested-futures` | correctness | FT | paid | `Future<Future<T>>` nesting mistake. | nice-to-have |
| `avoid-nested-streams-and-futures` | correctness | FT | paid | Nested stream/future type mistake. | nice-to-have |
| `avoid-missing-completer-stack-trace` | correctness | FT | paid | `Completer.completeError` without stack trace. | nice-to-have |
| `prefer-random-secure` | correctness | FT | paid | `Random()` where `Random.secure()` intended. | nice-to-have |
| `avoid-sensitive-query-params` | suspicious | S | paid | Secrets/tokens in URL query params. | nice-to-have |
| `avoid-assigning-to-static-field` | correctness | LT | paid | Mutating static state. | nice-to-have |
| `match-getter-setter-field-names` | correctness | LT | paid | Getter/setter names disagree with backing field. | nice-to-have |
| `prefer-correct-json-casts` | correctness | FT | paid | Unsafe casts decoding JSON maps. | nice-to-have |
| `avoid-recursive-calls` | correctness | LT | paid | Unbounded self-recursion (falcon has recursive-getters only). | nice-to-have |
| `avoid-not-encodable-in-to-json` | correctness | FT | paid | `toJson` returns a non-encodable value. | post-1.0 |
| `require-atomic-async-updates` | correctness | FT | paid | Read-modify-write across an await race. | post-1.0 |
| `pass-correct-accepted-type` | correctness | FT | paid | Argument type mismatch (DCM-style). | post-1.0 |
| `handle-throwing-invocations` | correctness | FT | paid | Call to throwing API without try/catch. | post-1.0 |

## 2b. core — suspicious

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-commented-out-code` | suspicious | S | paid | Commented-out code blocks left behind. | must-have |
| `avoid-shadowing` | suspicious | LT | paid | Local shadows an outer member. | must-have |
| `avoid-shadowed-extension-methods` | suspicious | LT | paid | Local shadows an extension member. | must-have |
| `avoid-assignments-as-conditions` | suspicious | S | paid | `if (x = y)` likely-bug. | must-have |
| `avoid-bitwise-operators-with-booleans` | suspicious | S | paid | `&`/`\|` on bools instead of `&&`/`\|\|`. | must-have |
| `avoid-banned-names` | suspicious | S | paid | Config-driven forbidden identifiers. | nice-to-have |
| `avoid-banned-types` | suspicious | FT | paid | Config-driven forbidden types. | nice-to-have |
| `avoid-banned-annotations` | suspicious | S | paid | Config-driven forbidden annotations. | nice-to-have |
| `avoid-banned-exports` | suspicious | S | paid | Config-driven forbidden exports. | nice-to-have |
| `banned-usage` | suspicious | FT | paid | Config-driven banned API usage. | nice-to-have |
| `avoid-barrel-files` | suspicious | X | paid | Discourage barrel/index re-export files. | nice-to-have |
| `avoid-double-slash-imports` | suspicious | S | paid | `//` in import path. | nice-to-have |
| `avoid-incorrect-uri` | suspicious | S | paid | Malformed URI in import/annotation. | nice-to-have |
| `avoid-accessing-other-classes-private-members` | suspicious | FT | paid | Reaching into another class's privates. | nice-to-have |
| `avoid-referencing-subclasses` | suspicious | FT | paid | Base class referencing its subclasses. | nice-to-have |
| `avoid-suspicious-super-overrides` | suspicious | FT | paid | Override that discards super behavior. | nice-to-have |
| `no-magic-string` | suspicious | S | paid | String-literal analog of no-magic-number. | nice-to-have |
| `avoid-non-final-exception-class-fields` | suspicious | LT | paid | Mutable fields on exception classes. | nice-to-have |
| `avoid-mutating-parameters` | suspicious | LT | paid | Reassigning/mutating a parameter. | nice-to-have |
| `avoid-nested-assignments` | suspicious | S | paid | Assignment buried in an expression. | nice-to-have |
| `avoid-multi-assignment` | suspicious | S | paid | Chained multi-assignment. | nice-to-have |
| `avoid-negated-conditions` | suspicious | S | paid | Prefer positive conditions for readability. | nice-to-have |
| `avoid-inverted-boolean-checks` | suspicious | S | paid | Prefer non-inverted boolean checks. | nice-to-have |
| `avoid-importing-entrypoint-exports` | suspicious | X | paid | Importing from an app entrypoint. | post-1.0 |
| `avoid-similar-names` | suspicious | S | paid | Confusingly similar identifiers in scope. | post-1.0 |

## 2c. core — style

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `prefer-early-return` | style | S | paid | Flatten nested if via early return. | must-have |
| `prefer-returning-condition` | style | S | paid | `if (x) return true; return false;` → `return x;`. | must-have |
| `prefer-switch-expression` | style | S | paid | Convert switch statement to Dart 3 switch expression. | must-have |
| `prefer-for-in` | style | S | paid | Index loop convertible to for-in. | must-have |
| `prefer-named-boolean-parameters` | style | S | paid | Bare bool positional params hurt call sites. | must-have |
| `prefer-any-or-every` | style | FT | paid | Manual loop replaceable by any/every. | nice-to-have |
| `prefer-getter-over-method` | style | LT | paid | Zero-arg method that should be a getter. | nice-to-have |
| `prefer-named-parameters` | style | S | paid | Too many positional params → named. | nice-to-have |
| `prefer-boolean-prefixes` | style | S | paid | Bool getters/fields lacking is/has/should prefix (verify vs boolean-prefixes). | nice-to-have |
| `prefer-match-file-name` | style | X | paid | Top-level type name should match file name. | nice-to-have |
| `prefer-type-over-var` | style | LT | paid | Explicit type instead of `var` where unclear. | nice-to-have |
| `prefer-correct-error-name` | style | S | paid | Naming convention for error types. | nice-to-have |
| `prefer-correct-handler-name` | style | S | paid | Naming convention for handlers. | nice-to-have |
| `prefer-correct-callback-field-name` | style | S | paid | Naming convention for callback fields. | nice-to-have |
| `prefer-commenting-analyzer-ignores` | style | S | paid | `// ignore:` must carry an explanation. | nice-to-have |
| `newline-before-method` | style | S | paid | Blank line before method declarations. | nice-to-have |
| `newline-before-case` | style | S | paid | Blank line before case clauses. | nice-to-have |
| `newline-before-throw` | style | S | paid | Blank line before throw. | nice-to-have |
| `newline-before-continue` | style | S | paid | Blank line before continue. | nice-to-have |
| `newline-before-break` | style | S | paid | Blank line before break. | nice-to-have |
| `newline-before-constructor` | style | S | paid | Blank line before constructor. | nice-to-have |
| `enum-constants-ordering` | style | S | paid | Config-driven enum constant ordering. | nice-to-have |
| `map-keys-ordering` | style | S | paid | Config-driven map key ordering. | nice-to-have |
| `parameters-ordering` | style | S | paid | Config-driven parameter ordering. | nice-to-have |
| `arguments-ordering` | style | S | paid | Config-driven argument ordering. | nice-to-have |
| `initializers-ordering` | style | S | paid | Config-driven initializer ordering. | nice-to-have |
| `pattern-fields-ordering` | style | S | paid | Config-driven pattern-field ordering. | nice-to-have |
| `record-fields-ordering` | style | S | paid | Config-driven record-field ordering. | nice-to-have |
| `prefer-single-declaration-per-file` | style | S | paid | One public type per file. | post-1.0 |
| `prefer-single-widget-per-file` | style | S | paid | One widget per file. | post-1.0 |
| `prefer-prefixed-global-constants` | style | S | paid | Global const naming convention. | post-1.0 |
| `format-test-name` | style | S | paid | Test description formatting. | post-1.0 |
| `match-class-name-pattern` | style | S | paid | Class names must match a configured regex. | post-1.0 |

## 2d. core — complexity

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-complex-conditions` | complexity | S | paid | Boolean-expression complexity limit. | must-have |
| `avoid-complex-loop-conditions` | complexity | S | paid | Loop-condition complexity limit. | must-have |
| `avoid-collapsible-if` | complexity | S | paid | Nested if mergeable into one condition. | must-have |
| `avoid-long-records` | complexity | S | paid | Record size limit (falcon has param-list). | nice-to-have |
| `avoid-complex-arithmetic-expressions` | complexity | S | paid | Arithmetic expression complexity. | nice-to-have |
| `avoid-if-with-many-branches` | complexity | S | paid | Long if/else-if chains → switch. | nice-to-have |
| `avoid-nested-switches` | complexity | S | paid | Deeply nested switch statements. | nice-to-have |
| `avoid-nested-switch-expressions` | complexity | S | paid | Deeply nested switch expressions. | nice-to-have |
| `avoid-nested-try-statements` | complexity | S | paid | Deeply nested try statements. | nice-to-have |
| `max-statements` | complexity | S | paid | Statement-count-per-function metric. | nice-to-have |
| `avoid-local-functions` | complexity | S | paid | Overuse of local functions. | post-1.0 |
| `avoid-excessive-expressions` | complexity | S | paid | Expression-length metric. | post-1.0 |

## 2e. core — performance

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-slow-collection-methods` | performance | FT | paid | Slow patterns (`.length == 0`, repeated `.where().first`). | must-have |
| `move-variable-outside-iteration` | performance | LT | paid | Loop-invariant allocation inside a loop. | must-have |
| `avoid-unnecessary-collections` | performance | LT | paid | Collection built only to be iterated once. | nice-to-have |
| `move-variable-closer-to-its-usage` | performance | LT | paid | Hoisted variable used in one branch. | nice-to-have |
| `avoid-unnecessary-futures` | performance | FT | paid | `async`/Future wrapper that adds no value. | nice-to-have |
| `prefer-return-await` | performance | FT | paid | `return await` needed for correct stack traces. | nice-to-have |
| `prefer-bytes-builder` | performance | FT | paid | Building byte lists without BytesBuilder. | post-1.0 |

## 2f. core — unnecessary/redundant cleanups (style)

Not covered by falcon's existing `unnecessary-*` family; verify each against the
shipped set before implementing — several are near-twins that should be aliased,
not re-added.

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-unnecessary-if` | style | LT | paid | Redundant if that can be simplified. | must-have |
| `avoid-unnecessary-conditionals` | style | LT | paid | Redundant conditional expression. | must-have |
| `avoid-unnecessary-return` | style | S | paid | Redundant trailing return. | must-have |
| `avoid-redundant-else` | style | S | paid | `else` after a returning if. | must-have |
| `avoid-unnecessary-block` | style | S | paid | Redundant block statement. | nice-to-have |
| `avoid-unnecessary-call` | style | FT | paid | Redundant `.call()`. | nice-to-have |
| `avoid-unnecessary-continue` | style | S | paid | Redundant trailing continue. | nice-to-have |
| `avoid-unnecessary-negations` | style | S | paid | Double/redundant negation. | nice-to-have |
| `avoid-unnecessary-length-check` | style | LT | paid | Redundant `.length` check before access. | nice-to-have |
| `avoid-unnecessary-local-variable` | style | LT | paid | Local variable used once immediately. | nice-to-have |
| `avoid-unnecessary-super` | style | LT | paid | Redundant super call. | nice-to-have |
| `avoid-unnecessary-overrides-in-state` | style | LT | paid | State override that only calls super. | nice-to-have |
| `avoid-unnecessary-getter` | style | LT | paid | Getter that only returns a field. | nice-to-have |
| `avoid-unnecessary-reassignment` | style | LT | paid | Variable reassigned to the same value. | nice-to-have |
| `use-existing-variable` | style | LT | paid | Reuse an existing binding instead of recomputing. | nice-to-have |
| `use-existing-destructuring` | style | LT | paid | Reuse an existing destructuring binding. | nice-to-have |

## 2g. flutter domain

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `use-setstate-synchronously` | correctness | FT | paid | `setState` after await without mounted check. | must-have |
| `avoid-empty-setstate` | correctness | S | paid | `setState(() {})` with an empty body. | must-have |
| `avoid-unnecessary-setstate` | correctness | FT | paid | setState in build/init where needless. | must-have |
| `use-closest-build-context` | correctness | FT | paid | Wrong BuildContext across an async gap. | must-have |
| `avoid-late-context` | correctness | FT | paid | Storing BuildContext in a late field. | must-have |
| `avoid-inherited-widget-in-initstate` | correctness | FT | paid | `.of(context)` in initState. | must-have |
| `avoid-undisposed-instances` | correctness | FT | paid | Controllers/notifiers never disposed. | must-have |
| `dispose-fields` | correctness | FT | paid | Disposable fields not disposed in `dispose()`. | must-have |
| `always-remove-listener` | correctness | FT | paid | `addListener` without `removeListener`. | must-have |
| `avoid-recursive-widget-calls` | correctness | LT | paid | Widget builds itself → stack overflow. | must-have |
| `avoid-shrink-wrap-in-lists` | performance | LT | paid | `shrinkWrap: true` perf trap in scrollables. | must-have |
| `prefer-align-over-container` | style | LT | paid | Container used only for alignment → Align. | must-have |
| `prefer-center-over-align` | style | LT | paid | Align used for centering → Center. | must-have |
| `prefer-padding-over-container` | style | LT | paid | Container used only for padding → Padding. | must-have |
| `prefer-constrained-box-over-container` | style | LT | paid | Container used only for constraints → ConstrainedBox. | must-have |
| `prefer-transform-over-container` | style | LT | paid | Container used only for transform → Transform. | must-have |
| `prefer-container` | style | LT | paid | Collapse stacked layout widgets into one Container. | must-have |
| `prefer-for-loop-in-children` | style | S | paid | `.map().toList()` in children → collection-for. | must-have |
| `prefer-action-button-tooltip` | style | LT | paid | IconButton/action lacking a tooltip (a11y). | must-have |
| `avoid-unnecessary-stateful-widgets` | correctness | FT | paid | StatefulWidget with no state → Stateless. | must-have |
| `avoid-mounted-in-setstate` | correctness | FT | paid | `mounted` check misuse around setState. | nice-to-have |
| `prefer-single-setstate` | performance | FT | paid | Multiple setState calls collapsible. | nice-to-have |
| `avoid-disposing-late-fields` | correctness | LT | paid | Disposing a late field that may be unset. | nice-to-have |
| `avoid-missing-controller` | correctness | FT | paid | Widget expecting a controller left unset. | nice-to-have |
| `always-pass-global-key` | correctness | LT | paid | GlobalKey should be passed, not created inline. | nice-to-have |
| `avoid-border-all` | performance | S | paid | `Border.all` → const-friendly alternative. | nice-to-have |
| `avoid-incorrect-image-opacity` | correctness | S | paid | Opacity widget wrapping Image (use color/opacity). | nice-to-have |
| `avoid-missing-image-alt` | style | S | paid | Image without a semantic label (a11y). | nice-to-have |
| `avoid-wrapping-in-padding` | style | LT | paid | Padding widget where Container padding suffices. | nice-to-have |
| `prefer-sized-box-square` | style | S | paid | `SizedBox(w:x,h:x)` → `SizedBox.square`. | nice-to-have |
| `prefer-using-list-view` | performance | LT | paid | Column + SingleChildScrollView → ListView. | nice-to-have |
| `prefer-define-hero-tag` | correctness | LT | paid | Hero without a tag. | nice-to-have |
| `prefer-void-callback` | style | S | paid | `void Function()` → `VoidCallback`. | nice-to-have |
| `avoid-unnecessary-gesture-detector` | performance | LT | paid | GestureDetector wrapping an already-tappable widget. | nice-to-have |
| `prefer-text-rich` | style | LT | paid | `Text.rich`/RichText guidance. | post-1.0 |
| `proper-super-calls` | correctness | FT | paid | initState/dispose super-call ordering (verify vs proper-super-init-state). | post-1.0 |

## 2h. test domain (new)

Activated for `*_test.dart`. Falcon ships none of these today.

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-duplicate-test-assertions` | correctness | S | paid | Same assertion repeated in a test. | must-have |
| `missing-test-assertion` | correctness | LT | paid | Test body with no expect/verify. | must-have |
| `avoid-empty-test-groups` | correctness | S | paid | `group()` with no tests. | nice-to-have |
| `prefer-unique-test-names` | suspicious | S | paid | Duplicate test descriptions. | nice-to-have |
| `prefer-correct-test-file-name` | style | X | paid | Test file naming convention. | nice-to-have |
| `prefer-expect-later` | correctness | FT | paid | Async matcher needs `expectLater`. | nice-to-have |
| `prefer-test-matchers` | style | FT | paid | Use dedicated matchers over manual expect. | nice-to-have |
| `avoid-top-level-members-in-tests` | suspicious | S | paid | Top-level state leaking across tests. | nice-to-have |
| `avoid-missing-test-files` | correctness | X | paid | Source file without a matching test. | post-1.0 |
| `avoid-misused-test-matchers` | correctness | FT | paid | Matcher applied to the wrong type. | post-1.0 |
| `format-test-name` | style | S | paid | Test description formatting. | post-1.0 |
| `prefer-test-structure` | style | S | paid | Arrange/act/assert structure. | post-1.0 |

## 2i. bloc domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `emit-new-bloc-state-instances` | correctness | FT | paid | Emitting the same state instance. | must-have |
| `prefer-sealed-bloc-events` | correctness | FT | paid | Bloc events should be sealed. | must-have |
| `prefer-sealed-bloc-state` | correctness | FT | paid | Bloc state should be sealed. | must-have |
| `prefer-immutable-bloc-events` | correctness | FT | paid | Bloc events must be immutable. | must-have |
| `prefer-immutable-bloc-state` | correctness | FT | paid | Bloc state must be immutable. | must-have |
| `avoid-duplicate-bloc-event-handlers` | correctness | FT | paid | Same event registered twice. | must-have |
| `check-is-not-closed-after-async-gap` | correctness | FT | paid | `emit` after await without an isClosed check. | must-have |
| `handle-bloc-event-subclasses` | correctness | FT | paid | `on<Event>` missing subclass handling. | nice-to-have |
| `avoid-passing-bloc-to-bloc` | suspicious | FT | paid | Bloc dependency anti-pattern. | nice-to-have |
| `avoid-passing-build-context-to-blocs` | suspicious | FT | paid | BuildContext leaking into a bloc. | nice-to-have |
| `avoid-bloc-public-fields` | style | LT | paid | Bloc encapsulation (fields). | nice-to-have |
| `avoid-bloc-public-methods` | style | LT | paid | Bloc encapsulation (methods). | nice-to-have |
| `prefer-bloc-event-suffix` | style | S | paid | Event naming convention. | nice-to-have |
| `prefer-bloc-state-suffix` | style | S | paid | State naming convention. | nice-to-have |
| `prefer-multi-bloc-provider` | style | LT | paid | Nested providers → MultiBlocProvider. | nice-to-have |
| `avoid-empty-build-when` | correctness | S | paid | Empty `buildWhen` callback. | post-1.0 |
| `avoid-cubits` | style | LT | paid | Opinionated: prefer Bloc over Cubit. | post-1.0 |
| `avoid-returning-value-from-cubit-methods` | style | LT | paid | Cubit methods should be void. | post-1.0 |

## 2j. riverpod domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-ref-watch-outside-build` | correctness | FT | paid | `ref.watch` outside build. | must-have |
| `avoid-ref-read-inside-build` | correctness | FT | paid | `ref.read` inside build. | must-have |
| `use-ref-read-synchronously` | correctness | FT | paid | Ref used after await without mounted. | must-have |
| `use-ref-and-state-synchronously` | correctness | FT | paid | Ref/state used after await without mounted. | must-have |
| `avoid-calling-notifier-members-inside-build` | correctness | FT | paid | Mutating a notifier during build. | must-have |
| `dispose-provided-instances` | correctness | FT | paid | Provided disposables not disposed. | must-have |
| `avoid-unnecessary-consumer-widgets` | performance | LT | paid | ConsumerWidget without ref use. | must-have |
| `avoid-ref-inside-state-dispose` | correctness | FT | paid | ref used in dispose. | nice-to-have |
| `avoid-public-notifier-properties` | style | LT | paid | Notifier exposing mutable state. | nice-to-have |
| `prefer-immutable-provider-arguments` | correctness | FT | paid | Provider family args must be immutable. | nice-to-have |
| `prefer-riverpod-notifier-suffix` | style | S | paid | Notifier naming convention. | nice-to-have |
| `prefer-riverpod-provider-suffix` | style | S | paid | Provider naming convention. | nice-to-have |
| `prefer-correct-notifier-file-name` | style | X | paid | Notifier file naming convention. | nice-to-have |
| `prefer-correct-provider-file-name` | style | X | paid | Provider file naming convention. | nice-to-have |
| `avoid-notifier-constructors` | style | LT | paid | Notifiers shouldn't declare constructors. | post-1.0 |
| `prefer-single-notifier-per-file` | style | S | paid | One notifier per file. | post-1.0 |

## 2k. provider domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-read-inside-build` | correctness | FT | paid | `.read` inside build. | must-have |
| `avoid-watch-outside-build` | correctness | FT | paid | `.watch` outside build. | must-have |
| `dispose-providers` | correctness | FT | paid | Provided disposables not disposed. | must-have |
| `avoid-instantiating-in-value-provider` | correctness | FT | paid | `.value` provider creating a new instance. | nice-to-have |
| `prefer-multi-provider` | style | LT | paid | Nested providers → MultiProvider. | nice-to-have |
| `prefer-provider-extensions` | style | LT | paid | Use context extensions over `Provider.of`. | nice-to-have |

## 2l. flutter_hooks domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `avoid-conditional-hooks` | correctness | S | paid | Hooks called conditionally break rules-of-hooks. | must-have |
| `avoid-hooks-outside-build` | correctness | FT | paid | Hook used outside a build method. | must-have |
| `avoid-misused-hooks` | correctness | FT | paid | Hook misuse. | must-have |
| `avoid-unnecessary-hook-widgets` | performance | LT | paid | HookWidget with no hooks. | must-have |
| `prefer-use-callback` | performance | FT | paid | Wrap callbacks in `useCallback`. | nice-to-have |
| `prefer-use-memo` | performance | FT | paid | Wrap values in `useMemo`. | nice-to-have |

## 2m. equatable domain (new)

| DCM rule | falcon group | analysis | paywall | description | priority |
|---|---|---|---|---|---|
| `list-all-equatable-fields` | correctness | FT | paid | Field missing from Equatable props. | must-have |
| `add-equatable-props` | correctness | FT | paid | Equatable subclass missing a props override. | must-have |
| `extend-equatable` | style | LT | paid | Value class should extend Equatable. | nice-to-have |
| `prefer-equatable-mixin` | style | LT | paid | Use EquatableMixin where appropriate. | nice-to-have |

## Deduplication notes

These DCM rows are covered elsewhere and are **not** counted in the DCM totals:

| DCM rule | listed instead as | reason |
|---|---|---|
| `avoid-collection-methods-with-unrelated-types` | official `collection_methods_unrelated_type` | same check; prefer official naming |
| `prefer-contains` | official `prefer_contains` | same check; prefer official naming |
| `avoid-self-assignment` | official `no_self_assignments` | same check; prefer official naming |
| `avoid-unnecessary-parentheses` | official `unnecessary_parenthesis` | same check; prefer official naming |
| `avoid-unnecessary-statements` | official `unnecessary_statements` | same check; prefer official naming |
| `avoid-banned-imports` | cross-file `banned-imports` | same architecture-boundary check; lives in the cross-file pass |

Related-but-distinct pairs kept separately (noted for implementers):
`avoid-collection-equality-checks` (collections) vs official
`unrelated_type_equality_checks` (unrelated types); DCM's granular BuildContext
checks (`use-setstate-synchronously`, `use-closest-build-context`) vs official
`use_build_context_synchronously`; `avoid-duplicate-switch-case-conditions` vs
falcon's shipped `no-duplicate-case-values`; `avoid-self-compare` vs falcon's
shipped `no-self-comparisons`.

**Excluded from the survey entirely** (already in falcon, or too niche for 1.0):
~55 DCM rules that falcon already ports, and the flame (4), patrol (2),
mocktail (4), get_it (1), and fake_async (1) niche-package rule sets.

---

# 3. Cross-file rules (11)

Whole-project / app-wide rules, living in the `cross-file` config section
(renamed from `project`; see the roadmap). Falcon already ships three cross-file
rules; these 11 are the proposed additions. Difficulty reflects existing
infrastructure: **A** reuses the resolved directive graph / lexical name index /
`ProjectIndex` as-is; **B** needs a new pubspec index
(`dependencies`/`dev_dependencies`/`flutter.assets`); **C** needs a new
asset/l10n index (filesystem walk and/or `.arb` parsing).

**Shipped baseline:** `unused-code`, `unused-files`, `unnecessary-nullable`.

## 3a. Tier 1 — must-have (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-dependencies` | correctness | needs-cross-file/project | B | A package in pubspec `dependencies:` that no source file imports via `package:<dep>/…`. |
| `undeclared-dependencies` | correctness | needs-cross-file/project | B | A `package:<x>/…` import whose `<x>` is neither a declared (dev)dependency nor the package itself. |
| `no-import-cycles` | correctness | needs-cross-file/project | A | Import/export cycle between library files (SCC over the resolved directive graph). |
| `banned-imports` | correctness | needs-cross-file/project | A | User-configured architecture boundaries — files matching glob X may not import glob Y. |

## 3b. Tier 2 — nice-to-have (3)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `unused-assets` | correctness | needs-cross-file/project | C | Asset declared in pubspec `flutter: assets:` but no string literal references its path. |
| `unused-l10n` | correctness | needs-cross-file/project | C | Generated localization key (`.arb`) never referenced through the generated accessor. |
| `unexported-public-api` | style | needs-cross-file/project | A | Public declaration used across the package but never surfaced through a barrel/public library. |

## 3c. Tier 3 — post-1.0 (4)

| rule | falcon group | analysis | difficulty | description |
|---|---|---|---|---|
| `missing-assets` | suspicious | needs-cross-file/project | C | String literal that looks like an asset path (`assets/…`) not declared in pubspec. |
| `unused-dev-dependencies` | correctness | needs-cross-file/project | B | Dev-dependency imported nowhere under `test/`, `tool/`, `integration_test/`. |
| `no-deep-package-imports` | correctness | needs-cross-file/project | A | Importing another package's internal file past its public barrel. |
| `duplicate-barrel-exports` | suspicious | needs-cross-file/project | A | Same symbol re-exported by two barrels → ambiguous import. |
