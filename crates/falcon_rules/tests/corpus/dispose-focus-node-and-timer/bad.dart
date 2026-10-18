import 'dart:async';
import 'package:flutter/material.dart';
class A extends State<StatefulWidget> {
  final focus = FocusNode(); /* expect: dispose-focus-node-and-timer */
  final timer = Timer(Duration.zero, () {}); /* expect: dispose-focus-node-and-timer */
  final periodic = Timer.periodic(Duration.zero, (_) {}); /* expect: dispose-focus-node-and-timer */
  final tearoff = FocusNode(); /* expect: dispose-focus-node-and-timer */
  final closure = FocusNode(); /* expect: dispose-focus-node-and-timer */
  final conditional = FocusNode(); /* expect: dispose-focus-node-and-timer */
  void dispose() {
    tearoff.dispose;
    final callback = () => closure.dispose();
    if (false) conditional.dispose();
    super.dispose();
  }
}
class EarlyReturn extends State<StatefulWidget> {
 final focus = FocusNode(); /* expect: dispose-focus-node-and-timer */
 final timer = Timer(Duration.zero, () {}); /* expect: dispose-focus-node-and-timer */
 void dispose() {
   if (mounted) return;
   focus.dispose();
   timer.cancel();
   super.dispose();
 }
}
class ThrowExpression extends State<StatefulWidget> {
 final focus = FocusNode(); /* expect: dispose-focus-node-and-timer */
 void dispose() {
   final check = mounted ? null : throw StateError('not mounted');
   focus.dispose();
   super.dispose();
 }
}
