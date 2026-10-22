---
default: patch
---

Resolve Flutter widget and state types and SDK asynchronous error APIs by declaration identity in `avoid-returning-widgets`, `member-ordering`, `avoid-returning-null-for-void`, and `avoid-catches-without-on-clauses`. User-defined lookalike types no longer receive framework-specific treatment, and prefixed imports and type aliases retain their original identities.

Require a resolved SDK `Future` identity before treating `.error(...)` as error forwarding; ordinary logger calls still report an untyped catch.

Keep unknown generic arity unknown instead of inventing zero type parameters for types such as `ValueNotifier` and `Provider`, while retaining explicitly curated zero-arity types. Cache package identities by canonical file and prevent nested virtual directory aliases from crossing package boundaries.
