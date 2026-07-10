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
