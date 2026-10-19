---
default: minor
---

Add configurable bans on declaration names, including loop variables, catch parameters, named constructors and import prefixes. References, initializing/super formals, and `@override`-annotated members and their parameters are exempt. Matching stays inactive until configured with deny patterns and diagnostic messages.
