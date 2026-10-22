---
default: minor
---

Add typed rule options with kebab-case keys, rule-specific JSON schema completion, and load-time errors that identify the configuration path for invalid options. Loading rejects unsupported options, invalid values, mixed lists, and unknown member-order tokens instead of falling back to defaults. Whole-number numeric options and max-errors values such as `1.0` are accepted, out-of-range integers name their allowed range, and optional sections accept null in both the loader and editor schema. Recognized legacy snake_case option keys remain accepted with deprecation warnings.
