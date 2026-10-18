import 'package:flutter/widgets.dart';

Widget grouped(Widget child) => ((Align(child: child))); /* expect: prefer-center-over-align */
