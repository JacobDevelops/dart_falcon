import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef I = Image;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  Image(image: image); /* expect: avoid-missing-image-alt */
  Image.asset("a"); /* expect: avoid-missing-image-alt */
  new Image.network("a", semanticLabel: null); /* expect: avoid-missing-image-alt */
  const Image(excludeFromSemantics: false); /* expect: avoid-missing-image-alt */
  f.Image.memory(image); /* expect: avoid-missing-image-alt */
}
