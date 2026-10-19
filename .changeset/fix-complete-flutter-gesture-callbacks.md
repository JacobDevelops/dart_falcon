---
default: patch
---

Recognize Flutter callbacks that enable active gestures in `accessible-widget-labels` and `minimum-touch-target`, including secondary and tertiary long presses, force presses, and drag cancellation. Match InkWell and InkResponse's separate enablement rules, so cancellation-only ink controls remain inactive. Unwired tap-movement callbacks, literal-null callbacks, and hover, focus, and highlight notifications don't enable gestures.
