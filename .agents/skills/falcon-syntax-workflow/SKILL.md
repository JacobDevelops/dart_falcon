---
name: falcon-syntax-workflow
description: Extend Dart parsing or arena CST node shapes; change parser recovery or speculation; change syntax traversal. Use for coordinated parser events, typed views, visitors, and syntax contract tests.
---

# Change Dart syntax support

Read `crates/falcon_dart_parser/src/lib.rs`, the entry points in
`crates/falcon_dart_parser/src/parser/mod.rs`, and the affected grammar-area
module. Consult `crates/falcon_dart_parser/src/grammar.md` for grammar context;
current code and contract tests establish the implemented shapes. CONTRIBUTING's
**Adding a rule end-to-end** covers rule-side consumption of the CST.

## Trace the full shape

1. Follow source through lexing, parser events, `SyntaxTree::build` in
   `crates/falcon_syntax/src/cst/tree.rs`, and the relevant typed view. The tree
   is an arena-backed lossless CST, not the historical owned AST. Views borrow
   both the tree and its original source; equal source length is not identity.
2. For a new or changed node shape, account for `cst/kind.rs`, typed views in
   `cst/decl.rs`, `types.rs`, `stmt.rs`, `expr.rs`, or `pattern.rs`, parser event
   emission, and `cst/visit.rs`, all under `crates/falcon_syntax/src/` except the
   parser. These are hand-maintained contracts, not outputs of a syntax generator.
   Finish this step with every affected cast, accessor, child edge, visitor hook,
   and walker identified.
3. Preserve event order, source offsets, trivia, EOF, and recovery-span ownership.
   Inspect `Marker::complete`, `Marker::abandon`, and `CompletedMarker::precede`
   before changing event wrapping. Forward-parent links participate in rollback;
   they are not ordinary immutable child pointers.

## Preserve speculation and recovery

- Use the existing checkpoint APIs rather than saving the token position alone.
  Rollback also covers emitted events, forward-parent edits, interpolation tokens,
  and split-token state used for generic closers such as `>>`.
- Choose `restore_cursor` when retaining diagnostics and the depth latch is
  intentional; `restore` also rolls those back. Read their current definitions
  before changing a speculative branch.
- Keep interpolation inside the same tree with absolute source spans. Include
  recovery and traversal assertions when adding an interpolation shape, not
  merely a successful parse assertion.

## Pin the contracts

1. Add the narrow regression in `crates/falcon_dart_parser/tests/`: use
   `ast_contract_tests.rs` for shape contracts despite its historical name,
   `recovery_tests.rs` for malformed input, `language_version_tests.rs` for
   version gates, or the relevant grammar-area tests. Include valid and malformed
   forms for a grammar extension; assert the shape consumers need.
2. For child-edge or visitor changes, update
   `crates/falcon_syntax/tests/visitor_schema_tests.rs` and
   `crates/falcon_syntax/tests/visitor_walk_tests.rs` as applicable. The recorder
   and representative cases are maintained by hand; enum exhaustiveness alone
   does not prove every new edge is visited exactly once in source order.
3. Run the relevant tests through the repository's test tooling, then the gates
   in `AGENTS.md`, including the rule corpus: a parser-only green result does not
   establish unchanged rule behavior. Finish when the lossless tree, typed views,
   recovery behavior, and traversal contracts all have matching regression proof.
