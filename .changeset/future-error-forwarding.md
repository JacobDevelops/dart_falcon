---
default: patch
---

`avoid-catches-without-on-clauses` requires a resolved SDK `Future` identity before treating `.error(...)` as error forwarding, so ordinary logger calls still report an untyped catch.
