// Bad: the child/children argument is not last in a widget constructor call.
import 'package:flutter/material.dart';

Widget b0() => Center(child: Text('a'), key: Key('k')); /* expect: sort-child-properties-last */

Widget b1() => Column(children: [Text('a'), Text('b')], key: Key('k')); /* expect: sort-child-properties-last */

Widget b2() => Padding(child: Text('a'), padding: EdgeInsets.zero); /* expect: sort-child-properties-last */

Widget b3() => Container(child: Text('a'), width: 10); /* expect: sort-child-properties-last */

Widget b4() => SizedBox(child: Text('a'), width: 10, height: 10); /* expect: sort-child-properties-last */
