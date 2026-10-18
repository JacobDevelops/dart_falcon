import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget transformed(Widget child, dynamic matrix) => Transform(transform: matrix, child: Padding(padding: EdgeInsets.all(8), child: Align(alignment: Alignment.center, child: child))); /* expect: prefer-container */
Widget constrained(Widget child) => Padding(padding: EdgeInsets.all(8), child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: Align(alignment: Alignment.topLeft, child: child))); /* expect: prefer-container */
Widget colored(Widget child) => ColoredBox(color: Colors.red, child: Padding(padding: EdgeInsets.all(8), child: Align(child: child))); /* expect: prefer-container */
Widget prefixed(Widget child) => m.Padding(padding: EdgeInsets.all(8), child: m.ConstrainedBox(constraints: BoxConstraints.tightFor(height: 20), child: m.Align(child: child))); /* expect: prefer-container */
Widget explicitNew(Widget child) => new Padding(padding: EdgeInsets.all(8), child: new ColoredBox(color: Colors.blue, child: new Align(child: child))); /* expect: prefer-container */
Widget longChain(Widget child, dynamic matrix) => Transform(transform: matrix, child: Padding(padding: EdgeInsets.all(8), child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: ColoredBox(color: Colors.red, child: Padding(padding: EdgeInsets.all(4), child: Align(child: child)))))); /* expect: prefer-container */
Widget grouped(Widget child) => (Padding(padding: EdgeInsets.all(8), child: (ConstrainedBox(constraints: BoxConstraints.tightFor(height: 20), child: (Align(child: child)))))); /* expect: prefer-container */
Widget alignedTransform(Widget child, Matrix4 matrix) => Transform(alignment: Alignment.center, transform: matrix, child: Padding(padding: EdgeInsets.all(8), child: Align(child: child))); /* expect: prefer-container */
Widget doublePadding(Widget child) => Padding(padding: EdgeInsets.all(8), child: Padding(padding: EdgeInsets.all(4), child: Align(child: child))); /* expect: prefer-container */
Widget outerKey(Widget child, Key key) => Padding(key: key, padding: EdgeInsets.all(8), child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 20), child: Align(child: child))); /* expect: prefer-container */
