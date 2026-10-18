import 'package:flutter/material.dart';

Widget parenthesized(List<String> labels) => (Column(children: labels.map((label) => Text(label)).toList())); /* expect: prefer-for-loop-in-children */
