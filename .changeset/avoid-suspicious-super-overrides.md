---
default: minor
---

Finds getters hiding fields supplied via the immediate super constructor's `this.` formals or declaring parameters. Distinguishes named from positional super parameters; initializer-list assignments and intermediate forwarding are not tracked.
