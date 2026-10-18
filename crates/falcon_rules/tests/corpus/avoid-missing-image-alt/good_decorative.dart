import 'package:flutter/widgets.dart';
void examples(dynamic image, bool decorative) {
  Image(image: image, excludeFromSemantics: decorative);
  Image(image: image, semanticLabel: '');
}
