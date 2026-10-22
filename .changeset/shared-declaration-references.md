---
default: patch
---

Share declaration-reference resolution across reachability, same-package deprecation, private-field initialization, and nullable-parameter analysis, preserving declared-member, library-scope, then inherited-member lookup precedence. `unreachable-from-main` now uses the shared Dartdoc reference parser: Markdown link display text and link definitions, inline and block code (fenced or indented), and Dartdoc directives no longer keep declarations reachable. Reference-style destination labels retain the shared parser's interpretation as references. Documentation grouping also follows the shared parser, so a later block documentation comment replaces an immediately preceding line-documentation group.

Shared documentation queries now include comments inside string interpolation bodies. Reachability alone excludes trailing inline line-documentation comments from the following declaration; other shared documentation clients retain their existing inline-comment interpretation.

Private-field initializing formals and constructor initializer targets continue to disqualify only their declaring class's field, not same-named fields in other classes; focused unit tests preserve this ownership behavior and library-scope precedence without adding corpus expectations.

Indexed file lookup now tries an absolute spelling of relative paths before filesystem canonicalization, avoiding repeated canonicalization for already indexed CLI inputs while preserving symlink and parent-component resolution.
