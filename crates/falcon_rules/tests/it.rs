use falcon_rules::all_rules;

#[path = "support/inference.rs"]
mod inference;
mod support;

mod always_specify_types_ranges;
mod codegen_templates;
mod config_loading_tests;
mod corpus_tests;
mod diagnostic_deduplication;
mod identity_context;
mod inference_external;
mod legacy_switch_consumers;
mod meta_tests;
mod no_new_bespoke_walkers;
mod preserved_expression_syntax;
mod reachability_review;
mod residual_inference_contexts;
mod root_config_tests;
mod rule_import_boundaries;
mod safe_rule_fixes;
mod suppression_tests;
