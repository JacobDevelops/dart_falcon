import 'package:flutter/material.dart';

Widget a() => Container(child: Expanded(child: Text('a'))); /* expect: proper_expanded_and_flexible */

Widget b() => Center(child: Flexible(child: Text('b'))); /* expect: proper_expanded_and_flexible */

Widget c() => SizedBox(child: Expanded(child: SizedBox())); /* expect: proper_expanded_and_flexible */

Widget d() => Padding(
      padding: EdgeInsets.zero,
      child: Expanded(child: Text('d')), /* expect: proper_expanded_and_flexible */
    );

Widget e() => const Align(child: Flexible(child: Text('e'))); /* expect: proper_expanded_and_flexible */
