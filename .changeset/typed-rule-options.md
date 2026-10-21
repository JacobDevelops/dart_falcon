---
default: minor
---

Add typed rule options with kebab-case keys, rule-specific JSON schema completion, and load-time errors that identify the configuration path for invalid options. Legacy snake_case option keys remain accepted with deprecation warnings, and `falcon migrate` rewrites them to kebab-case while preserving canonical values when both spellings are present. Migration rejects invalid rule options before writing a configuration.
