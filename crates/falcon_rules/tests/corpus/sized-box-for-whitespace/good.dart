// Good: Containers with real layout properties, or non-Container widgets.
import 'package:flutter/material.dart';

Widget g0() => Container(color: Colors.red, width: 10);

Widget g1() => Container(child: Text('a'));

Widget g2() => Container();

Widget g3() => Container(padding: EdgeInsets.zero, height: 10);

Widget g4() => SizedBox(width: 10, height: 10);

Widget g5() => Container(width: 10, decoration: BoxDecoration());

Widget g6() => Container(alignment: Alignment.center, width: 5, child: Text('b'));
