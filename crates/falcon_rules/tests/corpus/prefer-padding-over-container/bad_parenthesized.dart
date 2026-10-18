import 'package:flutter/material.dart';

Widget grouped(Widget child, EdgeInsetsGeometry value) => ((Container(padding: value, child: child))); /* expect: prefer-padding-over-container */
