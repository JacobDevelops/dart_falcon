import 'dart:async';
import 'package:flutter/material.dart';
class A extends State<StatefulWidget> {
  final focus = FocusNode();
  final timer = Timer(Duration.zero, () {});
  final periodic = Timer.periodic(Duration.zero, (_) {});
  final controller = TextEditingController();
  static final shared = FocusNode();
  FocusNode? borrowed;
  void dispose() {
    focus.dispose();
    timer.cancel();
    periodic.cancel();
    controller.dispose();
    super.dispose();
  }
}
class Other { final focus = FocusNode(); }
class Arrow extends State<StatefulWidget> {
 final focus = FocusNode();
 void dispose() => focus.dispose();
}
class OrderIndependent extends State<StatefulWidget> {
 Timer? timer;
 FocusNode? focus = FocusNode();
 void dispose() {
   timer!.cancel();
   focus!.dispose();
   super.dispose();
 }
 void start() { timer = Timer.periodic(Duration.zero, (_) {}); }
}
class NestedReturn extends State<StatefulWidget> {
 final focus = FocusNode();
 void dispose() {
   void nested() { return; }
   focus.dispose();
   super.dispose();
 }
}
