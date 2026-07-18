import 'package:flutter/material.dart';

Widget a() => Container(child: Expanded(child: Text('a'))); /* expect: proper-expanded-and-flexible */

Widget b() => Center(child: Flexible(child: Text('b'))); /* expect: proper-expanded-and-flexible */

Widget c() => SizedBox(child: Expanded(child: SizedBox())); /* expect: proper-expanded-and-flexible */

Widget d() => Padding(
      padding: EdgeInsets.zero,
      child: Expanded(child: Text('d')), /* expect: proper-expanded-and-flexible */
    );

Widget e() => const Align(child: Flexible(child: Text('e'))); /* expect: proper-expanded-and-flexible */
