---
default: minor
---

Add configurable bans on resolved explicit type annotations, including expanded aliases and nested generic arguments in new/const constructions. Constructed class names and unknown types are exempt. Matching stays inactive until configured with deny patterns and diagnostic messages.
