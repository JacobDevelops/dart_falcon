import 'package:flutter/material.dart';

Widget alreadyDecorated(Widget child) {
  return DecoratedBox(decoration: const BoxDecoration(), child: child);
}

Widget noChild() {
  return Container(decoration: const BoxDecoration());
}

Widget noDecoration(Widget child) {
  return Container(child: child);
}

Widget extraPadding(Widget child) {
  return Container(
    padding: const EdgeInsets.all(8),
    decoration: const BoxDecoration(),
    child: child,
  );
}

Widget extraForeground(Widget child) {
  return Container(
    foregroundDecoration: const BoxDecoration(),
    decoration: const BoxDecoration(),
    child: child,
  );
}

Widget sized(Widget child) {
  return Container(width: 10, decoration: const BoxDecoration(), child: child);
}
