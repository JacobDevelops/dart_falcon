---
default: patch
---
Keep absence-dependent checks conservative when package configuration is unreadable, invalid or partially invalid, without discarding valid entries' package roots, language versions and resolved import/test facts. Skip `unused-dependencies` and `unused-dev-dependencies` absence findings until complete metadata recovers; resume checking against current metadata afterward.
