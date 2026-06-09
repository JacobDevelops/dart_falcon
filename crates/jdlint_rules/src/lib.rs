//! Ported lint rules from dart_code_linter and pyramid_lint.
//!
//! Rules are registered via `RuleRegistry` in `jdlint_analyze`.
//! Each rule is a zero-sized struct implementing the `Rule` trait.
//!
//! Phase 1 target: ~60 rules from jfit's analysis_options.yaml.
//! Complexity tiers (assigned in M0.5): SIMPLE / MEDIUM / COMPLEX.

pub mod dart_code_linter;
pub mod pyramid_lint;
