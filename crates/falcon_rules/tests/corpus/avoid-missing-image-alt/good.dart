import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef I = Image;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  Image(semanticLabel: "Photo");
  Image.asset("a", semanticLabel: label);
  Image.network("a", excludeFromSemantics: true);
  const Image(semanticLabel: "Icon");
  I.memory(image, semanticLabel: "Preview");
}
