import 'package:flutter/material.dart';

Widget alreadyColored(Widget child) {
  return ColoredBox(color: Colors.blue, child: child);
}

Widget nullableColor(Color? color, Widget child) {
  return Container(color: color, child: child);
}

Widget shadeLookupIsNullable(Widget child) {
  return Container(color: Colors.blue[100], child: child);
}

Widget noChild() {
  return Container(color: Colors.blue);
}

Widget extraPadding(Widget child) {
  return Container(
    padding: const EdgeInsets.all(4),
    color: Colors.blue,
    child: child,
  );
}

Widget unprovenColor(BuildContext context, Widget child) {
  return Container(color: Theme.of(context).primaryColor, child: child);
}

Widget decorated(Widget child) {
  return Container(
    color: Colors.blue,
    alignment: Alignment.center,
    child: child,
  );
}
