import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef S = SizedBox;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  SizedBox(width: 10, height: 10); /* expect: prefer-sized-box-square */
  const SizedBox(width: 20, height: 20); /* expect: prefer-sized-box-square */
  new SizedBox(width: size, height: size); /* expect: prefer-sized-box-square */
  f.SizedBox(width: 2.5, height: 2.5); /* expect: prefer-sized-box-square */
  S(width: 4, height: 4, child: child); /* expect: prefer-sized-box-square */
}
