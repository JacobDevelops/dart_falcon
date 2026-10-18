import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef I = Image;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  Opacity(opacity: .5, child: Image(image: image)); /* expect: avoid-incorrect-image-opacity */
  Opacity(opacity: .2, child: Image.asset("a")); /* expect: avoid-incorrect-image-opacity */
  const Opacity(opacity: .5, child: Image()); /* expect: avoid-incorrect-image-opacity */
  f.Opacity(opacity: .5, child: f.Image.network("a")); /* expect: avoid-incorrect-image-opacity */
  new Opacity(opacity: .5, child: I.asset("a")); /* expect: avoid-incorrect-image-opacity */
}
