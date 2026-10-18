import 'package:flutter/material.dart';

Widget grouped(Widget child, Matrix4 value) => ((Container(transform: value, child: child))); /* expect: prefer-transform-over-container */
