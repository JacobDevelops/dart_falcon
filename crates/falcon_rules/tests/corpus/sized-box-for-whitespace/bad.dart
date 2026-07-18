// Bad: a Container used only for width/height whitespace.
import 'package:flutter/material.dart';

Widget b0() => Container(width: 10); /* expect: sized-box-for-whitespace */

Widget b1() => Container(height: 20); /* expect: sized-box-for-whitespace */

Widget b2() => Container(width: 10, height: 20); /* expect: sized-box-for-whitespace */

Widget b3() => Container(width: 10, child: Text('a')); /* expect: sized-box-for-whitespace */

Widget b4() => Container(height: 20, child: Text('b'), key: Key('k')); /* expect: sized-box-for-whitespace */

Widget b5() => Container(width: 5, height: 5, child: SizedBox()); /* expect: sized-box-for-whitespace */
