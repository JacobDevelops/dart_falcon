import 'dart:async'; /* expect: unnecessary-flutter-imports */
import 'package:flutter/widgets.dart'; /* expect: unnecessary-flutter-imports */
import 'package:flutter/material.dart';

class Example extends State<StatefulWidget> {
  Widget build() {
    setState(() {});
    final value = widget.hashCode;
    final location = context.hashCode;
    return MaterialOnlyWidget(value, location);
  }
}

class MaterialOnlyWidget extends MaterialOnly implements Widget {
  MaterialOnlyWidget(Object value, Object location);
}

mixin Counter {
  int count = 0;
  void increment() => count++;
}
class Parent {
  final int field = 1;
}
class Middle extends Parent {}
class Child extends Middle with Counter {
  int own = 2;
  int read() {
    increment();
    return field + count + own;
  }
  int shadow() {
    final field = 3;
    return field;
  }
}

extension ExampleExtension on Child {
  int get total => field + count + own + extra;
  int get extra => 4;
}
