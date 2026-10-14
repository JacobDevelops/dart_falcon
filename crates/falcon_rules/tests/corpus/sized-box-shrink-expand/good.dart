import 'package:flutter/material.dart';

Widget alreadyShrink(Widget logo) {
  return SizedBox.shrink(child: logo);
}

Widget alreadyExpand(Widget logo) {
  return SizedBox.expand(child: logo);
}

Widget onlyWidth() {
  return const SizedBox(width: 0);
}

Widget mixedDimensions() {
  return const SizedBox(width: 0, height: double.infinity);
}

Widget nonZero() {
  return const SizedBox(width: 8, height: 8);
}

Widget computedDimensions(double size) {
  return SizedBox(width: size, height: size);
}

Widget negativeZeroIsNotALiteral() {
  return const SizedBox(width: -0, height: -0);
}

Widget fromSize() {
  return SizedBox.fromSize(size: Size.zero);
}
