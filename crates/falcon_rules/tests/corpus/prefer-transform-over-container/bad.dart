import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void examples(Widget child, Matrix4 value) {
  Container(transform: value, child: child); /* expect: prefer-transform-over-container */
  new Container(transform: value, child: child); /* expect: prefer-transform-over-container */
  ui.Container(child: child, transform: value); /* expect: prefer-transform-over-container */
  new ui.Container(key: const Key("box"), transform: value, child: child); /* expect: prefer-transform-over-container */
  LayoutContainer(transform: value, child: child); /* expect: prefer-transform-over-container */
  Container(key: const Key("ordered"), child: child, transform: (value)); /* expect: prefer-transform-over-container */
  Container(transform: value, transformAlignment: Alignment.center, child: child); /* expect: prefer-transform-over-container */
}
