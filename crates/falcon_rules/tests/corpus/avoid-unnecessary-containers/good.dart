// Good: Containers with more than a child, or non-Container widgets.
import 'package:flutter/material.dart';

Widget g0() => Container(width: 10, child: Text('a'));

Widget g1() => Container(color: Colors.red, child: Text('b'));

Widget g2() => Container();

Widget g3() => Container(padding: EdgeInsets.zero, child: Text('c'));

Widget g4() => SizedBox(child: Text('d'));

Widget g5() => Center(child: Text('e'));

Widget g6() => Container(alignment: Alignment.center, child: Text('f'));
