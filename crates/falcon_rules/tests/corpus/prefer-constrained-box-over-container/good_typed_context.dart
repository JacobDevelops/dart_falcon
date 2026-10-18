import 'package:flutter/material.dart';

Container returned(Widget child, BoxConstraints value) => Container(constraints: value, child: child);

void consume(Container box) {}

void typedContexts(Widget child, BoxConstraints value) {
  final Container box = Container(constraints: value, child: child);
  consume(Container(constraints: value, child: child));
  final List<Container> boxes = [Container(constraints: value, child: child)];
}
