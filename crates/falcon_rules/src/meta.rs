//! Static metadata for every registered rule: its group, domains, whether it is
//! part of the recommended preset, and whether it is a project-level rule. This
//! is the contract that ties each rule to the biome-style config schema —
//! `linter.rules.<group>.<name>` for file rules, `project.rules.<group>.<name>`
//! for project (cross-file) rules (`project: true`).
//!
//! Every rule in `all_rules()` must have exactly one entry here, and vice
//! versa (enforced by `tests/meta_tests.rs`).

use falcon_config::{FalconConfig, RuleConfiguration, RulePlainConfiguration, Rules};

/// Group categories a rule can belong to.
pub const GROUPS: &[&str] = &[
    "complexity",
    "correctness",
    "performance",
    "style",
    "suspicious",
];

/// Known domains a rule can be gated by (see `linter.domains`).
pub const DOMAINS: &[&str] = &["flutter"];

/// Upstream provenance of a rule: which linter it was ported from (with the
/// upstream rule id, which occasionally differs from falcon's), or whether it is
/// original to falcon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleSource {
    /// Ported from dart_code_linter; carries the upstream rule/command id.
    DartCodeLinter(&'static str),
    /// Ported from pyramid_lint; carries the upstream rule id.
    PyramidLint(&'static str),
    /// Adopted from the official `package:lints` / `package:flutter_lints`
    /// rule sets; carries the official (snake_case) lint id.
    Lints(&'static str),
    /// Original to falcon — no upstream equivalent.
    Falcon,
}

/// Static metadata for a single rule.
pub struct RuleMeta {
    pub name: &'static str,
    pub group: &'static str,
    pub domains: &'static [&'static str],
    pub recommended: bool,
    /// Whether this is a project-level (cross-file) rule, configured under the
    /// top-level `project` block rather than `linter`. False for file rules.
    pub project: bool,
    /// Upstream provenance (biome-style): which linter the rule was ported from,
    /// or `Falcon` for original rules.
    pub source: RuleSource,
}

const FLUTTER: &[&str] = &["flutter"];
const NONE: &[&str] = &[];

/// Metadata for every registered rule. Keep in sync with `all_rules()`.
pub const RULE_METADATA: &[RuleMeta] = &[
    // ── complexity ──────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-unnecessary-containers",
        source: RuleSource::Lints("avoid_unnecessary_containers"),
        group: "complexity",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-overrides",
        source: RuleSource::Lints("unnecessary_overrides"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-function-literals-in-foreach-calls",
        source: RuleSource::Lints("avoid_function_literals_in_foreach_calls"),
        group: "complexity",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "prefer-conditional-assignment",
        source: RuleSource::Lints("prefer_conditional_assignment"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-for-elements-to-map-from-iterable",
        source: RuleSource::Lints("prefer_for_elements_to_map_fromIterable"),
        group: "complexity",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "prefer-if-null-operators",
        source: RuleSource::Lints("prefer_if_null_operators"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-null-aware-operators",
        source: RuleSource::Lints("prefer_null_aware_operators"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-const",
        source: RuleSource::Lints("unnecessary_const"),
        group: "complexity",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-getters-setters",
        source: RuleSource::Lints("unnecessary_getters_setters"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-nested-conditional-expressions",
        source: RuleSource::DartCodeLinter("avoid-nested-conditional-expressions"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-redundant-async",
        source: RuleSource::DartCodeLinter("avoid-redundant-async"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unnecessary-type-assertions",
        source: RuleSource::DartCodeLinter("avoid-unnecessary-type-assertions"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unnecessary-type-casts",
        source: RuleSource::DartCodeLinter("avoid-unnecessary-type-casts"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-inverted-boolean-expressions",
        source: RuleSource::PyramidLint("avoid_inverted_boolean_expressions"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-nested-if",
        source: RuleSource::PyramidLint("avoid_nested_if"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max-lines-for-file",
        source: RuleSource::PyramidLint("max_lines_for_file"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max-lines-for-function",
        source: RuleSource::PyramidLint("max_lines_for_function"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max-parameters-for-function",
        source: RuleSource::PyramidLint("max_parameters_for_function"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max-switch-cases",
        source: RuleSource::PyramidLint("max_switch_cases"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "cyclomatic-complexity",
        source: RuleSource::PyramidLint("cyclomatic_complexity"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "maximum-nesting-level",
        source: RuleSource::PyramidLint("maximum_nesting_level"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    // In the recommended preset now that the type-resolution layer backs it: the
    // rule flags a boolean-literal comparison whose other operand is provably
    // boolean (literals, negations, `is` checks, comparison/logical expressions)
    // or a local/param the resolver infers to be a *non-nullable* `bool`. A
    // `bool?` operand resolves to a nullable bool and stays exempt — `x == true`
    // is its idiomatic null-safe form.
    RuleMeta {
        name: "no-boolean-literal-compare",
        source: RuleSource::DartCodeLinter("no-boolean-literal-compare"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-conditional-expressions",
        source: RuleSource::DartCodeLinter("prefer-conditional-expressions"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-extracting-callbacks",
        source: RuleSource::DartCodeLinter("prefer-extracting-callbacks"),
        group: "complexity",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-immediate-return",
        source: RuleSource::DartCodeLinter("prefer-immediate-return"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-moving-to-variable",
        source: RuleSource::DartCodeLinter("prefer-moving-to-variable"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-iterable-any",
        source: RuleSource::PyramidLint("prefer_iterable_any"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-iterable-every",
        source: RuleSource::PyramidLint("prefer_iterable_every"),
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    // ── correctness ─────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-web-libraries-in-flutter",
        source: RuleSource::Lints("avoid_web_libraries_in_flutter"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-logic-in-create-state",
        source: RuleSource::Lints("no_logic_in_create_state"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-relative-lib-imports",
        source: RuleSource::Lints("avoid_relative_lib_imports"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "hash-and-equals",
        source: RuleSource::Lints("hash_and_equals"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "valid-regexps",
        source: RuleSource::Lints("valid_regexps"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "implementation-imports",
        source: RuleSource::Lints("implementation_imports"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-global-state",
        source: RuleSource::DartCodeLinter("avoid-global-state"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-returning-widgets",
        source: RuleSource::DartCodeLinter("avoid-returning-widgets"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unused-parameters",
        source: RuleSource::DartCodeLinter("avoid-unused-parameters"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-mutable-global-variables",
        source: RuleSource::PyramidLint("avoid_mutable_global_variables"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "correct-order-for-super-dispose",
        source: RuleSource::PyramidLint("correct_order_for_super_dispose"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-flutter-imports",
        source: RuleSource::Falcon,
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    // Project-level (cross-file) rules — CLI-only, run in the project pass.
    RuleMeta {
        name: "unused-files",
        source: RuleSource::DartCodeLinter("check-unused-files"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: true,
    },
    RuleMeta {
        name: "unused-code",
        source: RuleSource::DartCodeLinter("check-unused-code"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: true,
    },
    RuleMeta {
        // In the recommended preset now that the type-resolution layer backs it:
        // it flags a private declaration's nullable param only when every visible
        // call site is proven to pass a non-null value (a cross-file return-type
        // index plus local inference decide non-nullability; anything uncertain
        // suppresses). Restricting to `_`-prefixed names keeps all call sites in
        // view, which is what makes the heuristic sound.
        name: "unnecessary-nullable",
        source: RuleSource::DartCodeLinter("check-unnecessary-nullable"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: true,
    },
    RuleMeta {
        name: "unnecessary-nullable-return-type",
        source: RuleSource::PyramidLint("unnecessary_nullable_return_type"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-once-constructors-once-provider",
        source: RuleSource::PyramidLint("use_once_constructors_once_provider"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    // ── performance ─────────────────────────────────────────────────────────
    RuleMeta {
        name: "sized-box-for-whitespace",
        source: RuleSource::Lints("sized_box_for_whitespace"),
        group: "performance",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-to-list-in-spreads",
        source: RuleSource::Lints("unnecessary_to_list_in_spreads"),
        group: "performance",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-const-border-radius",
        source: RuleSource::DartCodeLinter("prefer-const-border-radius"),
        group: "performance",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-correct-edge-insets-constructor",
        source: RuleSource::DartCodeLinter("prefer-correct-edge-insets-constructor"),
        group: "performance",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-declaring-const-constructor",
        source: RuleSource::PyramidLint("prefer_declaring_const_constructor"),
        group: "performance",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    // ── style ─────────────────────────────────────────────────────────────
    RuleMeta {
        name: "sort-child-properties-last",
        source: RuleSource::Lints("sort_child_properties_last"),
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-full-hex-values-for-flutter-colors",
        source: RuleSource::Lints("use_full_hex_values_for_flutter_colors"),
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "camel-case-extensions",
        source: RuleSource::Lints("camel_case_extensions"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "camel-case-types",
        source: RuleSource::Lints("camel_case_types"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "curly-braces-in-flow-control-structures",
        source: RuleSource::Lints("curly_braces_in_flow_control_structures"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "dangling-library-doc-comments",
        source: RuleSource::Lints("dangling_library_doc_comments"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "non-constant-identifier-names",
        source: RuleSource::Lints("non_constant_identifier_names"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-generic-function-type-aliases",
        source: RuleSource::Lints("prefer_generic_function_type_aliases"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-is-empty",
        source: RuleSource::Lints("prefer_is_empty"),
        group: "style",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "prefer-is-not-empty",
        source: RuleSource::Lints("prefer_is_not_empty"),
        group: "style",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "prefer-iterable-where-type",
        source: RuleSource::Lints("prefer_iterable_whereType"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-typing-uninitialized-variables",
        source: RuleSource::Lints("prefer_typing_uninitialized_variables"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "provide-deprecation-message",
        source: RuleSource::Lints("provide_deprecation_message"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unintended-html-in-doc-comment",
        source: RuleSource::Lints("unintended_html_in_doc_comment"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-string-in-part-of-directives",
        source: RuleSource::Lints("use_string_in_part_of_directives"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-init-to-null",
        source: RuleSource::Lints("avoid_init_to_null"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-return-types-on-setters",
        source: RuleSource::Lints("avoid_return_types_on_setters"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-single-cascade-in-expression-statements",
        source: RuleSource::Lints("avoid_single_cascade_in_expression_statements"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "constant-identifier-names",
        source: RuleSource::Lints("constant_identifier_names"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "empty-constructor-bodies",
        source: RuleSource::Lints("empty_constructor_bodies"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "library-prefixes",
        source: RuleSource::Lints("library_prefixes"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-leading-underscores-for-library-prefixes",
        source: RuleSource::Lints("no_leading_underscores_for_library_prefixes"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-leading-underscores-for-local-identifiers",
        source: RuleSource::Lints("no_leading_underscores_for_local_identifiers"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-adjacent-string-concatenation",
        source: RuleSource::Lints("prefer_adjacent_string_concatenation"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-collection-literals",
        source: RuleSource::Lints("prefer_collection_literals"),
        group: "style",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "prefer-final-fields",
        source: RuleSource::Lints("prefer_final_fields"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-function-declarations-over-variables",
        source: RuleSource::Lints("prefer_function_declarations_over_variables"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-initializing-formals",
        source: RuleSource::Lints("prefer_initializing_formals"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-inlined-adds",
        source: RuleSource::Lints("prefer_inlined_adds"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-is-not-operator",
        source: RuleSource::Lints("prefer_is_not_operator"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-spread-collections",
        source: RuleSource::Lints("prefer_spread_collections"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "slash-for-doc-comments",
        source: RuleSource::Lints("slash_for_doc_comments"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "type-init-formals",
        source: RuleSource::Lints("type_init_formals"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-brace-in-string-interps",
        source: RuleSource::Lints("unnecessary_brace_in_string_interps"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-constructor-name",
        source: RuleSource::Lints("unnecessary_constructor_name"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-late",
        source: RuleSource::Lints("unnecessary_late"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-library-name",
        source: RuleSource::Lints("unnecessary_library_name"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-new",
        source: RuleSource::Lints("unnecessary_new"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-nullable-for-final-variable-declarations",
        source: RuleSource::Lints("unnecessary_nullable_for_final_variable_declarations"),
        group: "style",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-string-escapes",
        source: RuleSource::Lints("unnecessary_string_escapes"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-string-interpolations",
        source: RuleSource::Lints("unnecessary_string_interpolations"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-this",
        source: RuleSource::Lints("unnecessary_this"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-function-type-syntax-for-parameters",
        source: RuleSource::Lints("use_function_type_syntax_for_parameters"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-super-parameters",
        source: RuleSource::Lints("use_super_parameters"),
        group: "style",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "avoid-late-keyword",
        source: RuleSource::DartCodeLinter("avoid-late-keyword"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-non-null-assertion",
        source: RuleSource::DartCodeLinter("avoid-non-null-assertion"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-top-level-member-access",
        source: RuleSource::Falcon,
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-abbreviations-in-doc-comments",
        source: RuleSource::PyramidLint("avoid_abbreviations_in_doc_comments"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-positional-fields-in-records",
        source: RuleSource::PyramidLint("avoid_positional_fields_in_records"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "binary-expression-operand-order",
        source: RuleSource::DartCodeLinter("binary-expression-operand-order"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "boolean-prefixes",
        source: RuleSource::PyramidLint("boolean_prefixes"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "class-members-ordering",
        source: RuleSource::PyramidLint("class_members_ordering"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "double-literal-format",
        source: RuleSource::DartCodeLinter("double-literal-format"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "format-comment",
        source: RuleSource::DartCodeLinter("format-comment"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "member-ordering",
        source: RuleSource::DartCodeLinter("member-ordering"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "newline-before-return",
        source: RuleSource::DartCodeLinter("newline-before-return"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-magic-number",
        source: RuleSource::DartCodeLinter("no-magic-number"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-object-declaration",
        source: RuleSource::DartCodeLinter("no-object-declaration"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-async-await",
        source: RuleSource::DartCodeLinter("prefer-async-await"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-correct-identifier-length",
        source: RuleSource::DartCodeLinter("prefer-correct-identifier-length"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-correct-type-name",
        source: RuleSource::DartCodeLinter("prefer-correct-type-name"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-first",
        source: RuleSource::DartCodeLinter("prefer-first"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-iterable-of",
        source: RuleSource::DartCodeLinter("prefer-iterable-of"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-last",
        source: RuleSource::DartCodeLinter("prefer-last"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-trailing-comma",
        source: RuleSource::DartCodeLinter("prefer-trailing-comma"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-dedicated-media-query-methods",
        source: RuleSource::PyramidLint("prefer_dedicated_media_query_methods"),
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-underscore-for-unused-callback-parameters",
        source: RuleSource::PyramidLint("prefer_underscore_for_unused_callback_parameters"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-design-system-item",
        source: RuleSource::DartCodeLinter("use-design-system-item"),
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-spacer-as-expanded-child",
        source: RuleSource::PyramidLint("use_spacer_as_expanded_child"),
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-single-child-column-or-row",
        source: RuleSource::PyramidLint("avoid_single_child_column_or_row"),
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-async-callback",
        source: RuleSource::PyramidLint("prefer_async_callback"),
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-redundant-pattern-field-names",
        source: RuleSource::PyramidLint("avoid_redundant_pattern_field_names"),
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper-controller-dispose",
        source: RuleSource::PyramidLint("proper_controller_dispose"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper-expanded-and-flexible",
        source: RuleSource::PyramidLint("proper_expanded_and_flexible"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper-from-environment",
        source: RuleSource::PyramidLint("proper_from_environment"),
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper-super-init-state",
        source: RuleSource::PyramidLint("proper_super_init_state"),
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-self-comparisons",
        source: RuleSource::PyramidLint("no_self_comparisons"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    // ── suspicious ──────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-print",
        source: RuleSource::Lints("avoid_print"),
        group: "suspicious",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-empty-else",
        source: RuleSource::Lints("avoid_empty_else"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-shadowing-type-parameters",
        source: RuleSource::Lints("avoid_shadowing_type_parameters"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "empty-catches",
        source: RuleSource::Lints("empty_catches"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-wildcard-variable-uses",
        source: RuleSource::Lints("no_wildcard_variable_uses"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-returning-null-for-void",
        source: RuleSource::Lints("avoid_returning_null_for_void"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "control-flow-in-finally",
        source: RuleSource::Lints("control_flow_in_finally"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "empty-statements",
        source: RuleSource::Lints("empty_statements"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "recursive-getters",
        source: RuleSource::Lints("recursive_getters"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-null-aware-assignments",
        source: RuleSource::Lints("unnecessary_null_aware_assignments"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary-null-in-if-null-operators",
        source: RuleSource::Lints("unnecessary_null_in_if_null_operators"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-rethrow-when-possible",
        source: RuleSource::Lints("use_rethrow_when_possible"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-dynamic",
        source: RuleSource::DartCodeLinter("avoid-dynamic"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        // In the recommended preset now that the type-resolution layer backs it:
        // the callee's declared return type decides it — a known `void` return is
        // safe to discard, a known non-void return is flagged, and only an
        // unknown return type falls back to the receiver-less side-effect
        // allowlist. This removes the false positives that kept it opt-in.
        name: "avoid-ignoring-return-values",
        source: RuleSource::DartCodeLinter("avoid-ignoring-return-values"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-passing-async-when-sync-expected",
        source: RuleSource::DartCodeLinter("avoid-passing-async-when-sync-expected"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-throw-in-catch-block",
        source: RuleSource::DartCodeLinter("avoid-throw-in-catch-block"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unrelated-type-assertions",
        source: RuleSource::DartCodeLinter("avoid-unrelated-type-assertions"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-empty-block",
        source: RuleSource::DartCodeLinter("no-empty-block"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-equal-arguments",
        source: RuleSource::DartCodeLinter("no-equal-arguments"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-equal-then-else",
        source: RuleSource::DartCodeLinter("no-equal-then-else"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-duplicate-case-values",
        source: RuleSource::PyramidLint("no_duplicate_case_values"),
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
];

/// Legacy rule ids mapped to their canonical id. Covers the pre-1.0
/// `snake_case` → `kebab-case` renames and the removed twin rules (whose
/// pyramid_lint-derived variants were merged into the surviving dart_code_linter
/// rule). Keeps old `falcon.json` rule keys and old `// falcon-ignore
/// lint/<group>/<id>` suppression paths resolving after the id normalization.
pub const RULE_ALIASES: &[(&str, &str)] = &[
    // ── snake_case → kebab-case renames ──
    (
        "avoid_inverted_boolean_expressions",
        "avoid-inverted-boolean-expressions",
    ),
    ("avoid_nested_if", "avoid-nested-if"),
    ("max_lines_for_file", "max-lines-for-file"),
    ("max_lines_for_function", "max-lines-for-function"),
    ("max_parameters_for_function", "max-parameters-for-function"),
    ("max_switch_cases", "max-switch-cases"),
    ("cyclomatic_complexity", "cyclomatic-complexity"),
    ("maximum_nesting_level", "maximum-nesting-level"),
    ("prefer_iterable_any", "prefer-iterable-any"),
    ("prefer_iterable_every", "prefer-iterable-every"),
    (
        "avoid_mutable_global_variables",
        "avoid-mutable-global-variables",
    ),
    (
        "correct_order_for_super_dispose",
        "correct-order-for-super-dispose",
    ),
    ("unnecessary_flutter_imports", "unnecessary-flutter-imports"),
    (
        "unnecessary_nullable_return_type",
        "unnecessary-nullable-return-type",
    ),
    (
        "use_once_constructors_once_provider",
        "use-once-constructors-once-provider",
    ),
    ("proper_controller_dispose", "proper-controller-dispose"),
    (
        "proper_expanded_and_flexible",
        "proper-expanded-and-flexible",
    ),
    ("proper_from_environment", "proper-from-environment"),
    ("proper_super_init_state", "proper-super-init-state"),
    (
        "prefer_declaring_const_constructor",
        "prefer-declaring-const-constructor",
    ),
    (
        "avoid_abbreviations_in_doc_comments",
        "avoid-abbreviations-in-doc-comments",
    ),
    (
        "avoid_positional_fields_in_records",
        "avoid-positional-fields-in-records",
    ),
    ("boolean_prefixes", "boolean-prefixes"),
    ("class_members_ordering", "class-members-ordering"),
    (
        "prefer_dedicated_media_query_methods",
        "prefer-dedicated-media-query-methods",
    ),
    (
        "prefer_underscore_for_unused_callback_parameters",
        "prefer-underscore-for-unused-callback-parameters",
    ),
    (
        "use_spacer_as_expanded_child",
        "use-spacer-as-expanded-child",
    ),
    (
        "avoid_single_child_column_or_row",
        "avoid-single-child-column-or-row",
    ),
    ("prefer_async_callback", "prefer-async-callback"),
    (
        "avoid_redundant_pattern_field_names",
        "avoid-redundant-pattern-field-names",
    ),
    ("no_self_comparisons", "no-self-comparisons"),
    ("no_duplicate_case_values", "no-duplicate-case-values"),
    // ── unified twins: pyramid_lint variant → surviving canonical rule ──
    ("avoid_unused_parameters", "avoid-unused-parameters"),
    ("no_magic_number", "no-magic-number"),
    ("avoid_empty_blocks", "no-empty-block"),
    ("no_empty_block", "no-empty-block"),
];

/// Resolve a possibly-legacy rule id to its canonical id. Returns the input
/// unchanged when it is already canonical (or simply unknown).
pub fn canonical_rule_name(name: &str) -> &str {
    RULE_ALIASES
        .iter()
        .find(|(old, _)| *old == name)
        .map(|&(_, canonical)| canonical)
        .unwrap_or(name)
}

/// Look up metadata for a rule by its registered name, resolving legacy aliases
/// first so an old id still finds its canonical rule.
pub fn meta_for(name: &str) -> Option<&'static RuleMeta> {
    let canonical = canonical_rule_name(name);
    RULE_METADATA.iter().find(|m| m.name == canonical)
}

/// Suppression-path validation hook: maps a (possibly-legacy) rule name to its
/// `(canonical_name, group, is_project)` so `falcon_analyze` can validate a
/// `// falcon-ignore` path — and record the *canonical* id, so a suppression
/// written with an old id still matches the diagnostic's canonical rule — without
/// depending on this crate. Matches [`falcon_analyze::RuleLookup`].
pub fn suppression_lookup(name: &str) -> Option<(&'static str, &'static str, bool)> {
    meta_for(name).map(|m| (m.name, m.group, m.project))
}

/// Rewrite legacy rule ids used as config keys to their canonical ids, across
/// the base `linter`/`project` rule maps and every override. Applied once after
/// a config is loaded so old `falcon.json` files keep resolving against the
/// canonical rule table. When a legacy id and its canonical id (or two legacy
/// twins) collide in the same group, the more severe level is kept — matching
/// the `falcon migrate` upgrade path.
pub fn canonicalize_config(config: &mut FalconConfig) {
    canonicalize_rules(&mut config.linter.rules);
    canonicalize_rules(&mut config.project.rules);
    for ov in &mut config.overrides {
        if let Some(linter) = &mut ov.linter {
            canonicalize_rules(&mut linter.rules);
        }
        if let Some(project) = &mut ov.project {
            canonicalize_rules(&mut project.rules);
        }
    }
}

fn canonicalize_rules(rules: &mut Rules) {
    for group in rules.groups.values_mut() {
        let renames: Vec<(String, String)> = group
            .keys()
            .filter_map(|key| {
                let canonical = canonical_rule_name(key);
                (canonical != key.as_str()).then(|| (key.clone(), canonical.to_string()))
            })
            .collect();
        for (old, canonical) in renames {
            let Some(cfg) = group.remove(&old) else {
                continue;
            };
            match group.get(&canonical) {
                // A more-or-equally severe entry already occupies the canonical
                // slot — keep it. Otherwise this entry wins (more severe, or the
                // slot is empty).
                Some(existing) if severity_rank(existing) >= severity_rank(&cfg) => {}
                _ => {
                    group.insert(canonical, cfg);
                }
            }
        }
    }
}

/// Severity ordering used to merge colliding config entries: higher wins.
fn severity_rank(cfg: &RuleConfiguration) -> u8 {
    match cfg.level() {
        RulePlainConfiguration::Off => 1,
        RulePlainConfiguration::Info => 2,
        RulePlainConfiguration::On | RulePlainConfiguration::Warn => 3,
        RulePlainConfiguration::Error => 4,
    }
}
