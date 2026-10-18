import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void examples(Widget child, AlignmentGeometry value) {
  Container(alignment: value, child: child); /* expect: prefer-align-over-container */
  new Container(alignment: value, child: child); /* expect: prefer-align-over-container */
  ui.Container(child: child, alignment: value); /* expect: prefer-align-over-container */
  new ui.Container(key: const Key("box"), alignment: value, child: child); /* expect: prefer-align-over-container */
  LayoutContainer(alignment: value, child: child); /* expect: prefer-align-over-container */
  Container(key: const Key("ordered"), child: child, alignment: (value)); /* expect: prefer-align-over-container */
}
