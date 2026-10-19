---
default: minor
---

Add configurable bans on resolved API references, matching top-level names or `DeclaringType.member`. Covers calls, tear-offs, properties, cascades, context-typed dot shorthands, constructor invocations, initializers and redirecting factories. Lexical declarations take precedence over inherited members; unresolved targets are ignored. Matching stays inactive until configured with deny patterns and diagnostic messages.
