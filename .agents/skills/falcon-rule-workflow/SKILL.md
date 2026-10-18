---
name: falcon-rule-workflow
description: Add or remove a falcon lint rule; change rule diagnostics or options; refresh rule fixtures and generated website data. Use for the metadata, corpus, and documentation contracts beyond the contributor procedure.
---

# Change a falcon rule

Run from the repository root. Read CONTRIBUTING's **Adding a rule end-to-end**,
**The golden corpus**, and, for options, **Rule options**. Follow that procedure
for scaffolding, analysis kind, metadata, showcase config, and fixture format;
this skill covers the checks and generated surfaces it does not sequence.

## Establish the behavioral contract

1. Locate the rule through `RULE_METADATA` in `crates/falcon_rules/src/meta.rs`.
   For a new rule, use CONTRIBUTING's codegen command. Confirm its execution kind,
   group, domain, and source before choosing an existing implementation to copy.
2. Read `crates/falcon_rules/tests/corpus/FIXTURE_FORMAT.md` for the relevant
   fixture branch and inspect neighboring fixtures.
3. Pin the changed diagnostic behavior with positive and negative fixtures. For
   constructor checks, use CONTRIBUTING's construction-shape warning to choose
   cases. For cross-file analysis, include project fixtures, not only single-file
   lookalikes. This step is complete when each changed behavior has an expected
   diagnostic or an explicit clean case.

## Exercise both harnesses

- Iterate with `cargo xtask validate-rules --rule <rule-id>` and confirm the
  summary actually checked the intended fixtures. Filtering selects existing
  directories; a misspelled or missing directory is not coverage.
- Default `validate-rules` builds Falcon itself. With `--falcon-bin`, the caller
  owns binary freshness; `makers validate_rules` handles its build dependency.
- Keep both `makers test` and `makers validate_rules` in final verification.
  `crates/falcon_rules/tests/corpus_tests.rs` runs rules in-process and enforces
  registered-rule coverage, orphan-directory rejection, and non-placeholder
  fixtures. The xtask harness executes the CLI and iterates existing directories;
  a green CLI corpus alone does not establish registration coverage.

## Refresh derived contracts

1. After identity or configuration-surface changes, run `cargo xtask schema`.
   `schema/falcon.schema.json` is checked by `meta_tests.rs`; generating it does
   not update the root showcase config. For removals and renames, account for the
   old module, metadata, config entry, and fixture directory as well as the new one.
2. After rule documentation, metadata, fixture, diagnostic, option, or CLI/config
   reference changes, run `cargo xtask docgen`. It builds Falcon and writes
   `website/src/data/rules.json`, `domains.json`, `cli.json`, and
   `config-reference.json`. Rule prose comes from implementation documentation;
   examples come from corpus files and real CLI diagnostics. Review generated
   examples as behavior, not just a JSON diff.
3. Apply CONTRIBUTING's **Commits & pull requests** changeset policy. Finish when
   source, fixtures, showcase, applicable generated outputs, and changeset agree,
   and the gates in `AGENTS.md` have passed or their exact blockers are reported.
