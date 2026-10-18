import 'package:flutter/material.dart';

Container returned(Widget child, Matrix4 value) => Container(transform: value, child: child);

void consume(Container box) {}

void typedContexts(Widget child, Matrix4 value) {
  final Container box = Container(transform: value, child: child);
  consume(Container(transform: value, child: child));
  final List<Container> boxes = [Container(transform: value, child: child)];
}
