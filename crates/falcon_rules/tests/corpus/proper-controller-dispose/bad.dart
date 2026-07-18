import 'package:flutter/material.dart';

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
