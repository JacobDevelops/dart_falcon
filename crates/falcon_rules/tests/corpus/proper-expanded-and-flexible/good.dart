import 'package:flutter/material.dart';

Widget a() => Row(children: [Expanded(child: Text('a'))]);

Widget b() => Column(children: [Flexible(child: Text('b'))]);

Widget c() =>
    Flex(direction: Axis.horizontal, children: [Expanded(child: Text('c'))]);

Widget d() => Container(child: Text('d'));

Widget e() => Center(child: SizedBox(height: 8));

Widget f() => Row(children: [const Expanded(child: Text('f'))]);
