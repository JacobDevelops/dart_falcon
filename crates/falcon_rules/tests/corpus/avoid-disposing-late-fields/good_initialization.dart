import 'package:flutter/material.dart';
class Resource {
  void dispose() {}
  void close() {}
  void cancel() {}
}
class Initialized extends State<StatefulWidget> {
  late Resource controller;
  late final Resource subscription;
  late Resource stream;
  void dispose() {
    controller.dispose();
    this.subscription.cancel();
    stream..close();
    super.dispose();
  }
  void initState() {
    super.initState();
    final resource = Resource();
    controller = resource;
    this.subscription = Resource();
    stream = Resource()..close();
  }
}
class Arrow extends State<StatefulWidget> {
  late Resource controller;
  void initState() => this.controller = Resource();
  void dispose() { controller.dispose(); super.dispose(); }
}
class BeforeBranch extends State<StatefulWidget> {
  late Resource controller;
  void initState() {
    controller = Resource();
    if (mounted) return;
  }
  void dispose() { controller.dispose(); super.dispose(); }
}
class StandardController extends State<StatefulWidget> {
  late final TextEditingController controller;
  void initState() {
    super.initState();
    controller = TextEditingController();
  }
  void dispose() {
    controller.dispose();
    super.dispose();
  }
}
class LazyController extends State<StatefulWidget> {
  late final AnimationController controller =
      AnimationController(vsync: this)..repeat();
  void initState() {
    super.initState();
    controller.forward();
  }
  void dispose() { controller.dispose(); super.dispose(); }
}
class QualifiedLazyController extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() => this.controller.forward();
  void dispose() { this.controller.dispose(); super.dispose(); }
}
class CascadeLazyController extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() { controller..forward(); }
  void dispose() { controller.dispose(); super.dispose(); }
}
class PropertyLazyController extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() { controller.value; }
  void dispose() { controller.dispose(); super.dispose(); }
}
class BareLazyController extends State<StatefulWidget> {
  late final AnimationController controller = AnimationController(vsync: this);
  void initState() { controller; }
  void dispose() { controller.dispose(); super.dispose(); }
}
