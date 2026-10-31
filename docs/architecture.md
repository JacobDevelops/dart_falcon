# Falcon engineering and ownership contract

This is the accepted direction for cleanup, not a claim that every current API
already meets it. [The v1 checklist](v1-checklist.md) is the single readiness
backlog. Rule implementation completeness and release readiness are different.
Preserve observable behavior while moving one justified responsibility at a time.

## Domain boundaries

Strict domain-driven design applies to Falcon's language-analysis domain. The
owners are source snapshots, syntax, semantic facts, rule decisions, project
analysis and diagnostics. Domain operations protect their invariants; application
code coordinates those operations; filesystem, CLI and LSP adapters translate
external input and outcomes. Composition constructs/configures these owners and
manages lifecycle. A crate name does not prove the code respects these boundaries.

| Current location | Responsibility and target boundary |
| --- | --- |
| `falcon_syntax` | Arena CST, typed views, spans and traversal contracts; matching source/tree identities must stay owned together. |
| `falcon_dart_parser` | Lexing, grammar, lossless construction and recovery; produces syntax with explicit parse outcomes. |
| `falcon_diagnostics` | Diagnostic values and source locations; frontend presentation remains outside language decisions. |
| `falcon_config` | Configuration validation and bounded metadata ingestion; configuration choices become explicit analysis inputs. |
| `falcon_analyze` | Immutable parsed snapshots, semantic facts, shared visitors, rule lifecycle and analysis results. Current environment/source loading is an acquisition boundary to clarify, not permission for arbitrary rule I/O. |
| `falcon_rules` | Declared rule policy over supplied syntax/semantic/project facts. Existing manifest reads are migration debt; use shared snapshots as those consumers are migrated. |
| `falcon_project` | Shared application orchestration: discovery, configuration, analysis planning, document/project stages, suppression and finalization. CLI and LSP retain only their distinct lifecycle/presentation policies. |
| `falcon_cli`, `falcon_lsp` | Arguments/protocol, transport, editor document lifecycle, cancellation and output adapters. Neither may become a second rule or semantic engine. |
| Root executable | Composition and allocator selection. |
| `xtask` | Contributor/release-generation tooling; not a dependency of runtime crates. |

Use existing modules and concrete types first. New crates, traits, repositories,
services, DTOs, state machines or event systems need a present caller and a
responsibility that is actually simpler to enforce there. Do not transplant an
ordering/payment model or create a crate for every layer. Coherent shared pipeline
operations should not be split merely to satisfy a diagram.

The current permitted local production/build edges are in
[`scripts/architecture.json`](../scripts/architecture.json). `makers architecture`
reads Cargo metadata, including renamed, optional and target-specific declared
dependencies. It rejects new/unclassified workspace members, unclassified local
production dependencies, forbidden edges and production cycles. Test-only edges
are exempt; third-party dependencies need normal review. The standalone Zed
workspace and non-Rust clients are outside this graph. This is a dependency
regression gate, not a proof of pure domain code, source-level access restrictions
or correct ownership. Removing an edge is allowed; expanding the policy requires
an explained boundary decision and corresponding checklist update, not just a
new allowlist entry to make a check green.

## Rust and invariant discipline

- Keep fields and types private until a real consumer needs access. Constructors
  establish relationships; accessors expose facts. Derive plan flags from registry
  requirements instead of accepting independently writable contradictory values.
  A borrowed view must retain the identity/lifetime of its original source/tree.
- Borrow for inspection; own for retention or transfer. `Arc`, locks, interior
  mutability, clones and retained caches require an actual sharing/lifetime need.
  Do not remove a useful snapshot or cache just to reduce field count.
- Expected invalid input, unreadable dependencies, cancellation and partial
  analysis have meaningful outcomes. `Option` represents legitimate absence,
  not an unexplained swallowed failure. Map errors at their owner boundary and
  retain enough context for the CLI/editor to report what was not analyzed.
- Use assertions for internal relationships not already guaranteed by types;
  expected user input failures must not depend on panics. Justify production
  `expect`; use closed-enum matches so new states require a decision. Do not hide
  reachable uncertainty with a simpler-looking success result.
- Bound input bytes, recursive depth, queues, retained results and whole requests.
  A per-file bound does not bound a project; a thread count does not cap memory.
  Revalidate after suspension or snapshot replacement. Cancellation must reach
  long-lived work, and shutdown must release owned resources.
- Prefer readable guard clauses, `?`, ordinary iterators and coherent helpers.
  Avoid arbitrary line-count extraction, blanket newtypes, automatic trait layers
  and mechanical style churn. Use checked conversions/arithmetic where input can
  exceed the representation, especially source offsets and allocation sizes.
- Keep existing formatter and strict Clippy gates. Do not suppress warnings,
  weaken fixture expectations or remove failure tests merely to pass. Do not copy
  business-specific numeric policies or introduce sweeping lint bans without
  assessing this codebase and a bounded migration.

## Design and review a cleanup slice

Before editing, identify the checklist ID, one real caller, the owner/invariant,
current failure mode and the proposed smaller API. State what remains outside the
slice. For an ownership move, trace data acquisition through analysis to CLI/LSP
publication and explain exactly which decision moves; a directory move alone is
not completion. Use the syntax/rule skills when those contracts change.

Tests must distinguish a plausible wrong result: independent expected diagnostics,
positive/negative and boundary cases, relevant invalid-to-valid recovery, source
replacement and cancellation/reordering. Compare actual diagnostic identity,
range, message, severity and suppression behavior where parity is claimed. A mock
or count-only comparison cannot prove a full user journey. Retain rejected-input
and same-size edit coverage for fingerprints. Do not certify incomplete results
as clean merely because a worker returned successfully.

Obtain independent review for ownership, concurrency, input-boundary and public
behavior changes when available. The reviewer reads code/tests and challenges the
actual caller and failure contract, not just lint output. State any unavailable
review explicitly. Record command, exact revision/source fingerprint, environment,
results, skips and limitations in the checklist entry or change handoff. Review
and checks are invalidated by source/base changes; rebase and revalidate the settled
candidate before integration. Follow [verification](verification.md) for scoped,
Blaze and final gates. No repository scheduler or personal subscription is needed.

## Measurements and release claims

Clean boundaries make profiling easier; they do not guarantee faster runtime.
Separate parse/structural microbenchmarks, semantic/registry execution and complete
CLI analysis. Assert representative work occurs outside timed sections. Record
corpus revision, rule/configuration parity, environment, toolchain, cache state,
failures, elapsed distribution and peak resources. Attribute shared visitor/index
work to shared stages; a preparation span is not total per-rule cost.

Retain an uncached full-analysis CLI measurement lane alongside any future caching.
Distinguish analyzer state from OS file cache and equivalent rule names from
identical work. Beating warm Dart analysis with cold Falcon is an aspiration until
the representative suite and acceptance margin are agreed. The checklist records
this unresolved release decision; do not invent numeric commitments.

## Provenance

Adapted from LOKE's Rust style, design-sketch and review practices: explicit domain
owners, private invariants, bounded work, concrete abstractions and discriminating
verification. LOKE's product model, finance-specific rules, numeric function-size
threshold, backend wrappers and personal orchestration are not Falcon standards.
This document is self-contained; contributors need no LOKE checkout or Numen.
