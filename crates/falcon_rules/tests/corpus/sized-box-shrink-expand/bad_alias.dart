// Analyzer-confirmed AdvB4 sb_paren_and_alias regression.
import 'package:flutter/widgets.dart';
typedef SB = SizedBox;
typedef Cont = Container;
List<Widget> f(Widget c, Decoration d) => [
  SizedBox(width: (0), height: 0),
  SB(width: 0, height: 0), /* expect: sized-box-shrink-expand */
  Cont(decoration: d, child: c),
];

Widget explicitNewConstructor() => SizedBox.new(width: 0, height: 0);
Widget explicitAliasConstructor() => SB.new(width: 0, height: 0);
Widget newKeywordConstructor() => new SB(width: 0, height: 0); /* expect: sized-box-shrink-expand */
Widget constKeywordConstructor() => const SB(width: 0, height: 0); /* expect: sized-box-shrink-expand */
