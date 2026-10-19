import 'package:flutter/material.dart';
class Resource { void dispose() {} }
class Conditional extends State<StatefulWidget> {
  late Resource controller;
  void initState() { if (mounted) controller = Resource(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class EarlyReturn extends State<StatefulWidget> {
  late Resource controller;
  void initState() { if (!mounted) return; controller = Resource(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class Shadowed extends State<StatefulWidget> {
  late Resource controller;
  void initState() { Resource controller; controller = Resource(); }
  void dispose() { this.controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class Deferred extends State<StatefulWidget> {
  late Resource controller;
  void initState() { final callback = () { controller = Resource(); }; }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class Async extends State<StatefulWidget> {
  late Resource controller;
  void initState() async { controller = Resource(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class Coalescing extends State<StatefulWidget> {
  late Resource? controller;
  void initState() { controller ??= Resource(); }
  void dispose() { controller?.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class Helper extends State<StatefulWidget> {
  late Resource controller;
  void initState() { initialize(); }
  void initialize() { controller = Resource(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class InitializedPeer extends State<StatefulWidget> {
  late Resource controller;
  void initState() { controller = Resource(); }
  void dispose() { controller.dispose(); super.dispose(); }
}
class NestedBlock extends State<StatefulWidget> {
  late Resource controller;
  void initState() { { controller = Resource(); } }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class CaughtFailure extends State<StatefulWidget> {
  late Resource controller;
  void initState() { try { controller = Resource(); } catch (_) {} }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class ConditionalLazyRead extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() { if (mounted) controller.forward(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields, msg: "Initialize this late field unconditionally in initState before releasing it in dispose, or make it nullable." */
}
class ShadowedLazyRead extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() { final controller = AnimationController(); controller.forward(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class DeferredLazyRead extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() { final callback = () { controller.forward(); }; }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class CaughtLazyRead extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() { try { controller.forward(); } catch (_) {} }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class BuildOnlyLazyRead extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  Widget build() { controller.forward(); return Widget(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
class UnsetRead extends State<StatefulWidget> {
  late AnimationController controller;
  void initState() { controller.forward(); }
  void dispose() { controller.dispose(); super.dispose(); } /* expect: avoid-disposing-late-fields */
}
