import 'package:flutter/material.dart';

Widget grouped(Widget child, AlignmentGeometry value) => ((Container(alignment: value, child: child))); /* expect: prefer-align-over-container */
