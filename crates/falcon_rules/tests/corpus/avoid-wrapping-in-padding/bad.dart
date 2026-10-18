import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef P = Padding;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  Padding(padding: 1, child: Container(child: child)); /* expect: avoid-wrapping-in-padding */
  Padding(padding: 2, child: Container(key: image, child: child)); /* expect: avoid-wrapping-in-padding */
  new Padding(padding: 3, child: Container()); /* expect: avoid-wrapping-in-padding */
  f.Padding(padding: 4, child: f.Container(child: child)); /* expect: avoid-wrapping-in-padding */
  P(padding: 5, child: Container(child: child)); /* expect: avoid-wrapping-in-padding */
}
