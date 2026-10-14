//! The rule table supplies constructors, execution identities, and configuration metadata.

use falcon_analyze::{CrossFileRule, Rule};
use falcon_config::{FalconConfig, RuleConfiguration, RulePlainConfiguration, Rules};

use crate::{cross_file, lint};

macro_rules! metadata_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

metadata_enum!(RuleGroup {
    Complexity => "complexity",
    Correctness => "correctness",
    Performance => "performance",
    Style => "style",
    Suspicious => "suspicious",
});

metadata_enum!(Domain {
    Flutter => "flutter",
});

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

/// A rule's execution identity, constructor, and configuration metadata.
pub struct RuleMeta {
    pub name: &'static str,
    pub group: RuleGroup,
    pub domain: Option<Domain>,
    pub recommended: bool,
    pub kind: RuleKind,
    /// Upstream provenance (biome-style): which linter the rule was ported from,
    /// or `Falcon` for original rules.
    pub source: RuleSource,
}

impl RuleMeta {
    const fn file<T: Rule + Default + 'static>(
        name: &'static str,
        group: RuleGroup,
        domain: Option<Domain>,
        recommended: bool,
        source: RuleSource,
    ) -> Self {
        Self {
            name,
            group,
            domain,
            recommended,
            source,
            kind: RuleKind::File {
                make: make_rule::<T>,
                needs_resolution: false,
            },
        }
    }

    const fn resolved<T: Rule + Default + 'static>(
        name: &'static str,
        group: RuleGroup,
        domain: Option<Domain>,
        recommended: bool,
        source: RuleSource,
    ) -> Self {
        let mut meta = Self::file::<T>(name, group, domain, recommended, source);
        meta.kind = RuleKind::File {
            make: make_rule::<T>,
            needs_resolution: true,
        };
        meta
    }

    const fn cross_file<T: CrossFileRule + Default + 'static>(
        name: &'static str,
        group: RuleGroup,
        domain: Option<Domain>,
        recommended: bool,
        source: RuleSource,
    ) -> Self {
        Self {
            name,
            group,
            domain,
            recommended,
            source,
            kind: RuleKind::CrossFile(make_cross_file_rule::<T>),
        }
    }

    pub fn domain_names(&self) -> &'static [&'static str] {
        const FLUTTER_NAMES: &[&str] = &[Domain::Flutter.as_str()];
        match self.domain {
            None => &[],
            Some(Domain::Flutter) => FLUTTER_NAMES,
        }
    }

    pub const fn is_cross_file(&self) -> bool {
        matches!(self.kind, RuleKind::CrossFile(_))
    }
}

fn make_rule<T: Rule + Default + 'static>() -> Box<dyn Rule> {
    Box::new(T::default())
}

fn make_cross_file_rule<T: CrossFileRule + Default + 'static>() -> Box<dyn CrossFileRule> {
    Box::new(T::default())
}

const FLUTTER: Option<Domain> = Some(Domain::Flutter);
const NONE: Option<Domain> = None;

