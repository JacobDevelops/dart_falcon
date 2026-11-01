---
default: patch
---

Unreadable dependency exports now warn once per failure episode and conservatively skip only their dependent deep-import check. Inaccessible test-file and conditional-URI targets now warn and suppress the dependent missing-file diagnostic instead of being treated as absent. Invalid UTF-8 asset entry warnings now say "file name" rather than "asset file name" and include both the directory and entry paths.
