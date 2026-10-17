import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as flutter;

class A extends State<StatefulWidget> {
  final controller = TextEditingController(); /* expect: proper-controller-dispose */

  @override
  void dispose() {
    super.dispose();
  }
}

class B extends State<StatefulWidget> {
  final page = PageController(); /* expect: proper-controller-dispose */
}

class C extends State<StatefulWidget> {
  late ScrollController scroll; /* expect: proper-controller-dispose */

  @override
  void initState() {
    super.initState();
    scroll = ScrollController();
  }

  @override
  void dispose() {
    super.dispose();
  }
}

class D extends State<StatefulWidget> {
  final tabs = TabController(length: 2, vsync: this); /* expect: proper-controller-dispose */
  final text = TextEditingController();

  @override
  void dispose() {
    text.dispose();
    super.dispose();
  }
}

class E extends State<StatefulWidget> {
  late final anim = AnimationController.unbounded(vsync: this); /* expect: proper-controller-dispose */

  @override
  void dispose() {
    super.dispose();
  }
}

class F extends State<StatefulWidget> {
  final controller = TextEditingController(); /* expect: proper-controller-dispose */

  @override
  void dispose() {
    final controller = TextEditingController();
    controller.dispose();
    super.dispose();
  }
}

class G extends State<StatefulWidget> {
  final controller = TextEditingController(text: widget.initialText); /* expect: proper-controller-dispose */
}

class H extends flutter.State<flutter.StatefulWidget> {
  final controller = flutter.ScrollController(); /* expect: proper-controller-dispose */
}

class CollectionIfCaseInsideStatement extends State<StatefulWidget> {
  late TextEditingController controller; /* expect: proper-controller-dispose */

  @override
  void initState() {
    super.initState();
    if (widget case final controller?) {
      final list = [if (widget case final nested?) nested];
    }
    controller = TextEditingController();
  }
}

class ShadowedBodyDisposals extends State<StatefulWidget> {
  final blockShadow = TextEditingController(); /* expect: proper-controller-dispose */
  final forShadow = TextEditingController(); /* expect: proper-controller-dispose */
  final patternShadow = TextEditingController(); /* expect: proper-controller-dispose */
  final caseShadow = TextEditingController(); /* expect: proper-controller-dispose */
  final catchShadow = TextEditingController(); /* expect: proper-controller-dispose */
  final closureShadow = TextEditingController(); /* expect: proper-controller-dispose */
  final functionShadow = TextEditingController(); /* expect: proper-controller-dispose */

  @override
  void dispose() {
    {
      final blockShadow = TextEditingController();
      blockShadow.dispose();
    }
    for (final forShadow in <TextEditingController>[]) {
      forShadow.dispose();
    }
    for (final [patternShadow] in <List<TextEditingController>>[]) {
      patternShadow.dispose();
    }
    if (widget case TextEditingController caseShadow) {
      caseShadow.dispose();
    }
    try {
      throw widget;
    } catch (catchShadow) {
      catchShadow.dispose();
    }
    final callback = (TextEditingController closureShadow) {
      closureShadow.dispose();
    };
    void local(TextEditingController functionShadow) {
      functionShadow.dispose();
    }
    super.dispose();
  }
}
