import 'package:flutter/material.dart';

Widget shortChain(Widget child) => Padding(padding: EdgeInsets.all(8), child: Align(child: child));
Widget wrongOrder(Widget child, dynamic matrix) => Align(child: Padding(padding: EdgeInsets.all(8), child: Transform(transform: matrix, child: child)));
Widget innerKey(Widget child) => Padding(padding: EdgeInsets.all(8), child: ConstrainedBox(key: Key('keep'), constraints: BoxConstraints.tightFor(width: 20), child: Align(child: child)));
Widget sizeFactor(Widget child) => Padding(padding: EdgeInsets.all(8), child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: Align(widthFactor: 2, child: child)));
Widget hitTests(Widget child, dynamic matrix) => Transform(transform: matrix, transformHitTests: false, child: Padding(padding: EdgeInsets.all(8), child: Align(child: child)));
Widget decorated(Widget child) => DecoratedBox(decoration: BoxDecoration(border: Border.all()), child: Padding(padding: EdgeInsets.all(8), child: Align(child: child)));
Widget shadowed(dynamic Padding, Widget child) => Padding(padding: EdgeInsets.all(8), child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: Align(child: child)));
Widget separated(Widget child) => Padding(padding: EdgeInsets.all(8), child: Semantics(child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: Align(child: child))));
Widget nullableChild(Widget? child) => Padding(padding: EdgeInsets.all(8), child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: Align(child: child)));
final explicitConst = const Padding(padding: EdgeInsets.all(8), child: ColoredBox(color: Color(0xffff0000), child: Align(child: Text('x'))));
const implicitConst = Center(child: Padding(padding: EdgeInsets.all(8), child: ColoredBox(color: Color(0xffff0000), child: Align(child: Text('x')))));
final constList = const [Padding(padding: EdgeInsets.all(8), child: ColoredBox(color: Color(0xffff0000), child: Align(child: Text('x'))))];
Padding concreteType(Widget child) => Padding(padding: EdgeInsets.all(8), child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: Align(child: child)));
