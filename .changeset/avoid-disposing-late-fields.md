---
default: minor
---

Avoid releasing late State fields from dispose: they may be unset or lazily initialized solely to release them. Flutter widget checks resolve package identities.
