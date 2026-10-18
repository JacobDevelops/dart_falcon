import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 void helper() { Theme.of(context); }
 void initState() {
  helper(); /* expect: avoid-inherited-widget-in-initstate */
  context.dependOnInheritedWidgetOfExactType<InheritedWidget>(); /* expect: avoid-inherited-widget-in-initstate */
 }
}
