---
default: minor
---

Add `avoid-undisposed-instances`, `dispose-focus-node-and-timer`, `dispose-fields`, `always-remove-listener`, `avoid-recursive-widget-calls` and `avoid-unnecessary-stateful-widgets`. A disposable passed to another call counts as handed over, so `Navigator.push(MaterialPageRoute(...))` isn't flagged.
