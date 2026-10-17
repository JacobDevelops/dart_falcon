import 'package:flutter/material.dart';

class A extends State<StatefulWidget> {
  final controller = TextEditingController();

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }
}

class B extends State<StatefulWidget> {
  late final controller = widget.controller;

  @override
  void dispose() {
    super.dispose();
  }
}

class C extends State<StatefulWidget> {
  final scroll = ScrollController();
  final text = TextEditingController();

  @override
  void dispose() {
    scroll.dispose();
    text.dispose();
    super.dispose();
  }
}

class D {
  final controller = TextEditingController();
}

class E extends State<StatefulWidget> {
  TabController? tabs;

  @override
  void dispose() {
    tabs?.dispose();
    super.dispose();
  }
}

class F extends State<StatefulWidget> {
  final _controller = ScrollController();

  @override
  void dispose() {
    _controller
      ..removeListener(_onScroll)
      ..dispose();
    super.dispose();
  }

  void _onScroll() {}
}

class G extends State<StatefulWidget> {
  final controller = TextEditingController();

  @override
  void dispose() {
    this.controller.dispose();
    super.dispose();
  }
}

class CollectionIfCases extends State<StatefulWidget> {
  final controller = TextEditingController();

  @override
  void dispose() {
    final value = widget;
    final list = [if (value case final controller?) controller];
    final set = <Object>{if (value case final controller?) controller};
    final map = <Object, Object>{
      if (value case final controller?) controller: controller,
    };
    controller.dispose();
    super.dispose();
  }
}

class RestoredBodyScopes extends State<StatefulWidget> {
  final controller = TextEditingController();

  @override
  void dispose() {
    {
      final controller = TextEditingController();
      controller.dispose();
    }
    for (final controller in <TextEditingController>[]) {
      controller.dispose();
    }
    for (final [controller] in <List<TextEditingController>>[]) {
      controller.dispose();
    }
    if (widget case final controller?) {
      final list = [if (widget case final nested?) nested];
    } else {
      controller.dispose();
    }
    try {
      throw widget;
    } catch (controller) {}
    final callback = (TextEditingController controller) {
      controller.dispose();
    };
    void local(TextEditingController controller) {
      controller.dispose();
    }
    controller.dispose();
    super.dispose();
  }
}

class InitializerIsNotBodyOwnership extends State<StatefulWidget> {
  final TextEditingController controller;

  InitializerIsNotBodyOwnership() : controller = TextEditingController();
}
