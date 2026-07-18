// Bad: a Container whose only argument is child.
import 'package:flutter/material.dart';

Widget b0() => Container(child: Text('a')); /* expect: avoid-unnecessary-containers */

Widget b1() => Container(child: SizedBox()); /* expect: avoid-unnecessary-containers */

Widget b2() {
  return Container( /* expect: avoid-unnecessary-containers */
    child: Text('x'),
  );
}

Widget b3() => Container(child: Icon(Icons.home)); /* expect: avoid-unnecessary-containers */

Widget b4() => Container(child: Padding(padding: EdgeInsets.zero)); /* expect: avoid-unnecessary-containers */

Widget b5() => Container(child: Text('last')); /* expect: avoid-unnecessary-containers */
