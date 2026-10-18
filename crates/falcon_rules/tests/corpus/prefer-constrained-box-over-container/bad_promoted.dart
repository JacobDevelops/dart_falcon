import 'package:flutter/material.dart';

Widget? promoted(Widget? child, BoxConstraints? value) {
  if (child == null || value == null) return null;
  return Container(constraints: value, child: child); /* expect: prefer-constrained-box-over-container */
}
