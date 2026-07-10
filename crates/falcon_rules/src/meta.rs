//! Static metadata for every registered rule: its group, domains, and whether
//! it is part of the recommended preset. This is the contract that ties each
//! rule to the biome-style config schema (`linter.rules.<group>.<name>`).
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
    },
    RuleMeta {
        name: "avoid-redundant-async",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-unnecessary-type-assertions",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-unnecessary-type-casts",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid_inverted_boolean_expressions",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid_nested_if",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "max_lines_for_file",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "max_lines_for_function",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "max_parameters_for_function",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "max_switch_cases",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no-boolean-literal-compare",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-conditional-expressions",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-extracting-callbacks",
        group: "complexity",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-immediate-return",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-moving-to-variable",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer_iterable_any",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer_iterable_every",
        group: "complexity",
        domains: NONE,
        recommended: true,
    },
    // ── correctness ─────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-global-state",
        group: "correctness",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-returning-widgets",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-unused-parameters",
        group: "correctness",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid_mutable_global_variables",
        group: "correctness",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid_unused_parameters",
        group: "correctness",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "correct_order_for_super_dispose",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "unnecessary_flutter_imports",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "unnecessary_nullable_return_type",
        group: "correctness",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "use_once_constructors_once_provider",
        group: "correctness",
        domains: FLUTTER,
        recommended: true,
    },
    // ── performance ─────────────────────────────────────────────────────────
    RuleMeta {
        name: "prefer-const-border-radius",
        group: "performance",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-correct-edge-insets-constructor",
        group: "performance",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "prefer_declaring_const_constructor",
        group: "performance",
        domains: NONE,
        recommended: true,
    },
    // ── style ─────────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-late-keyword",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-non-null-assertion",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-top-level-member-access",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid_abbreviations_in_doc_comments",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid_positional_fields_in_records",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "binary-expression-operand-order",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "boolean_prefixes",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "class_members_ordering",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "double-literal-format",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "format-comment",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "member-ordering",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "newline-before-return",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no-magic-number",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no-object-declaration",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no_magic_number",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-async-await",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-correct-identifier-length",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-correct-type-name",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-first",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-iterable-of",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-last",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer-trailing-comma",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "prefer_dedicated_media_query_methods",
        group: "style",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "prefer_underscore_for_unused_callback_parameters",
        group: "style",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "use-design-system-item",
        group: "style",
        domains: FLUTTER,
        recommended: true,
    },
    RuleMeta {
        name: "use_spacer_as_expanded_child",
        group: "style",
        domains: FLUTTER,
        recommended: true,
    },
    // ── suspicious ──────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-dynamic",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-ignoring-return-values",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-passing-async-when-sync-expected",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-throw-in-catch-block",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid-unrelated-type-assertions",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "avoid_empty_blocks",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no-empty-block",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no-equal-arguments",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no-equal-then-else",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no_duplicate_case_values",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
    RuleMeta {
        name: "no_empty_block",
        group: "suspicious",
        domains: NONE,
        recommended: true,
    },
];

/// Look up metadata for a rule by its registered name.
pub fn meta_for(name: &str) -> Option<&'static RuleMeta> {
    RULE_METADATA.iter().find(|m| m.name == name)
}
