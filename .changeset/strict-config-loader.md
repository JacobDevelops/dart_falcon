---
default: patch
---

Reject unknown configuration keys, rule names, rule groups, and domains with their full key paths, including inside overrides, instead of silently ignoring typos. Legacy spellings remain accepted with returned deprecation warnings. Configuration loading now consistently canonicalizes rule aliases, using the same severity and options collision policy as migration. Library loading and registry validation no longer print warnings directly; the CLI reports them and the language server delivers configuration warnings and errors to the editor.

Reject rules configured under the wrong group or section and unsupported override preset toggles, rather than accepting settings that have no effect. Keep the published override schema in sync and report the configuration discarded when legacy and canonical rule names collide.

Preserve original source-path file filters and ordered rule overrides when SDK-backed corpus validation stages sources under temporary project roots.
