---
default: patch
---

Recover unclosed string interpolations without swallowing subsequent declarations, preserve constructor-initializer parsing state across nested brackets and speculative rollback, and discard diagnostics from abandoned member and typedef parses. Keep cascade and postfix selectors on the same parsing path, retain written identifiers rather than speculative type nodes in typedefs and binding patterns, and scan nested language-version header comments in linear time.
