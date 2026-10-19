import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef B = Border;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  Border.all(); /* expect: avoid-border-all */
  Border.all(width: 2); /* expect: avoid-border-all */
  new Border.all(); /* expect: avoid-border-all */
  f.Border.all(color: 0xff112233); /* expect: avoid-border-all */
  B.all(width: 3); /* expect: avoid-border-all */
}
