import 'package:flutter/material.dart';

Widget? promoted(Widget? child, EdgeInsetsGeometry? value) {
  if (child == null || value == null) return null;
  return Container(padding: value, child: child); /* expect: prefer-padding-over-container */
}
