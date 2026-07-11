import 'package:flutter/material.dart';

Widget a() => Column(children: [Text('a')]); /* expect: avoid-single-child-column-or-row */

Widget b() => Row(children: [Icon(Icons.add)]); /* expect: avoid-single-child-column-or-row */

Widget c() =>
    Flex(direction: Axis.vertical, children: [SizedBox()]); /* expect: avoid-single-child-column-or-row */

Widget d() => Column(children: [const Divider()]); /* expect: avoid-single-child-column-or-row */

Widget e() => const Row(children: [Text('x')]); /* expect: avoid-single-child-column-or-row */
