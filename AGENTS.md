# falcon agent guide

falcon is a Rust linter and LSP for Dart. The root Cargo package is
`dart_falcon`; the executable is `falcon`.

Project skills live in `.agents/skills/`.

## Start with the relevant contract

- Before setup or code changes, read `CONTRIBUTING.md` for the dev environment,
  contributor gates, and contribution policy. `Makefile.toml` owns task definitions;
  `mise.toml` and `rust-toolchain.toml` own tool selection.
- For lint-rule implementation, behavior changes, or removal, use
  `.agents/skills/falcon-rule-workflow/SKILL.md`. It supplements, rather than
  replaces, CONTRIBUTING's rule-authoring procedure.
- For parser or syntax-model changes, use
  `.agents/skills/falcon-syntax-workflow/SKILL.md`.
- For configuration changes, read `docs/configuration.md` and
  `crates/falcon_config/src/lib.rs`. Preserve the Biome-shaped grouped surface;
  new JSON keys and rule ids use kebab-case, while Rust fields use snake_case.
- Each user-facing change ships with its own short changeset
  (`makers changeset`); read CONTRIBUTING's **Commits & pull requests** for the
  format and policy and **Releasing (maintainers)** before touching versions or
  release workflows.
- Docs pin the current falcon version only where a pin is the point (mise and
  CI snippets); `cargo xtask version` rewrites those, and an xtask test fails if
  one drifts. Install links use the versionless `releases/latest/download/` form.
- For planned scope, consult `docs/v1-checklist.md`; it lists unshipped work,
  so don't treat it as evidence of implemented behavior.

## Boundaries and generated content

- `crates/falcon_syntax` owns arena CST storage and typed views;
  `crates/falcon_dart_parser` builds it. `crates/falcon_analyze` owns shared
  analysis and visitor contracts; `crates/falcon_rules` implements rules.
  Read the affected crate's `src/lib.rs` and nearby tests before crossing a boundary.
- `schema/falcon.schema.json` and the JSON files in `website/src/data/` are
  generated contracts. Use `cargo xtask schema` and `cargo xtask docgen`,
  respectively, when their inputs change; keep generated changes with the source.
  Docgen runs real diagnostics from a freshly built binary, not static examples alone.
- For website work, read `website/package.json` and `website/DEPLOY.md`.
  `website/src/content/` is generated; edit the source Markdown in `docs/` instead.

## Completion

Read CONTRIBUTING's **The gates** before checks. Use `makers verify-rule <id>`
or `makers verify-package <package>` while iterating; use `makers verify-fast`
for a coherent settled slice. The coordinator runs `makers verify-full` (alias
`makers A`) once on the settled final candidate, not once per agent or edit.
Scoped/fast checks do not replace final coverage. Use `makers verify-tooling`
for changes to verification tooling itself; report heavier gates not run.

Keep writers out of the source being verified. Concurrent independent workspaces
may run bounded checks; retain their own target directories and let mbx share
compiler outputs. Do not start redundant aggregate gates, delete shared cache,
or reuse an earlier success after source/config/toolchain changes. The wrapper
fails on observed source drift and never caches a passing check result. Record
command, checked source, observed result and review limitations in the handoff.
See `docs/verification.md` for budgets, cancellation and scope. Personal agent
loops, cowork coordination and model routing belong outside this OSS repository.

For agent-documentation-only changes, check paths, commands, and symlink targets;
Rust builds are not a substitute for checking the instructions themselves.
