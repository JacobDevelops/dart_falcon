import 'package:flutter/material.dart';

Widget? promoted(Widget? child, Matrix4? value) {
  if (child == null || value == null) return null;
  return Container(transform: value, child: child); /* expect: prefer-transform-over-container */
}
