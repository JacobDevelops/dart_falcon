//! Static metadata for every registered rule: its group, domains, whether it is
//! part of the recommended preset, and whether it is a project-level rule. This
//! is the contract that ties each rule to the biome-style config schema —
//! `linter.rules.<group>.<name>` for file rules, `project.rules.<group>.<name>`
//! for project (cross-file) rules (`project: true`).
//!
//! Every rule in `all_rules()` must have exactly one entry here, and vice
//! versa (enforced by `tests/meta_tests.rs`).

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

/// Static metadata for a single rule.
pub struct RuleMeta {
    pub name: &'static str,
    pub group: &'static str,
    pub domains: &'static [&'static str],
    pub recommended: bool,
    /// Whether this is a project-level (cross-file) rule, configured under the
    /// top-level `project` block rather than `linter`. False for file rules.
    pub project: bool,
}

const FLUTTER: &[&str] = &["flutter"];
const NONE: &[&str] = &[];

/// Metadata for every registered rule. Keep in sync with `all_rules()`.
pub const RULE_METADATA: &[RuleMeta] = &[
    // ── complexity ──────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-nested-conditional-expressions",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-redundant-async",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unnecessary-type-assertions",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unnecessary-type-casts",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_inverted_boolean_expressions",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_nested_if",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max_lines_for_file",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max_lines_for_function",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max_parameters_for_function",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "max_switch_cases",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "cyclomatic_complexity",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "maximum_nesting_level",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    // Not in the recommended preset: faithfully reproducing dart_code_linter
    // requires type information. dcl only flags a boolean-literal comparison
    // when the other operand's static type is non-nullable `bool`; `x == true`
    // is the correct null-safe idiom for a `bool?` and must not be flagged.
    // Without type resolution falcon only flags provably-boolean operands
    // (literals, negations, `is` checks, comparison/logical expressions), so the
    // rule is opt-in rather than on by default.
    RuleMeta {
        name: "no-boolean-literal-compare",
        group: "complexity",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "prefer-conditional-expressions",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-extracting-callbacks",
        group: "complexity",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-immediate-return",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-moving-to-variable",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer_iterable_any",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer_iterable_every",
        group: "complexity",
        domains: NONE,
        recommended: true,
        project: false,
    },
    // ── correctness ─────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-global-state",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-returning-widgets",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unused-parameters",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_mutable_global_variables",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_unused_parameters",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "correct_order_for_super_dispose",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "unnecessary_flutter_imports",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    // Project-level (cross-file) rules — CLI-only, run in the project pass.
    RuleMeta {
        name: "unused-files",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: true,
    },
    RuleMeta {
        name: "unused-code",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: true,
    },
    RuleMeta {
        // Off in the recommended preset: heuristic without type resolution
        // (flags nullable params never passed null project-wide). Opt in.
        name: "unnecessary-nullable",
        group: "correctness",
        domains: NONE,
        recommended: false,
        project: true,
    },
    RuleMeta {
        name: "unnecessary_nullable_return_type",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use_once_constructors_once_provider",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    // ── performance ─────────────────────────────────────────────────────────
    RuleMeta {
        name: "prefer-const-border-radius",
        group: "performance",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-correct-edge-insets-constructor",
        group: "performance",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer_declaring_const_constructor",
        group: "performance",
        domains: NONE,
        recommended: true,
        project: false,
    },
    // ── style ─────────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-late-keyword",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-non-null-assertion",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-top-level-member-access",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_abbreviations_in_doc_comments",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_positional_fields_in_records",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "binary-expression-operand-order",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "boolean_prefixes",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "class_members_ordering",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "double-literal-format",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "format-comment",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "member-ordering",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "newline-before-return",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-magic-number",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-object-declaration",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no_magic_number",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-async-await",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-correct-identifier-length",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-correct-type-name",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-first",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-iterable-of",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-last",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer-trailing-comma",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer_dedicated_media_query_methods",
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer_underscore_for_unused_callback_parameters",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use-design-system-item",
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "use_spacer_as_expanded_child",
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_single_child_column_or_row",
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "prefer_async_callback",
        group: "style",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_redundant_pattern_field_names",
        group: "style",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper_controller_dispose",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper_expanded_and_flexible",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper_from_environment",
        group: "correctness",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "proper_super_init_state",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no_self_comparisons",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    // ── suspicious ──────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-dynamic",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        // Off in the recommended preset: without type resolution falcon cannot
        // tell a discarded meaningful return from a side-effect call, making the
        // rule inherently false-positive heavy (see rule impl). Opt in explicitly.
        name: "avoid-ignoring-return-values",
        group: "suspicious",
        domains: NONE,
        recommended: false,
        project: false,
    },
    RuleMeta {
        name: "avoid-passing-async-when-sync-expected",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-throw-in-catch-block",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid-unrelated-type-assertions",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "avoid_empty_blocks",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-empty-block",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-equal-arguments",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no-equal-then-else",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no_duplicate_case_values",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
    RuleMeta {
        name: "no_empty_block",
        group: "suspicious",
        domains: NONE,
        recommended: true,
        project: false,
    },
];

/// Look up metadata for a rule by its registered name.
pub fn meta_for(name: &str) -> Option<&'static RuleMeta> {
    RULE_METADATA.iter().find(|m| m.name == name)
}
