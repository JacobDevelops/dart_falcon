---
default: minor
---

Add the cross-file rules `unused-dependencies`, `undeclared-dependencies`, `no-import-cycles` and `banned-imports`, all opt-in. When `undeclared-dependencies` is enabled it takes over the cases it shares with `depend-on-referenced-packages`. `banned-imports` takes `files`/`deny` glob rules; see the configuration docs.