/// The sole constructor table for registered rules.
pub const RULE_METADATA: &[RuleMeta] = &[
    // ── complexity ──────────────────────────────────────────────────────────
    RuleMeta::file::<lint::complexity::avoid_unnecessary_containers::AvoidUnnecessaryContainers>(
        "avoid-unnecessary-containers",
        RuleGroup::Complexity,
        FLUTTER,
        true,
        RuleSource::Lints("avoid_unnecessary_containers"),
    ),
    RuleMeta::file::<lint::complexity::unnecessary_overrides::UnnecessaryOverrides>(
        "unnecessary-overrides",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::Lints("unnecessary_overrides"),
    ),
    RuleMeta::file::<lint::complexity::avoid_function_literals_in_foreach_calls::AvoidFunctionLiteralsInForeachCalls>(
        "avoid-function-literals-in-foreach-calls",
        RuleGroup::Complexity,
        NONE,
        false,
        RuleSource::Lints("avoid_function_literals_in_foreach_calls"),
    ),
    RuleMeta::file::<lint::complexity::prefer_conditional_assignment::PreferConditionalAssignment>(
        "prefer-conditional-assignment",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::Lints("prefer_conditional_assignment"),
    ),
    RuleMeta::file::<lint::complexity::prefer_for_elements_to_map_from_iterable::PreferForElementsToMapFromIterable>(
        "prefer-for-elements-to-map-from-iterable",
        RuleGroup::Complexity,
        NONE,
        false,
        RuleSource::Lints("prefer_for_elements_to_map_fromIterable"),
    ),
    RuleMeta::file::<lint::complexity::prefer_if_null_operators::PreferIfNullOperators>(
        "prefer-if-null-operators",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::Lints("prefer_if_null_operators"),
    ),
    RuleMeta::file::<lint::complexity::prefer_null_aware_operators::PreferNullAwareOperators>(
        "prefer-null-aware-operators",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::Lints("prefer_null_aware_operators"),
    ),
    RuleMeta::file::<lint::complexity::unnecessary_const::UnnecessaryConst>(
        "unnecessary-const",
        RuleGroup::Complexity,
        NONE,
        false,
        RuleSource::Lints("unnecessary_const"),
    ),
    RuleMeta::file::<lint::complexity::unnecessary_getters_setters::UnnecessaryGettersSetters>(
        "unnecessary-getters-setters",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::Lints("unnecessary_getters_setters"),
    ),
    RuleMeta::file::<lint::complexity::avoid_nested_conditional_expressions::AvoidNestedConditionalExpressions>(
        "avoid-nested-conditional-expressions",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-nested-conditional-expressions"),
    ),
    RuleMeta::file::<lint::complexity::avoid_redundant_async::AvoidRedundantAsync>(
        "avoid-redundant-async",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-redundant-async"),
    ),
    RuleMeta::resolved::<lint::complexity::avoid_unnecessary_type_assertions::AvoidUnnecessaryTypeAssertions>(
        "avoid-unnecessary-type-assertions",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-unnecessary-type-assertions"),
    ),
    RuleMeta::resolved::<lint::complexity::avoid_unnecessary_type_casts::AvoidUnnecessaryTypeCasts>(
        "avoid-unnecessary-type-casts",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-unnecessary-type-casts"),
    ),
    RuleMeta::file::<lint::complexity::avoid_inverted_boolean_expressions::AvoidInvertedBooleanExpressions>(
        "avoid-inverted-boolean-expressions",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("avoid_inverted_boolean_expressions"),
    ),
    RuleMeta::file::<lint::complexity::avoid_nested_if::AvoidNestedIf>(
        "avoid-nested-if",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("avoid_nested_if"),
    ),
    RuleMeta::file::<lint::complexity::max_lines_for_file::MaxLinesForFile>(
        "max-lines-for-file",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("max_lines_for_file"),
    ),
    RuleMeta::file::<lint::complexity::max_lines_for_function::MaxLinesForFunction>(
        "max-lines-for-function",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("max_lines_for_function"),
    ),
    RuleMeta::file::<lint::complexity::max_parameters_for_function::MaxParametersForFunction>(
        "max-parameters-for-function",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("max_parameters_for_function"),
    ),
    RuleMeta::file::<lint::complexity::max_switch_cases::MaxSwitchCases>(
        "max-switch-cases",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("max_switch_cases"),
    ),
    RuleMeta::file::<lint::complexity::cyclomatic_complexity::CyclomaticComplexity>(
        "cyclomatic-complexity",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("cyclomatic_complexity"),
    ),
    RuleMeta::file::<lint::complexity::maximum_nesting_level::MaximumNestingLevel>(
        "maximum-nesting-level",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("maximum_nesting_level"),
    ),
    RuleMeta::resolved::<lint::complexity::no_boolean_literal_compare::NoBooleanLiteralCompare>(
        "no-boolean-literal-compare",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("no-boolean-literal-compare"),
    ),
    RuleMeta::file::<lint::complexity::prefer_conditional_expressions::PreferConditionalExpressions>(
        "prefer-conditional-expressions",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-conditional-expressions"),
    ),
    RuleMeta::resolved::<lint::complexity::prefer_extracting_callbacks::PreferExtractingCallbacks>(
        "prefer-extracting-callbacks",
        RuleGroup::Complexity,
        FLUTTER,
        true,
        RuleSource::DartCodeLinter("prefer-extracting-callbacks"),
    ),
    RuleMeta::file::<lint::complexity::prefer_immediate_return::PreferImmediateReturn>(
        "prefer-immediate-return",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-immediate-return"),
    ),
    RuleMeta::file::<lint::complexity::prefer_moving_to_variable::PreferMovingToVariable>(
        "prefer-moving-to-variable",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-moving-to-variable"),
    ),
    RuleMeta::file::<lint::complexity::prefer_iterable_any::PreferIterableAny>(
        "prefer-iterable-any",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("prefer_iterable_any"),
    ),
    RuleMeta::file::<lint::complexity::prefer_iterable_every::PreferIterableEvery>(
        "prefer-iterable-every",
        RuleGroup::Complexity,
        NONE,
        true,
        RuleSource::PyramidLint("prefer_iterable_every"),
    ),
    RuleMeta::file::<lint::correctness::avoid_web_libraries_in_flutter::AvoidWebLibrariesInFlutter>(
        "avoid-web-libraries-in-flutter",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::Lints("avoid_web_libraries_in_flutter"),
    ),
    RuleMeta::resolved::<lint::correctness::no_logic_in_create_state::NoLogicInCreateState>(
        "no-logic-in-create-state",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::Lints("no_logic_in_create_state"),
    ),
    RuleMeta::file::<lint::correctness::avoid_relative_lib_imports::AvoidRelativeLibImports>(
        "avoid-relative-lib-imports",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::Lints("avoid_relative_lib_imports"),
    ),
    RuleMeta::file::<lint::correctness::hash_and_equals::HashAndEquals>(
        "hash-and-equals",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::Lints("hash_and_equals"),
    ),
    RuleMeta::resolved::<lint::correctness::valid_regexps::ValidRegexps>(
        "valid-regexps",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::Lints("valid_regexps"),
    ),
    RuleMeta::resolved::<lint::correctness::implementation_imports::ImplementationImports>(
        "implementation-imports",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::Lints("implementation_imports"),
    ),
    RuleMeta::file::<lint::correctness::avoid_global_state::AvoidGlobalState>(
        "avoid-global-state",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-global-state"),
    ),
    RuleMeta::file::<lint::correctness::avoid_returning_widgets::AvoidReturningWidgets>(
        "avoid-returning-widgets",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::DartCodeLinter("avoid-returning-widgets"),
    ),
    RuleMeta::file::<lint::correctness::avoid_unused_parameters::AvoidUnusedParameters>(
        "avoid-unused-parameters",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-unused-parameters"),
    ),
    RuleMeta::file::<lint::correctness::avoid_mutable_global_variables::AvoidMutableGlobalVariables>(
        "avoid-mutable-global-variables",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::PyramidLint("avoid_mutable_global_variables"),
    ),
    RuleMeta::file::<lint::correctness::correct_order_for_super_dispose::CorrectOrderForSuperDispose>(
        "correct-order-for-super-dispose",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::PyramidLint("correct_order_for_super_dispose"),
    ),
    RuleMeta::resolved::<lint::correctness::unnecessary_flutter_imports::UnnecessaryFlutterImports>(
        "unnecessary-flutter-imports",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::Falcon,
    ),
    RuleMeta::cross_file::<cross_file::unused_files::UnusedFiles>(
        "unused-files",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::DartCodeLinter("check-unused-files"),
    ),
    RuleMeta::cross_file::<cross_file::unused_code::UnusedCode>(
        "unused-code",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::DartCodeLinter("check-unused-code"),
    ),
    RuleMeta::cross_file::<cross_file::unnecessary_nullable::UnnecessaryNullable>(
        "unnecessary-nullable",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::DartCodeLinter("check-unnecessary-nullable"),
    ),
    RuleMeta::file::<lint::correctness::unnecessary_nullable_return_type::UnnecessaryNullableReturnType>(
        "unnecessary-nullable-return-type",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::PyramidLint("unnecessary_nullable_return_type"),
    ),
    RuleMeta::file::<lint::correctness::use_once_constructors_once_provider::UseOnceConstructorsOnceProvider>(
        "use-once-constructors-once-provider",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::PyramidLint("use_once_constructors_once_provider"),
    ),
    RuleMeta::file::<lint::performance::sized_box_for_whitespace::SizedBoxForWhitespace>(
        "sized-box-for-whitespace",
        RuleGroup::Performance,
        FLUTTER,
        true,
        RuleSource::Lints("sized_box_for_whitespace"),
    ),
    RuleMeta::file::<lint::performance::unnecessary_to_list_in_spreads::UnnecessaryToListInSpreads>(
        "unnecessary-to-list-in-spreads",
        RuleGroup::Performance,
        NONE,
        true,
        RuleSource::Lints("unnecessary_to_list_in_spreads"),
    ),
    RuleMeta::file::<lint::performance::prefer_const_border_radius::PreferConstBorderRadius>(
        "prefer-const-border-radius",
        RuleGroup::Performance,
        FLUTTER,
        true,
        RuleSource::DartCodeLinter("prefer-const-border-radius"),
    ),
    RuleMeta::file::<lint::performance::prefer_correct_edge_insets_constructor::PreferCorrectEdgeInsetsConstructor>(
        "prefer-correct-edge-insets-constructor",
        RuleGroup::Performance,
        FLUTTER,
        true,
        RuleSource::DartCodeLinter("prefer-correct-edge-insets-constructor"),
    ),
    RuleMeta::resolved::<lint::performance::prefer_declaring_const_constructor::PreferDeclaringConstConstructor>(
        "prefer-declaring-const-constructor",
        RuleGroup::Performance,
        FLUTTER,
        true,
        RuleSource::PyramidLint("prefer_declaring_const_constructor"),
    ),
    RuleMeta::file::<lint::style::sort_child_properties_last::SortChildPropertiesLast>(
        "sort-child-properties-last",
        RuleGroup::Style,
        FLUTTER,
        true,
        RuleSource::Lints("sort_child_properties_last"),
    ),
    RuleMeta::resolved::<lint::style::use_full_hex_values_for_flutter_colors::UseFullHexValuesForFlutterColors>(
        "use-full-hex-values-for-flutter-colors",
        RuleGroup::Style,
        FLUTTER,
        true,
        RuleSource::Lints("use_full_hex_values_for_flutter_colors"),
    ),
    RuleMeta::file::<lint::style::camel_case_extensions::CamelCaseExtensions>(
        "camel-case-extensions",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("camel_case_extensions"),
    ),
    RuleMeta::file::<lint::style::camel_case_types::CamelCaseTypes>(
        "camel-case-types",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("camel_case_types"),
    ),
    RuleMeta::file::<lint::style::curly_braces_in_flow_control_structures::CurlyBracesInFlowControlStructures>(
        "curly-braces-in-flow-control-structures",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("curly_braces_in_flow_control_structures"),
    ),
    RuleMeta::file::<lint::style::dangling_library_doc_comments::DanglingLibraryDocComments>(
        "dangling-library-doc-comments",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("dangling_library_doc_comments"),
    ),
    RuleMeta::file::<lint::style::non_constant_identifier_names::NonConstantIdentifierNames>(
        "non-constant-identifier-names",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("non_constant_identifier_names"),
    ),
    RuleMeta::file::<lint::style::prefer_generic_function_type_aliases::PreferGenericFunctionTypeAliases>(
        "prefer-generic-function-type-aliases",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_generic_function_type_aliases"),
    ),
    RuleMeta::resolved::<lint::style::prefer_is_empty::PreferIsEmpty>(
        "prefer-is-empty",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("prefer_is_empty"),
    ),
    RuleMeta::resolved::<lint::style::prefer_is_not_empty::PreferIsNotEmpty>(
        "prefer-is-not-empty",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("prefer_is_not_empty"),
    ),
    RuleMeta::resolved::<lint::style::prefer_iterable_where_type::PreferIterableWhereType>(
        "prefer-iterable-where-type",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_iterable_whereType"),
    ),
    RuleMeta::file::<lint::style::prefer_typing_uninitialized_variables::PreferTypingUninitializedVariables>(
        "prefer-typing-uninitialized-variables",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_typing_uninitialized_variables"),
    ),
    RuleMeta::file::<lint::style::provide_deprecation_message::ProvideDeprecationMessage>(
        "provide-deprecation-message",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("provide_deprecation_message"),
    ),
    RuleMeta::file::<lint::style::unintended_html_in_doc_comment::UnintendedHtmlInDocComment>(
        "unintended-html-in-doc-comment",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unintended_html_in_doc_comment"),
    ),
    RuleMeta::file::<lint::style::use_string_in_part_of_directives::UseStringInPartOfDirectives>(
        "use-string-in-part-of-directives",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("use_string_in_part_of_directives"),
    ),
    RuleMeta::file::<lint::style::avoid_init_to_null::AvoidInitToNull>(
        "avoid-init-to-null",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("avoid_init_to_null"),
    ),
    RuleMeta::file::<lint::style::avoid_return_types_on_setters::AvoidReturnTypesOnSetters>(
        "avoid-return-types-on-setters",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("avoid_return_types_on_setters"),
    ),
    RuleMeta::file::<lint::style::avoid_single_cascade_in_expression_statements::AvoidSingleCascadeInExpressionStatements>(
        "avoid-single-cascade-in-expression-statements",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("avoid_single_cascade_in_expression_statements"),
    ),
    RuleMeta::file::<lint::style::constant_identifier_names::ConstantIdentifierNames>(
        "constant-identifier-names",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("constant_identifier_names"),
    ),
    RuleMeta::file::<lint::style::empty_constructor_bodies::EmptyConstructorBodies>(
        "empty-constructor-bodies",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("empty_constructor_bodies"),
    ),
    RuleMeta::file::<lint::style::library_prefixes::LibraryPrefixes>(
        "library-prefixes",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("library_prefixes"),
    ),
    RuleMeta::file::<lint::style::no_leading_underscores_for_library_prefixes::NoLeadingUnderscoresForLibraryPrefixes>(
        "no-leading-underscores-for-library-prefixes",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("no_leading_underscores_for_library_prefixes"),
    ),
    RuleMeta::file::<lint::style::no_leading_underscores_for_local_identifiers::NoLeadingUnderscoresForLocalIdentifiers>(
        "no-leading-underscores-for-local-identifiers",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("no_leading_underscores_for_local_identifiers"),
    ),
    RuleMeta::file::<lint::style::prefer_adjacent_string_concatenation::PreferAdjacentStringConcatenation>(
        "prefer-adjacent-string-concatenation",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_adjacent_string_concatenation"),
    ),
    RuleMeta::resolved::<lint::style::prefer_collection_literals::PreferCollectionLiterals>(
        "prefer-collection-literals",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("prefer_collection_literals"),
    ),
    RuleMeta::resolved::<lint::style::prefer_final_fields::PreferFinalFields>(
        "prefer-final-fields",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_final_fields"),
    ),
    RuleMeta::file::<lint::style::prefer_function_declarations_over_variables::PreferFunctionDeclarationsOverVariables>(
        "prefer-function-declarations-over-variables",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_function_declarations_over_variables"),
    ),
    RuleMeta::file::<lint::style::prefer_initializing_formals::PreferInitializingFormals>(
        "prefer-initializing-formals",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_initializing_formals"),
    ),
    RuleMeta::file::<lint::style::prefer_inlined_adds::PreferInlinedAdds>(
        "prefer-inlined-adds",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_inlined_adds"),
    ),
    RuleMeta::file::<lint::style::prefer_is_not_operator::PreferIsNotOperator>(
        "prefer-is-not-operator",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_is_not_operator"),
    ),
    RuleMeta::file::<lint::style::prefer_spread_collections::PreferSpreadCollections>(
        "prefer-spread-collections",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("prefer_spread_collections"),
    ),
    RuleMeta::file::<lint::style::slash_for_doc_comments::SlashForDocComments>(
        "slash-for-doc-comments",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("slash_for_doc_comments"),
    ),
    RuleMeta::file::<lint::style::type_init_formals::TypeInitFormals>(
        "type-init-formals",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("type_init_formals"),
    ),
    RuleMeta::file::<lint::style::unnecessary_brace_in_string_interps::UnnecessaryBraceInStringInterps>(
        "unnecessary-brace-in-string-interps",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_brace_in_string_interps"),
    ),
    RuleMeta::file::<lint::style::unnecessary_constructor_name::UnnecessaryConstructorName>(
        "unnecessary-constructor-name",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_constructor_name"),
    ),
    RuleMeta::file::<lint::style::unnecessary_late::UnnecessaryLate>(
        "unnecessary-late",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_late"),
    ),
    RuleMeta::file::<lint::style::unnecessary_library_name::UnnecessaryLibraryName>(
        "unnecessary-library-name",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_library_name"),
    ),
    RuleMeta::file::<lint::style::unnecessary_new::UnnecessaryNew>(
        "unnecessary-new",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_new"),
    ),
    RuleMeta::file::<lint::style::unnecessary_nullable_for_final_variable_declarations::UnnecessaryNullableForFinalVariableDeclarations>(
        "unnecessary-nullable-for-final-variable-declarations",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("unnecessary_nullable_for_final_variable_declarations"),
    ),
    RuleMeta::file::<lint::style::unnecessary_string_escapes::UnnecessaryStringEscapes>(
        "unnecessary-string-escapes",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_string_escapes"),
    ),
    RuleMeta::resolved::<lint::style::unnecessary_string_interpolations::UnnecessaryStringInterpolations>(
        "unnecessary-string-interpolations",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_string_interpolations"),
    ),
    RuleMeta::file::<lint::style::unnecessary_this::UnnecessaryThis>(
        "unnecessary-this",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("unnecessary_this"),
    ),
    RuleMeta::file::<lint::style::use_function_type_syntax_for_parameters::UseFunctionTypeSyntaxForParameters>(
        "use-function-type-syntax-for-parameters",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Lints("use_function_type_syntax_for_parameters"),
    ),
    RuleMeta::file::<lint::style::use_super_parameters::UseSuperParameters>(
        "use-super-parameters",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("use_super_parameters"),
    ),
    RuleMeta::file::<lint::style::avoid_late_keyword::AvoidLateKeyword>(
        "avoid-late-keyword",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-late-keyword"),
    ),
    RuleMeta::resolved::<lint::style::avoid_non_null_assertion::AvoidNonNullAssertion>(
        "avoid-non-null-assertion",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-non-null-assertion"),
    ),
    RuleMeta::file::<lint::style::avoid_top_level_member_access::AvoidTopLevelMemberAccess>(
        "avoid-top-level-member-access",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::Falcon,
    ),
    RuleMeta::file::<lint::style::avoid_abbreviations_in_doc_comments::AvoidAbbreviationsInDocComments>(
        "avoid-abbreviations-in-doc-comments",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::PyramidLint("avoid_abbreviations_in_doc_comments"),
    ),
    RuleMeta::file::<lint::style::avoid_positional_fields_in_records::AvoidPositionalFieldsInRecords>(
        "avoid-positional-fields-in-records",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::PyramidLint("avoid_positional_fields_in_records"),
    ),
    RuleMeta::file::<lint::style::binary_expression_operand_order::BinaryExpressionOperandOrder>(
        "binary-expression-operand-order",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("binary-expression-operand-order"),
    ),
    RuleMeta::file::<lint::style::boolean_prefixes::BooleanPrefixes>(
        "boolean-prefixes",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::PyramidLint("boolean_prefixes"),
    ),
    RuleMeta::file::<lint::style::class_members_ordering::ClassMembersOrdering>(
        "class-members-ordering",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::PyramidLint("class_members_ordering"),
    ),
    RuleMeta::file::<lint::style::double_literal_format::DoubleLiteralFormat>(
        "double-literal-format",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("double-literal-format"),
    ),
    RuleMeta::file::<lint::style::format_comment::FormatComment>(
        "format-comment",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("format-comment"),
    ),
    RuleMeta::file::<lint::style::member_ordering::MemberOrdering>(
        "member-ordering",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("member-ordering"),
    ),
    RuleMeta::file::<lint::style::newline_before_return::NewlineBeforeReturn>(
        "newline-before-return",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("newline-before-return"),
    ),
    RuleMeta::file::<lint::style::no_magic_number::NoMagicNumber>(
        "no-magic-number",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("no-magic-number"),
    ),
    RuleMeta::file::<lint::style::no_object_declaration::NoObjectDeclaration>(
        "no-object-declaration",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("no-object-declaration"),
    ),
    RuleMeta::file::<lint::style::prefer_async_await::PreferAsyncAwait>(
        "prefer-async-await",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-async-await"),
    ),
    RuleMeta::file::<lint::style::prefer_correct_identifier_length::PreferCorrectIdentifierLength>(
        "prefer-correct-identifier-length",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-correct-identifier-length"),
    ),
    RuleMeta::file::<lint::style::prefer_correct_type_name::PreferCorrectTypeName>(
        "prefer-correct-type-name",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-correct-type-name"),
    ),
    RuleMeta::file::<lint::style::prefer_first::PreferFirst>(
        "prefer-first",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-first"),
    ),
    RuleMeta::file::<lint::style::prefer_iterable_of::PreferIterableOf>(
        "prefer-iterable-of",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-iterable-of"),
    ),
    RuleMeta::file::<lint::style::prefer_last::PreferLast>(
        "prefer-last",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-last"),
    ),
    RuleMeta::file::<lint::style::prefer_trailing_comma::PreferTrailingComma>(
        "prefer-trailing-comma",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::DartCodeLinter("prefer-trailing-comma"),
    ),
    RuleMeta::file::<lint::style::prefer_dedicated_media_query_methods::PreferDedicatedMediaQueryMethods>(
        "prefer-dedicated-media-query-methods",
        RuleGroup::Style,
        FLUTTER,
        true,
        RuleSource::PyramidLint("prefer_dedicated_media_query_methods"),
    ),
    RuleMeta::file::<lint::style::prefer_underscore_for_unused_callback_parameters::PreferUnderscoreForUnusedCallbackParameters>(
        "prefer-underscore-for-unused-callback-parameters",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::PyramidLint("prefer_underscore_for_unused_callback_parameters"),
    ),
    RuleMeta::file::<lint::style::use_design_system_item::UseDesignSystemItem>(
        "use-design-system-item",
        RuleGroup::Style,
        FLUTTER,
        true,
        RuleSource::DartCodeLinter("use-design-system-item"),
    ),
    RuleMeta::file::<lint::style::use_spacer_as_expanded_child::UseSpacerAsExpandedChild>(
        "use-spacer-as-expanded-child",
        RuleGroup::Style,
        FLUTTER,
        true,
        RuleSource::PyramidLint("use_spacer_as_expanded_child"),
    ),
    RuleMeta::file::<lint::style::avoid_single_child_column_or_row::AvoidSingleChildColumnOrRow>(
        "avoid-single-child-column-or-row",
        RuleGroup::Style,
        FLUTTER,
        true,
        RuleSource::PyramidLint("avoid_single_child_column_or_row"),
    ),
    RuleMeta::file::<lint::style::prefer_async_callback::PreferAsyncCallback>(
        "prefer-async-callback",
        RuleGroup::Style,
        FLUTTER,
        true,
        RuleSource::PyramidLint("prefer_async_callback"),
    ),
    RuleMeta::file::<lint::style::avoid_redundant_pattern_field_names::AvoidRedundantPatternFieldNames>(
        "avoid-redundant-pattern-field-names",
        RuleGroup::Style,
        NONE,
        true,
        RuleSource::PyramidLint("avoid_redundant_pattern_field_names"),
    ),
    RuleMeta::resolved::<lint::correctness::proper_controller_dispose::ProperControllerDispose>(
        "proper-controller-dispose",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::PyramidLint("proper_controller_dispose"),
    ),
    RuleMeta::file::<lint::correctness::proper_expanded_and_flexible::ProperExpandedAndFlexible>(
        "proper-expanded-and-flexible",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::PyramidLint("proper_expanded_and_flexible"),
    ),
    RuleMeta::file::<lint::correctness::proper_from_environment::ProperFromEnvironment>(
        "proper-from-environment",
        RuleGroup::Correctness,
        NONE,
        true,
        RuleSource::PyramidLint("proper_from_environment"),
    ),
    RuleMeta::resolved::<lint::correctness::proper_super_init_state::ProperSuperInitState>(
        "proper-super-init-state",
        RuleGroup::Correctness,
        FLUTTER,
        true,
        RuleSource::PyramidLint("proper_super_init_state"),
    ),
    RuleMeta::resolved::<lint::suspicious::no_self_comparisons::NoSelfComparisons>(
        "no-self-comparisons",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::PyramidLint("no_self_comparisons"),
    ),
    RuleMeta::resolved::<lint::suspicious::avoid_print::AvoidPrint>(
        "avoid-print",
        RuleGroup::Suspicious,
        FLUTTER,
        true,
        RuleSource::Lints("avoid_print"),
    ),
    RuleMeta::file::<lint::suspicious::avoid_empty_else::AvoidEmptyElse>(
        "avoid-empty-else",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("avoid_empty_else"),
    ),
    RuleMeta::file::<lint::suspicious::avoid_shadowing_type_parameters::AvoidShadowingTypeParameters>(
        "avoid-shadowing-type-parameters",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("avoid_shadowing_type_parameters"),
    ),
    RuleMeta::file::<lint::suspicious::empty_catches::EmptyCatches>(
        "empty-catches",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("empty_catches"),
    ),
    RuleMeta::file::<lint::suspicious::no_wildcard_variable_uses::NoWildcardVariableUses>(
        "no-wildcard-variable-uses",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("no_wildcard_variable_uses"),
    ),
    RuleMeta::file::<lint::suspicious::avoid_returning_null_for_void::AvoidReturningNullForVoid>(
        "avoid-returning-null-for-void",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("avoid_returning_null_for_void"),
    ),
    RuleMeta::file::<lint::suspicious::control_flow_in_finally::ControlFlowInFinally>(
        "control-flow-in-finally",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("control_flow_in_finally"),
    ),
    RuleMeta::file::<lint::suspicious::empty_statements::EmptyStatements>(
        "empty-statements",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("empty_statements"),
    ),
    RuleMeta::file::<lint::suspicious::recursive_getters::RecursiveGetters>(
        "recursive-getters",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("recursive_getters"),
    ),
    RuleMeta::file::<lint::suspicious::unnecessary_null_aware_assignments::UnnecessaryNullAwareAssignments>(
        "unnecessary-null-aware-assignments",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("unnecessary_null_aware_assignments"),
    ),
    RuleMeta::file::<lint::suspicious::unnecessary_null_in_if_null_operators::UnnecessaryNullInIfNullOperators>(
        "unnecessary-null-in-if-null-operators",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("unnecessary_null_in_if_null_operators"),
    ),
    RuleMeta::file::<lint::suspicious::use_rethrow_when_possible::UseRethrowWhenPossible>(
        "use-rethrow-when-possible",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::Lints("use_rethrow_when_possible"),
    ),
    RuleMeta::file::<lint::suspicious::avoid_dynamic::AvoidDynamic>(
        "avoid-dynamic",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-dynamic"),
    ),
    RuleMeta::resolved::<lint::suspicious::avoid_ignoring_return_values::AvoidIgnoringReturnValues>(
        "avoid-ignoring-return-values",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-ignoring-return-values"),
    ),
    RuleMeta::resolved::<lint::suspicious::avoid_passing_async_when_sync_expected::AvoidPassingAsyncWhenSyncExpected>(
        "avoid-passing-async-when-sync-expected",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-passing-async-when-sync-expected"),
    ),
    RuleMeta::file::<lint::suspicious::avoid_throw_in_catch_block::AvoidThrowInCatchBlock>(
        "avoid-throw-in-catch-block",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-throw-in-catch-block"),
    ),
    RuleMeta::resolved::<lint::suspicious::avoid_unrelated_type_assertions::AvoidUnrelatedTypeAssertions>(
        "avoid-unrelated-type-assertions",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("avoid-unrelated-type-assertions"),
    ),
    RuleMeta::file::<lint::suspicious::no_empty_block::NoEmptyBlock>(
        "no-empty-block",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("no-empty-block"),
    ),
    RuleMeta::file::<lint::suspicious::no_equal_arguments::NoEqualArguments>(
        "no-equal-arguments",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("no-equal-arguments"),
    ),
    RuleMeta::file::<lint::suspicious::no_equal_then_else::NoEqualThenElse>(
        "no-equal-then-else",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::DartCodeLinter("no-equal-then-else"),
    ),
    RuleMeta::resolved::<lint::suspicious::no_duplicate_case_values::NoDuplicateCaseValues>(
        "no-duplicate-case-values",
        RuleGroup::Suspicious,
        NONE,
        true,
        RuleSource::PyramidLint("no_duplicate_case_values"),
    ),
    RuleMeta::resolved::<lint::correctness::unrelated_type_equality_checks::UnrelatedTypeEqualityChecks>(
        "unrelated-type-equality-checks",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("unrelated_type_equality_checks"),
    ),
    RuleMeta::resolved::<lint::correctness::collection_methods_unrelated_type::CollectionMethodsUnrelatedType>(
        "collection-methods-unrelated-type",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("collection_methods_unrelated_type"),
    ),
    RuleMeta::resolved::<lint::style::annotate_overrides::AnnotateOverrides>(
        "annotate-overrides",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("annotate_overrides"),
    ),
    RuleMeta::resolved::<lint::style::avoid_renaming_method_parameters::AvoidRenamingMethodParameters>(
        "avoid-renaming-method-parameters",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("avoid_renaming_method_parameters"),
    ),
    RuleMeta::resolved::<lint::suspicious::overridden_fields::OverriddenFields>(
        "overridden-fields",
        RuleGroup::Suspicious,
        NONE,
        false,
        RuleSource::Lints("overridden_fields"),
    ),
    RuleMeta::resolved::<lint::correctness::use_build_context_synchronously::UseBuildContextSynchronously>(
        "use-build-context-synchronously",
        RuleGroup::Correctness,
        FLUTTER,
        false,
        RuleSource::Lints("use_build_context_synchronously"),
    ),
    RuleMeta::resolved::<lint::correctness::use_key_in_widget_constructors::UseKeyInWidgetConstructors>(
        "use-key-in-widget-constructors",
        RuleGroup::Correctness,
        FLUTTER,
        false,
        RuleSource::Lints("use_key_in_widget_constructors"),
    ),
    RuleMeta::resolved::<lint::style::prefer_const_constructors_in_immutables::PreferConstConstructorsInImmutables>(
        "prefer-const-constructors-in-immutables",
        RuleGroup::Style,
        FLUTTER,
        false,
        RuleSource::Lints("prefer_const_constructors_in_immutables"),
    ),
    RuleMeta::resolved::<lint::correctness::await_only_futures::AwaitOnlyFutures>(
        "await-only-futures",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("await_only_futures"),
    ),
    RuleMeta::resolved::<lint::correctness::void_checks::VoidChecks>(
        "void-checks",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("void_checks"),
    ),
    RuleMeta::resolved::<lint::correctness::null_closures::NullClosures>(
        "null-closures",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("null_closures"),
    ),
    RuleMeta::resolved::<lint::correctness::null_check_on_nullable_type_parameter::NullCheckOnNullableTypeParameter>(
        "null-check-on-nullable-type-parameter",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("null_check_on_nullable_type_parameter"),
    ),
    RuleMeta::resolved::<lint::style::library_annotations::LibraryAnnotations>(
        "library-annotations",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("library_annotations"),
    ),
    RuleMeta::resolved::<lint::style::prefer_interpolation_to_compose_strings::PreferInterpolationToComposeStrings>(
        "prefer-interpolation-to-compose-strings",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("prefer_interpolation_to_compose_strings"),
    ),
    RuleMeta::resolved::<lint::correctness::type_literal_in_constant_pattern::TypeLiteralInConstantPattern>(
        "type-literal-in-constant-pattern",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("type_literal_in_constant_pattern"),
    ),
    RuleMeta::resolved::<lint::suspicious::avoid_types_as_parameter_names::AvoidTypesAsParameterNames>(
        "avoid-types-as-parameter-names",
        RuleGroup::Suspicious,
        NONE,
        false,
        RuleSource::Lints("avoid_types_as_parameter_names"),
    ),
    RuleMeta::resolved::<lint::correctness::exhaustive_cases::ExhaustiveCases>(
        "exhaustive-cases",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("exhaustive_cases"),
    ),
    RuleMeta::resolved::<lint::style::implicit_call_tearoffs::ImplicitCallTearoffs>(
        "implicit-call-tearoffs",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("implicit_call_tearoffs"),
    ),
    RuleMeta::resolved::<lint::correctness::library_private_types_in_public_api::LibraryPrivateTypesInPublicApi>(
        "library-private-types-in-public-api",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("library_private_types_in_public_api"),
    ),
    RuleMeta::resolved::<lint::correctness::invalid_runtime_check_with_js_interop_types::InvalidRuntimeCheckWithJsInteropTypes>(
        "invalid-runtime-check-with-js-interop-types",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("invalid_runtime_check_with_js_interop_types"),
    ),
    RuleMeta::file::<lint::style::file_names::FileNames>(
        "file-names",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("file_names"),
    ),
    RuleMeta::resolved::<lint::performance::prefer_contains::PreferContains>(
        "prefer-contains",
        RuleGroup::Performance,
        NONE,
        false,
        RuleSource::Lints("prefer_contains"),
    ),
    RuleMeta::cross_file::<cross_file::depend_on_referenced_packages::DependOnReferencedPackages>(
        "depend-on-referenced-packages",
        RuleGroup::Correctness,
        NONE,
        false,
        RuleSource::Lints("depend_on_referenced_packages"),
    ),
    RuleMeta::cross_file::<cross_file::package_names::PackageNames>(
        "package-names",
        RuleGroup::Style,
        NONE,
        false,
        RuleSource::Lints("package_names"),
    ),
    RuleMeta::cross_file::<cross_file::secure_pubspec_urls::SecurePubspecUrls>(
        "secure-pubspec-urls",
        RuleGroup::Suspicious,
        NONE,
        false,
        RuleSource::Lints("secure_pubspec_urls"),
    ),
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
