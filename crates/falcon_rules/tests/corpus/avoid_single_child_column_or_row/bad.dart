import 'package:flutter/material.dart';

Widget a() => Column(children: [Text('a')]); /* expect: avoid_single_child_column_or_row */

Widget b() => Row(children: [Icon(Icons.add)]); /* expect: avoid_single_child_column_or_row */

Widget c() =>
    Flex(direction: Axis.vertical, children: [SizedBox()]); /* expect: avoid_single_child_column_or_row */

Widget d() => Column(children: [const Divider()]); /* expect: avoid_single_child_column_or_row */

Widget e() => const Row(children: [Text('x')]); /* expect: avoid_single_child_column_or_row */
