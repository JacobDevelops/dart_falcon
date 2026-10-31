mod common;
#[path = "common/only_class.rs"]
mod common_class;
#[path = "common/errs.rs"]
mod common_errs;
#[path = "common/first_stmt_expr.rs"]
mod common_expr;
#[path = "common/string_lit.rs"]
mod common_string;

mod annotation_robustness_tests;
mod ast_contract_tests;
mod class_member_tests;
mod corpus_tests;
mod decl_tests;
mod directive_tests;
mod expr_tests;
mod expression_robustness_tests;
mod function_type_tests;
mod interpolation_tests;
mod language_version_tests;
mod lexer_tests;
mod nullable_decl_tests;
mod parser_leftovers_tests;
mod parser_tests;
mod pattern_tests;
mod recovery_tests;
mod recursive_robustness_tests;
mod stmt_tests;
