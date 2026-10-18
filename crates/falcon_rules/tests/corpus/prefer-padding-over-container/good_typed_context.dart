import 'package:flutter/material.dart';

Container returned(Widget child, EdgeInsetsGeometry value) => Container(padding: value, child: child);

void consume(Container box) {}

void typedContexts(Widget child, EdgeInsetsGeometry value) {
  final Container box = Container(padding: value, child: child);
  consume(Container(padding: value, child: child));
  final List<Container> boxes = [Container(padding: value, child: child)];
}
