import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void examples(Widget child, EdgeInsetsGeometry value) {
  Container(padding: value, child: child); /* expect: prefer-padding-over-container */
  new Container(padding: value, child: child); /* expect: prefer-padding-over-container */
  ui.Container(child: child, padding: value); /* expect: prefer-padding-over-container */
  new ui.Container(key: const Key("box"), padding: value, child: child); /* expect: prefer-padding-over-container */
  LayoutContainer(padding: value, child: child); /* expect: prefer-padding-over-container */
  Container(key: const Key("ordered"), child: child, padding: (value)); /* expect: prefer-padding-over-container */
  Container(margin: value, child: child); /* expect: prefer-padding-over-container */
  ui.Container(key: const Key("margin"), margin: value, child: child); /* expect: prefer-padding-over-container */
}
