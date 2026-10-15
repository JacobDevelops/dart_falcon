import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget namedColor(Widget child) {
  return Container( /* expect: use-colored-box */
    color: Colors.blue,
    child: child,
  );
}

Widget constructedColor(Widget child) {
  return Container(color: const Color(0xFF00FF00), child: child); /* expect: use-colored-box */
}

Widget nonNullableParameter(Color color, Widget child) {
  return Container(key: const Key('k'), color: color, child: child); /* expect: use-colored-box */
}

Widget dynamicColor(dynamic color, Widget child) {
  return Container(child: child, color: color); /* expect: use-colored-box */
}

Widget prefixedContainer(Widget child) {
  return m.Container(color: Colors.red, child: child); /* expect: use-colored-box */
}

Widget explicitNew(Widget child) {
  return new Container(color: Colors.transparent, child: child); /* expect: use-colored-box */
}

Widget explicitNullColor(Widget child) {
  return Container(color: null, child: child); /* expect: use-colored-box */
}
