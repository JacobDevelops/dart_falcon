import 'package:flutter/fade_in_image.dart';
void examples(dynamic image, dynamic placeholder, String label, bool decorative) {
  FadeInImage(image: image, placeholder: placeholder, imageSemanticLabel: label);
  FadeInImage.assetNetwork(image: 'image', placeholder: 'placeholder', imageSemanticLabel: 'Photo');
  FadeInImage(image: image, placeholder: placeholder, excludeFromSemantics: true);
  FadeInImage(image: image, placeholder: placeholder, excludeFromSemantics: decorative);
}
