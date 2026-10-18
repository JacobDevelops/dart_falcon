import 'package:flutter/material.dart';

Widget grouped(Widget child, BoxConstraints value) => ((Container(constraints: value, child: child))); /* expect: prefer-constrained-box-over-container */
