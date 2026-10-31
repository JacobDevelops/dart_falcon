---
default: patch
---

Reject incomplete analysis and fixes when existing project companion sources have genuine read failures, and suppress cross-file language-server results until those sources recover or are no longer referenced while retaining per-file semantic analysis. Missing or non-file directive targets remain absent; unreadable external sources remain optional with warnings.
Companion scope follows analyzed packages, discovered pub/Melos workspaces and the containing directories of loose sources, independently of the CLI current directory, config/VCS roots, or language-server workspace folders. Language-server scope includes packages without open buffers, and fix-all actions refuse incomplete semantic sources.
