---
default: patch
---

Preserve configuration key order during in-place `falcon migrate` upgrades, including renamed sections and rules. Use the same collision policy for YAML conversion, JSON upgrades, and configuration loading: the highest severity wins, and an explicitly canonical rule keeps its options on a severity tie.
