---
default: minor
---

Add configurable bans on resolved API references, matching top-level names or `DeclaringType.member`. Covers calls, tear-offs, properties, cascades, context-typed dot shorthands, constructor invocations, initializers and redirecting factories. Lexical declarations take precedence over inherited members; unresolved targets are ignored. Matching stays inactive until configured with deny patterns and diagnostic messages.

Unqualified instance and static fields remain checked unless shadowed by a local declaration. Named extensions resolve their own members without consulting the on-type. Property reads and writes use their respective declaring owners, including extension setters. Compound updates prefer matching setter bans and fall back to getter bans, preserving all configured messages for the selected owner.

Cascade assignment targets resolve only against the cascade receiver; assigned values retain normal lexical resolution.
