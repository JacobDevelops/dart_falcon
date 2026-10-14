//! Rule constructors and configuration metadata for file and cross-file rules.

use falcon_analyze::{CrossFileRule, Rule};
use falcon_config::{FalconConfig, RuleConfiguration, RulePlainConfiguration, Rules};

use crate::{cross_file, lint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleGroup {
    Complexity,
    Correctness,
    Performance,
    Style,
    Suspicious,
}

impl RuleGroup {
    pub const ALL: &'static [Self] = &[
        Self::Complexity,
        Self::Correctness,
        Self::Performance,
        Self::Style,
        Self::Suspicious,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complexity => "complexity",
            Self::Correctness => "correctness",
            Self::Performance => "performance",
            Self::Style => "style",
            Self::Suspicious => "suspicious",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Flutter,
}

impl Domain {
    pub const ALL: &'static [Self] = &[Self::Flutter];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flutter => "flutter",
        }
    }
}

#[derive(Clone, Copy)]
pub enum RuleKind {
    File {
        make: fn() -> Box<dyn Rule>,
        needs_resolution: bool,
    },
    CrossFile(fn() -> Box<dyn CrossFileRule>),
}

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
    pub group: RuleGroup,
    pub domains: &'static [Domain],
    pub recommended: bool,
    pub kind: RuleKind,
    /// Upstream provenance (biome-style): which linter the rule was ported from,
    /// or `Falcon` for original rules.
    pub source: RuleSource,
}

impl RuleMeta {
    pub fn domain_names(&self) -> &'static [&'static str] {
        const FLUTTER_NAMES: &[&str] = &[Domain::Flutter.as_str()];
        match self.domains.first() {
            None => &[],
            Some(Domain::Flutter) => FLUTTER_NAMES,
        }
    }

    pub const fn is_cross_file(&self) -> bool {
        matches!(self.kind, RuleKind::CrossFile(_))
    }
}

const FLUTTER: &[Domain] = &[Domain::Flutter];
const NONE: &[Domain] = &[];

