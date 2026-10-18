import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget direct(List<String> labels) => Column(children: labels.map((label) => Text(label)).toList()); /* expect: prefer-for-loop-in-children */
Widget row(Set<String> labels) => new Row(children: labels.map((label) => Text(label)).toList()); /* expect: prefer-for-loop-in-children */
Widget prefixed(Iterable<String> labels) => m.Column(children: labels.map((label) => m.Text(label)).toList()); /* expect: prefer-for-loop-in-children */
Widget listView(List<String> labels) => ListView(children: labels.map<Widget>((label) => Text(label)).toList()); /* expect: prefer-for-loop-in-children */
Widget literal() => Column(children: ['one', 'two'].map((label) => Text(label)).toList()); /* expect: prefer-for-loop-in-children */
