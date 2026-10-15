// Analyzer-confirmed AdvB4 sb_paren_and_alias regression.
import 'package:flutter/widgets.dart';
typedef SB = SizedBox;
typedef Cont = Container;
List<Widget> f(Widget c, Decoration d) => [
  SizedBox(width: (0), height: 0),
  SB(width: 0, height: 0),
  Cont(decoration: d, child: c), /* expect: use-decorated-box */
];

Widget newKeywordConstructor(Widget c, Decoration d) =>
    new Cont(decoration: d, child: c); /* expect: use-decorated-box */
