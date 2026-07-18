// Good: child/children is last, absent, or the call is not a widget constructor.
import 'package:flutter/material.dart';

Widget g0() => Center(key: Key('k'), child: Text('a'));

Widget g1() => Column(key: Key('k'), children: [Text('a'), Text('b')]);

Widget g2() => Padding(padding: EdgeInsets.zero, child: Text('a'));

Widget g3() => Text('no child');

Widget g4() => SizedBox(width: 10, height: 10, child: Text('a'));

Widget g5() => Center(child: Text('a'));

int g6() => doWork(child: 5, other: 6);
