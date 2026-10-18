import 'package:flutter/material.dart';

Container returned(Widget child, AlignmentGeometry value) => Container(alignment: value, child: child);

void consume(Container box) {}

void typedContexts(Widget child, AlignmentGeometry value) {
  final Container box = Container(alignment: value, child: child);
  consume(Container(alignment: value, child: child));
  final List<Container> boxes = [Container(alignment: value, child: child)];
}
