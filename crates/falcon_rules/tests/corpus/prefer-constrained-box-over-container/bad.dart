import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void examples(Widget child, BoxConstraints value) {
  Container(constraints: value, child: child); /* expect: prefer-constrained-box-over-container */
  new Container(constraints: value, child: child); /* expect: prefer-constrained-box-over-container */
  ui.Container(child: child, constraints: value); /* expect: prefer-constrained-box-over-container */
  new ui.Container(key: const Key("box"), constraints: value, child: child); /* expect: prefer-constrained-box-over-container */
  LayoutContainer(constraints: value, child: child); /* expect: prefer-constrained-box-over-container */
  Container(key: const Key("ordered"), child: child, constraints: (value)); /* expect: prefer-constrained-box-over-container */
}
