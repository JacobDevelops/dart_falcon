---
default: minor
---

Add typed rule options with kebab-case keys, rule-specific JSON schema completion, and load-time errors that identify the configuration path for invalid options. Loading is stricter: options on rules that accept none, explicit null in defaulted fields that do not accept null, wrong-typed or mixed lists, even one invalid list entry, and unknown member-order tokens now fail instead of falling back to defaults or ignoring invalid values. Recognized legacy snake_case option keys remain accepted with deprecation warnings; unknown snake_case keys are left unchanged. `falcon migrate` rewrites recognized keys, preserves canonical values with collision warnings when both spellings are present, and drops invalid or unsupported entries with warnings identifying their full paths and discarded values so the migrated configuration loads successfully.
