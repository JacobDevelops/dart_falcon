import 'package:flutter/fade_in_image.dart';
void examples(dynamic image, dynamic placeholder) {
  FadeInImage(image: image, placeholder: placeholder); /* expect: avoid-missing-image-alt */
  FadeInImage.assetNetwork(image: 'image', placeholder: 'placeholder'); /* expect: avoid-missing-image-alt */
  FadeInImage(image: image, placeholder: placeholder, imageSemanticLabel: null); /* expect: avoid-missing-image-alt */
}