/// The sole constructor table for registered rules.
pub const RULE_METADATA: &[RuleMeta] = &[
    // ── complexity ──────────────────────────────────────────────────────────
    RuleMeta {
        name: "avoid-unnecessary-containers",
        source: RuleSource::Lints("avoid_unnecessary_containers"),
        group: RuleGroup::Complexity,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::avoid_unnecessary_containers::AvoidUnnecessaryContainers)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-overrides",
        source: RuleSource::Lints("unnecessary_overrides"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::unnecessary_overrides::UnnecessaryOverrides),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-function-literals-in-foreach-calls",
        source: RuleSource::Lints("avoid_function_literals_in_foreach_calls"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::avoid_function_literals_in_foreach_calls::AvoidFunctionLiteralsInForeachCalls)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-conditional-assignment",
        source: RuleSource::Lints("prefer_conditional_assignment"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::complexity::prefer_conditional_assignment::PreferConditionalAssignment,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-for-elements-to-map-from-iterable",
        source: RuleSource::Lints("prefer_for_elements_to_map_fromIterable"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::prefer_for_elements_to_map_from_iterable::PreferForElementsToMapFromIterable)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-if-null-operators",
        source: RuleSource::Lints("prefer_if_null_operators"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::prefer_if_null_operators::PreferIfNullOperators),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-null-aware-operators",
        source: RuleSource::Lints("prefer_null_aware_operators"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::prefer_null_aware_operators::PreferNullAwareOperators)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-const",
        source: RuleSource::Lints("unnecessary_const"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::unnecessary_const::UnnecessaryConst),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-getters-setters",
        source: RuleSource::Lints("unnecessary_getters_setters"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::unnecessary_getters_setters::UnnecessaryGettersSetters)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-nested-conditional-expressions",
        source: RuleSource::DartCodeLinter("avoid-nested-conditional-expressions"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::avoid_nested_conditional_expressions::AvoidNestedConditionalExpressions)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-redundant-async",
        source: RuleSource::DartCodeLinter("avoid-redundant-async"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::avoid_redundant_async::AvoidRedundantAsync),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-unnecessary-type-assertions",
        source: RuleSource::DartCodeLinter("avoid-unnecessary-type-assertions"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::avoid_unnecessary_type_assertions::AvoidUnnecessaryTypeAssertions)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-unnecessary-type-casts",
        source: RuleSource::DartCodeLinter("avoid-unnecessary-type-casts"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::avoid_unnecessary_type_casts::AvoidUnnecessaryTypeCasts)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-inverted-boolean-expressions",
        source: RuleSource::PyramidLint("avoid_inverted_boolean_expressions"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::avoid_inverted_boolean_expressions::AvoidInvertedBooleanExpressions)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-nested-if",
        source: RuleSource::PyramidLint("avoid_nested_if"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::avoid_nested_if::AvoidNestedIf),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "max-lines-for-file",
        source: RuleSource::PyramidLint("max_lines_for_file"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::max_lines_for_file::MaxLinesForFile),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "max-lines-for-function",
        source: RuleSource::PyramidLint("max_lines_for_function"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::max_lines_for_function::MaxLinesForFunction),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "max-parameters-for-function",
        source: RuleSource::PyramidLint("max_parameters_for_function"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::max_parameters_for_function::MaxParametersForFunction)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "max-switch-cases",
        source: RuleSource::PyramidLint("max_switch_cases"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::max_switch_cases::MaxSwitchCases),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "cyclomatic-complexity",
        source: RuleSource::PyramidLint("cyclomatic_complexity"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::cyclomatic_complexity::CyclomaticComplexity),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "maximum-nesting-level",
        source: RuleSource::PyramidLint("maximum_nesting_level"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::maximum_nesting_level::MaximumNestingLevel),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-boolean-literal-compare",
        source: RuleSource::DartCodeLinter("no-boolean-literal-compare"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::no_boolean_literal_compare::NoBooleanLiteralCompare)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-conditional-expressions",
        source: RuleSource::DartCodeLinter("prefer-conditional-expressions"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::complexity::prefer_conditional_expressions::PreferConditionalExpressions,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-extracting-callbacks",
        source: RuleSource::DartCodeLinter("prefer-extracting-callbacks"),
        group: RuleGroup::Complexity,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::complexity::prefer_extracting_callbacks::PreferExtractingCallbacks)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-immediate-return",
        source: RuleSource::DartCodeLinter("prefer-immediate-return"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::prefer_immediate_return::PreferImmediateReturn),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-moving-to-variable",
        source: RuleSource::DartCodeLinter("prefer-moving-to-variable"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::prefer_moving_to_variable::PreferMovingToVariable),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-iterable-any",
        source: RuleSource::PyramidLint("prefer_iterable_any"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::prefer_iterable_any::PreferIterableAny),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-iterable-every",
        source: RuleSource::PyramidLint("prefer_iterable_every"),
        group: RuleGroup::Complexity,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::complexity::prefer_iterable_every::PreferIterableEvery),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-web-libraries-in-flutter",
        source: RuleSource::Lints("avoid_web_libraries_in_flutter"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::correctness::avoid_web_libraries_in_flutter::AvoidWebLibrariesInFlutter,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-logic-in-create-state",
        source: RuleSource::Lints("no_logic_in_create_state"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::no_logic_in_create_state::NoLogicInCreateState),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-relative-lib-imports",
        source: RuleSource::Lints("avoid_relative_lib_imports"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::avoid_relative_lib_imports::AvoidRelativeLibImports)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "hash-and-equals",
        source: RuleSource::Lints("hash_and_equals"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::hash_and_equals::HashAndEquals),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "valid-regexps",
        source: RuleSource::Lints("valid_regexps"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::valid_regexps::ValidRegexps),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "implementation-imports",
        source: RuleSource::Lints("implementation_imports"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::implementation_imports::ImplementationImports),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-global-state",
        source: RuleSource::DartCodeLinter("avoid-global-state"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::avoid_global_state::AvoidGlobalState),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-returning-widgets",
        source: RuleSource::DartCodeLinter("avoid-returning-widgets"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::avoid_returning_widgets::AvoidReturningWidgets),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-unused-parameters",
        source: RuleSource::DartCodeLinter("avoid-unused-parameters"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::avoid_unused_parameters::AvoidUnusedParameters),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-mutable-global-variables",
        source: RuleSource::PyramidLint("avoid_mutable_global_variables"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::correctness::avoid_mutable_global_variables::AvoidMutableGlobalVariables,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "correct-order-for-super-dispose",
        source: RuleSource::PyramidLint("correct_order_for_super_dispose"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::correctness::correct_order_for_super_dispose::CorrectOrderForSuperDispose,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-flutter-imports",
        source: RuleSource::Falcon,
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::unnecessary_flutter_imports::UnnecessaryFlutterImports)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "unused-files",
        source: RuleSource::DartCodeLinter("check-unused-files"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::CrossFile(|| Box::new(cross_file::unused_files::UnusedFiles)),
    },
    RuleMeta {
        name: "unused-code",
        source: RuleSource::DartCodeLinter("check-unused-code"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::CrossFile(|| Box::new(cross_file::unused_code::UnusedCode)),
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
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::CrossFile(|| {
            Box::new(cross_file::unnecessary_nullable::UnnecessaryNullable)
        }),
    },
    RuleMeta {
        name: "unnecessary-nullable-return-type",
        source: RuleSource::PyramidLint("unnecessary_nullable_return_type"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::unnecessary_nullable_return_type::UnnecessaryNullableReturnType)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-once-constructors-once-provider",
        source: RuleSource::PyramidLint("use_once_constructors_once_provider"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::use_once_constructors_once_provider::UseOnceConstructorsOnceProvider)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "sized-box-for-whitespace",
        source: RuleSource::Lints("sized_box_for_whitespace"),
        group: RuleGroup::Performance,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::performance::sized_box_for_whitespace::SizedBoxForWhitespace),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-to-list-in-spreads",
        source: RuleSource::Lints("unnecessary_to_list_in_spreads"),
        group: RuleGroup::Performance,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::performance::unnecessary_to_list_in_spreads::UnnecessaryToListInSpreads,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-const-border-radius",
        source: RuleSource::DartCodeLinter("prefer-const-border-radius"),
        group: RuleGroup::Performance,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::performance::prefer_const_border_radius::PreferConstBorderRadius)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-correct-edge-insets-constructor",
        source: RuleSource::DartCodeLinter("prefer-correct-edge-insets-constructor"),
        group: RuleGroup::Performance,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::performance::prefer_correct_edge_insets_constructor::PreferCorrectEdgeInsetsConstructor)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-declaring-const-constructor",
        source: RuleSource::PyramidLint("prefer_declaring_const_constructor"),
        group: RuleGroup::Performance,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::performance::prefer_declaring_const_constructor::PreferDeclaringConstConstructor)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "sort-child-properties-last",
        source: RuleSource::Lints("sort_child_properties_last"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::sort_child_properties_last::SortChildPropertiesLast),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-full-hex-values-for-flutter-colors",
        source: RuleSource::Lints("use_full_hex_values_for_flutter_colors"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::use_full_hex_values_for_flutter_colors::UseFullHexValuesForFlutterColors)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "camel-case-extensions",
        source: RuleSource::Lints("camel_case_extensions"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::camel_case_extensions::CamelCaseExtensions),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "camel-case-types",
        source: RuleSource::Lints("camel_case_types"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::camel_case_types::CamelCaseTypes),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "curly-braces-in-flow-control-structures",
        source: RuleSource::Lints("curly_braces_in_flow_control_structures"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::curly_braces_in_flow_control_structures::CurlyBracesInFlowControlStructures)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "dangling-library-doc-comments",
        source: RuleSource::Lints("dangling_library_doc_comments"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::dangling_library_doc_comments::DanglingLibraryDocComments)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "non-constant-identifier-names",
        source: RuleSource::Lints("non_constant_identifier_names"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::non_constant_identifier_names::NonConstantIdentifierNames)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-generic-function-type-aliases",
        source: RuleSource::Lints("prefer_generic_function_type_aliases"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_generic_function_type_aliases::PreferGenericFunctionTypeAliases)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-is-empty",
        source: RuleSource::Lints("prefer_is_empty"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_is_empty::PreferIsEmpty),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-is-not-empty",
        source: RuleSource::Lints("prefer_is_not_empty"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_is_not_empty::PreferIsNotEmpty),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-iterable-where-type",
        source: RuleSource::Lints("prefer_iterable_whereType"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_iterable_where_type::PreferIterableWhereType),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-typing-uninitialized-variables",
        source: RuleSource::Lints("prefer_typing_uninitialized_variables"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_typing_uninitialized_variables::PreferTypingUninitializedVariables)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "provide-deprecation-message",
        source: RuleSource::Lints("provide_deprecation_message"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::provide_deprecation_message::ProvideDeprecationMessage),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unintended-html-in-doc-comment",
        source: RuleSource::Lints("unintended_html_in_doc_comment"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::unintended_html_in_doc_comment::UnintendedHtmlInDocComment)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-string-in-part-of-directives",
        source: RuleSource::Lints("use_string_in_part_of_directives"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::use_string_in_part_of_directives::UseStringInPartOfDirectives)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-init-to-null",
        source: RuleSource::Lints("avoid_init_to_null"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::avoid_init_to_null::AvoidInitToNull),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-return-types-on-setters",
        source: RuleSource::Lints("avoid_return_types_on_setters"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::avoid_return_types_on_setters::AvoidReturnTypesOnSetters)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-single-cascade-in-expression-statements",
        source: RuleSource::Lints("avoid_single_cascade_in_expression_statements"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::avoid_single_cascade_in_expression_statements::AvoidSingleCascadeInExpressionStatements)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "constant-identifier-names",
        source: RuleSource::Lints("constant_identifier_names"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::constant_identifier_names::ConstantIdentifierNames),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "empty-constructor-bodies",
        source: RuleSource::Lints("empty_constructor_bodies"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::empty_constructor_bodies::EmptyConstructorBodies),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "library-prefixes",
        source: RuleSource::Lints("library_prefixes"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::library_prefixes::LibraryPrefixes),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-leading-underscores-for-library-prefixes",
        source: RuleSource::Lints("no_leading_underscores_for_library_prefixes"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::no_leading_underscores_for_library_prefixes::NoLeadingUnderscoresForLibraryPrefixes)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-leading-underscores-for-local-identifiers",
        source: RuleSource::Lints("no_leading_underscores_for_local_identifiers"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::no_leading_underscores_for_local_identifiers::NoLeadingUnderscoresForLocalIdentifiers)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-adjacent-string-concatenation",
        source: RuleSource::Lints("prefer_adjacent_string_concatenation"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_adjacent_string_concatenation::PreferAdjacentStringConcatenation)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-collection-literals",
        source: RuleSource::Lints("prefer_collection_literals"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_collection_literals::PreferCollectionLiterals),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-final-fields",
        source: RuleSource::Lints("prefer_final_fields"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_final_fields::PreferFinalFields),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-function-declarations-over-variables",
        source: RuleSource::Lints("prefer_function_declarations_over_variables"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_function_declarations_over_variables::PreferFunctionDeclarationsOverVariables)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-initializing-formals",
        source: RuleSource::Lints("prefer_initializing_formals"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_initializing_formals::PreferInitializingFormals),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-inlined-adds",
        source: RuleSource::Lints("prefer_inlined_adds"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_inlined_adds::PreferInlinedAdds),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-is-not-operator",
        source: RuleSource::Lints("prefer_is_not_operator"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_is_not_operator::PreferIsNotOperator),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-spread-collections",
        source: RuleSource::Lints("prefer_spread_collections"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_spread_collections::PreferSpreadCollections),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "slash-for-doc-comments",
        source: RuleSource::Lints("slash_for_doc_comments"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::slash_for_doc_comments::SlashForDocComments),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "type-init-formals",
        source: RuleSource::Lints("type_init_formals"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::type_init_formals::TypeInitFormals),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-brace-in-string-interps",
        source: RuleSource::Lints("unnecessary_brace_in_string_interps"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::unnecessary_brace_in_string_interps::UnnecessaryBraceInStringInterps)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-constructor-name",
        source: RuleSource::Lints("unnecessary_constructor_name"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::unnecessary_constructor_name::UnnecessaryConstructorName)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-late",
        source: RuleSource::Lints("unnecessary_late"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::unnecessary_late::UnnecessaryLate),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-library-name",
        source: RuleSource::Lints("unnecessary_library_name"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::unnecessary_library_name::UnnecessaryLibraryName),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-new",
        source: RuleSource::Lints("unnecessary_new"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::unnecessary_new::UnnecessaryNew),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-nullable-for-final-variable-declarations",
        source: RuleSource::Lints("unnecessary_nullable_for_final_variable_declarations"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::unnecessary_nullable_for_final_variable_declarations::UnnecessaryNullableForFinalVariableDeclarations)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-string-escapes",
        source: RuleSource::Lints("unnecessary_string_escapes"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::unnecessary_string_escapes::UnnecessaryStringEscapes),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-string-interpolations",
        source: RuleSource::Lints("unnecessary_string_interpolations"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::style::unnecessary_string_interpolations::UnnecessaryStringInterpolations,
                )
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "unnecessary-this",
        source: RuleSource::Lints("unnecessary_this"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::unnecessary_this::UnnecessaryThis),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-function-type-syntax-for-parameters",
        source: RuleSource::Lints("use_function_type_syntax_for_parameters"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::use_function_type_syntax_for_parameters::UseFunctionTypeSyntaxForParameters)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-super-parameters",
        source: RuleSource::Lints("use_super_parameters"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::use_super_parameters::UseSuperParameters),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-late-keyword",
        source: RuleSource::DartCodeLinter("avoid-late-keyword"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::avoid_late_keyword::AvoidLateKeyword),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-non-null-assertion",
        source: RuleSource::DartCodeLinter("avoid-non-null-assertion"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::avoid_non_null_assertion::AvoidNonNullAssertion),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-top-level-member-access",
        source: RuleSource::Falcon,
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::avoid_top_level_member_access::AvoidTopLevelMemberAccess)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-abbreviations-in-doc-comments",
        source: RuleSource::PyramidLint("avoid_abbreviations_in_doc_comments"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::avoid_abbreviations_in_doc_comments::AvoidAbbreviationsInDocComments)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-positional-fields-in-records",
        source: RuleSource::PyramidLint("avoid_positional_fields_in_records"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::style::avoid_positional_fields_in_records::AvoidPositionalFieldsInRecords,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "binary-expression-operand-order",
        source: RuleSource::DartCodeLinter("binary-expression-operand-order"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::binary_expression_operand_order::BinaryExpressionOperandOrder)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "boolean-prefixes",
        source: RuleSource::PyramidLint("boolean_prefixes"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::boolean_prefixes::BooleanPrefixes),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "class-members-ordering",
        source: RuleSource::PyramidLint("class_members_ordering"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::class_members_ordering::ClassMembersOrdering),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "double-literal-format",
        source: RuleSource::DartCodeLinter("double-literal-format"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::double_literal_format::DoubleLiteralFormat),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "format-comment",
        source: RuleSource::DartCodeLinter("format-comment"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::format_comment::FormatComment),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "member-ordering",
        source: RuleSource::DartCodeLinter("member-ordering"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::member_ordering::MemberOrdering),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "newline-before-return",
        source: RuleSource::DartCodeLinter("newline-before-return"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::newline_before_return::NewlineBeforeReturn),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-magic-number",
        source: RuleSource::DartCodeLinter("no-magic-number"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::no_magic_number::NoMagicNumber),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-object-declaration",
        source: RuleSource::DartCodeLinter("no-object-declaration"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::no_object_declaration::NoObjectDeclaration),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-async-await",
        source: RuleSource::DartCodeLinter("prefer-async-await"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_async_await::PreferAsyncAwait),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-correct-identifier-length",
        source: RuleSource::DartCodeLinter("prefer-correct-identifier-length"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::style::prefer_correct_identifier_length::PreferCorrectIdentifierLength,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-correct-type-name",
        source: RuleSource::DartCodeLinter("prefer-correct-type-name"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_correct_type_name::PreferCorrectTypeName),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-first",
        source: RuleSource::DartCodeLinter("prefer-first"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_first::PreferFirst),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-iterable-of",
        source: RuleSource::DartCodeLinter("prefer-iterable-of"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_iterable_of::PreferIterableOf),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-last",
        source: RuleSource::DartCodeLinter("prefer-last"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_last::PreferLast),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-trailing-comma",
        source: RuleSource::DartCodeLinter("prefer-trailing-comma"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_trailing_comma::PreferTrailingComma),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-dedicated-media-query-methods",
        source: RuleSource::PyramidLint("prefer_dedicated_media_query_methods"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_dedicated_media_query_methods::PreferDedicatedMediaQueryMethods)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-underscore-for-unused-callback-parameters",
        source: RuleSource::PyramidLint("prefer_underscore_for_unused_callback_parameters"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_underscore_for_unused_callback_parameters::PreferUnderscoreForUnusedCallbackParameters)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-design-system-item",
        source: RuleSource::DartCodeLinter("use-design-system-item"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::use_design_system_item::UseDesignSystemItem),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-spacer-as-expanded-child",
        source: RuleSource::PyramidLint("use_spacer_as_expanded_child"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::use_spacer_as_expanded_child::UseSpacerAsExpandedChild),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-single-child-column-or-row",
        source: RuleSource::PyramidLint("avoid_single_child_column_or_row"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::avoid_single_child_column_or_row::AvoidSingleChildColumnOrRow)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-async-callback",
        source: RuleSource::PyramidLint("prefer_async_callback"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::style::prefer_async_callback::PreferAsyncCallback),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-redundant-pattern-field-names",
        source: RuleSource::PyramidLint("avoid_redundant_pattern_field_names"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::avoid_redundant_pattern_field_names::AvoidRedundantPatternFieldNames)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "proper-controller-dispose",
        source: RuleSource::PyramidLint("proper_controller_dispose"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::proper_controller_dispose::ProperControllerDispose)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "proper-expanded-and-flexible",
        source: RuleSource::PyramidLint("proper_expanded_and_flexible"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::proper_expanded_and_flexible::ProperExpandedAndFlexible)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "proper-from-environment",
        source: RuleSource::PyramidLint("proper_from_environment"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::proper_from_environment::ProperFromEnvironment),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "proper-super-init-state",
        source: RuleSource::PyramidLint("proper_super_init_state"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::proper_super_init_state::ProperSuperInitState),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "no-self-comparisons",
        source: RuleSource::PyramidLint("no_self_comparisons"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::no_self_comparisons::NoSelfComparisons),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-print",
        source: RuleSource::Lints("avoid_print"),
        group: RuleGroup::Suspicious,
        domains: FLUTTER,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::avoid_print::AvoidPrint),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-empty-else",
        source: RuleSource::Lints("avoid_empty_else"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::avoid_empty_else::AvoidEmptyElse),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-shadowing-type-parameters",
        source: RuleSource::Lints("avoid_shadowing_type_parameters"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::suspicious::avoid_shadowing_type_parameters::AvoidShadowingTypeParameters,
                )
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "empty-catches",
        source: RuleSource::Lints("empty_catches"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::empty_catches::EmptyCatches),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-wildcard-variable-uses",
        source: RuleSource::Lints("no_wildcard_variable_uses"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::no_wildcard_variable_uses::NoWildcardVariableUses),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-returning-null-for-void",
        source: RuleSource::Lints("avoid_returning_null_for_void"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::suspicious::avoid_returning_null_for_void::AvoidReturningNullForVoid)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "control-flow-in-finally",
        source: RuleSource::Lints("control_flow_in_finally"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::control_flow_in_finally::ControlFlowInFinally),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "empty-statements",
        source: RuleSource::Lints("empty_statements"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::empty_statements::EmptyStatements),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "recursive-getters",
        source: RuleSource::Lints("recursive_getters"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::recursive_getters::RecursiveGetters),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-null-aware-assignments",
        source: RuleSource::Lints("unnecessary_null_aware_assignments"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::suspicious::unnecessary_null_aware_assignments::UnnecessaryNullAwareAssignments)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "unnecessary-null-in-if-null-operators",
        source: RuleSource::Lints("unnecessary_null_in_if_null_operators"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::suspicious::unnecessary_null_in_if_null_operators::UnnecessaryNullInIfNullOperators)
            },
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "use-rethrow-when-possible",
        source: RuleSource::Lints("use_rethrow_when_possible"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::use_rethrow_when_possible::UseRethrowWhenPossible),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-dynamic",
        source: RuleSource::DartCodeLinter("avoid-dynamic"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::avoid_dynamic::AvoidDynamic),
            needs_resolution: false,
        },
    },
    RuleMeta {
        // In the recommended preset now that the type-resolution layer backs it:
        // the callee's declared return type decides it — a known `void` return is
        // safe to discard, a known non-void return is flagged, and only an
        // unknown return type falls back to the receiver-less side-effect
        // allowlist. This removes the false positives that kept it opt-in.
        name: "avoid-ignoring-return-values",
        source: RuleSource::DartCodeLinter("avoid-ignoring-return-values"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::suspicious::avoid_ignoring_return_values::AvoidIgnoringReturnValues)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-passing-async-when-sync-expected",
        source: RuleSource::DartCodeLinter("avoid-passing-async-when-sync-expected"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::suspicious::avoid_passing_async_when_sync_expected::AvoidPassingAsyncWhenSyncExpected)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-throw-in-catch-block",
        source: RuleSource::DartCodeLinter("avoid-throw-in-catch-block"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::avoid_throw_in_catch_block::AvoidThrowInCatchBlock),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "avoid-unrelated-type-assertions",
        source: RuleSource::DartCodeLinter("avoid-unrelated-type-assertions"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::suspicious::avoid_unrelated_type_assertions::AvoidUnrelatedTypeAssertions,
                )
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "no-empty-block",
        source: RuleSource::DartCodeLinter("no-empty-block"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::no_empty_block::NoEmptyBlock),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-equal-arguments",
        source: RuleSource::DartCodeLinter("no-equal-arguments"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::no_equal_arguments::NoEqualArguments),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-equal-then-else",
        source: RuleSource::DartCodeLinter("no-equal-then-else"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::no_equal_then_else::NoEqualThenElse),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "no-duplicate-case-values",
        source: RuleSource::PyramidLint("no_duplicate_case_values"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: true,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::no_duplicate_case_values::NoDuplicateCaseValues),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "unrelated-type-equality-checks",
        source: RuleSource::Lints("unrelated_type_equality_checks"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::correctness::unrelated_type_equality_checks::UnrelatedTypeEqualityChecks,
                )
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "collection-methods-unrelated-type",
        source: RuleSource::Lints("collection_methods_unrelated_type"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::collection_methods_unrelated_type::CollectionMethodsUnrelatedType)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "annotate-overrides",
        source: RuleSource::Lints("annotate_overrides"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::annotate_overrides::AnnotateOverrides),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-renaming-method-parameters",
        source: RuleSource::Lints("avoid_renaming_method_parameters"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::style::avoid_renaming_method_parameters::AvoidRenamingMethodParameters,
                )
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "overridden-fields",
        source: RuleSource::Lints("overridden_fields"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::suspicious::overridden_fields::OverriddenFields),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "use-build-context-synchronously",
        source: RuleSource::Lints("use_build_context_synchronously"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::use_build_context_synchronously::UseBuildContextSynchronously)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "use-key-in-widget-constructors",
        source: RuleSource::Lints("use_key_in_widget_constructors"),
        group: RuleGroup::Correctness,
        domains: FLUTTER,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::correctness::use_key_in_widget_constructors::UseKeyInWidgetConstructors,
                )
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-const-constructors-in-immutables",
        source: RuleSource::Lints("prefer_const_constructors_in_immutables"),
        group: RuleGroup::Style,
        domains: FLUTTER,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_const_constructors_in_immutables::PreferConstConstructorsInImmutables)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "await-only-futures",
        source: RuleSource::Lints("await_only_futures"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::await_only_futures::AwaitOnlyFutures),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "void-checks",
        source: RuleSource::Lints("void_checks"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::void_checks::VoidChecks),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "null-closures",
        source: RuleSource::Lints("null_closures"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::null_closures::NullClosures),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "null-check-on-nullable-type-parameter",
        source: RuleSource::Lints("null_check_on_nullable_type_parameter"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::null_check_on_nullable_type_parameter::NullCheckOnNullableTypeParameter)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "library-annotations",
        source: RuleSource::Lints("library_annotations"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::library_annotations::LibraryAnnotations),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "prefer-interpolation-to-compose-strings",
        source: RuleSource::Lints("prefer_interpolation_to_compose_strings"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::style::prefer_interpolation_to_compose_strings::PreferInterpolationToComposeStrings)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "type-literal-in-constant-pattern",
        source: RuleSource::Lints("type_literal_in_constant_pattern"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::type_literal_in_constant_pattern::TypeLiteralInConstantPattern)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "avoid-types-as-parameter-names",
        source: RuleSource::Lints("avoid_types_as_parameter_names"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(
                    lint::suspicious::avoid_types_as_parameter_names::AvoidTypesAsParameterNames,
                )
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "exhaustive-cases",
        source: RuleSource::Lints("exhaustive_cases"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::correctness::exhaustive_cases::ExhaustiveCases),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "implicit-call-tearoffs",
        source: RuleSource::Lints("implicit_call_tearoffs"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::implicit_call_tearoffs::ImplicitCallTearoffs),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "library-private-types-in-public-api",
        source: RuleSource::Lints("library_private_types_in_public_api"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::library_private_types_in_public_api::LibraryPrivateTypesInPublicApi)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "invalid-runtime-check-with-js-interop-types",
        source: RuleSource::Lints("invalid_runtime_check_with_js_interop_types"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || {
                Box::new(lint::correctness::invalid_runtime_check_with_js_interop_types::InvalidRuntimeCheckWithJsInteropTypes)
            },
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "file-names",
        source: RuleSource::Lints("file_names"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::style::file_names::FileNames),
            needs_resolution: false,
        },
    },
    RuleMeta {
        name: "prefer-contains",
        source: RuleSource::Lints("prefer_contains"),
        group: RuleGroup::Performance,
        domains: NONE,
        recommended: false,
        kind: RuleKind::File {
            make: || Box::new(lint::performance::prefer_contains::PreferContains),
            needs_resolution: true,
        },
    },
    RuleMeta {
        name: "depend-on-referenced-packages",
        source: RuleSource::Lints("depend_on_referenced_packages"),
        group: RuleGroup::Correctness,
        domains: NONE,
        recommended: false,
        kind: RuleKind::CrossFile(|| {
            Box::new(cross_file::depend_on_referenced_packages::DependOnReferencedPackages)
        }),
    },
    RuleMeta {
        name: "package-names",
        source: RuleSource::Lints("package_names"),
        group: RuleGroup::Style,
        domains: NONE,
        recommended: false,
        kind: RuleKind::CrossFile(|| Box::new(cross_file::package_names::PackageNames)),
    },
    RuleMeta {
        name: "secure-pubspec-urls",
        source: RuleSource::Lints("secure_pubspec_urls"),
        group: RuleGroup::Suspicious,
        domains: NONE,
        recommended: false,
        kind: RuleKind::CrossFile(|| Box::new(cross_file::secure_pubspec_urls::SecurePubspecUrls)),
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
/// `(canonical_name, group, is_cross_file)` so `falcon_analyze` can validate a
/// `// falcon-ignore` path — and record the *canonical* id, so a suppression
/// written with an old id still matches the diagnostic's canonical rule — without
/// depending on this crate. Matches [`falcon_analyze::RuleLookup`].
pub fn suppression_lookup(name: &str) -> Option<(&'static str, &'static str, bool)> {
    meta_for(name).map(|m| (m.name, m.group.as_str(), m.is_cross_file()))
}

/// Rewrite legacy rule ids used as config keys to their canonical ids, across
/// the base `linter`/`cross_file` rule maps and every override. Applied once
/// after a config is loaded so old `falcon.json` files keep resolving against the
/// canonical rule table. When a legacy id and its canonical id (or two legacy
/// twins) collide in the same group, the more severe level is kept — matching
/// the `falcon migrate` upgrade path.
pub fn canonicalize_config(config: &mut FalconConfig) {
    canonicalize_rules(&mut config.linter.rules);
    canonicalize_rules(&mut config.cross_file.rules);
    for ov in &mut config.overrides {
        if let Some(linter) = &mut ov.linter {
            canonicalize_rules(&mut linter.rules);
        }
        if let Some(cross_file) = &mut ov.cross_file {
            canonicalize_rules(&mut cross_file.rules);
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
