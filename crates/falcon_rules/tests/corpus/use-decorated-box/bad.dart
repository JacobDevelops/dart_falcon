import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget decorationAndChild(Widget child) {
  return Container( /* expect: use-decorated-box */
    decoration: const BoxDecoration(),
    child: child,
  );
}

Widget childFirst(Widget child) {
  return Container(child: child, decoration: BoxDecoration()); /* expect: use-decorated-box */
}

Widget withKey(Widget child) {
  return Container( /* expect: use-decorated-box */
    key: const Key('box'),
    decoration: BoxDecoration(color: Colors.red),
    child: child,
  );
}

Widget prefixed(Widget child) {
  return m.Container(decoration: m.BoxDecoration(), child: child); /* expect: use-decorated-box */
}

Widget explicitNew(Widget child) {
  return new Container(decoration: BoxDecoration(), child: child); /* expect: use-decorated-box */
}

Widget nested(Widget child) {
  return Center(
    child: Container(decoration: BoxDecoration(), child: child), /* expect: use-decorated-box */
  );
}
