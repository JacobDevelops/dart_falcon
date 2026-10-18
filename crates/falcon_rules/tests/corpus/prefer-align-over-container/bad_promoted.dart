import 'package:flutter/material.dart';

Widget? promoted(Widget? child, AlignmentGeometry? value) {
  if (child == null || value == null) return null;
  return Container(alignment: value, child: child); /* expect: prefer-align-over-container */
}
