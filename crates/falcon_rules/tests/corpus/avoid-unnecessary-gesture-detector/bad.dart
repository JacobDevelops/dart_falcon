import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef Detector = GestureDetector;
void examples(Widget child) {
  GestureDetector(); /* expect: avoid-unnecessary-gesture-detector */
  GestureDetector(child: child); /* expect: avoid-unnecessary-gesture-detector */
  f.GestureDetector(child: TextButton()); /* expect: avoid-unnecessary-gesture-detector */
  Detector(onTap: null, child: child); /* expect: avoid-unnecessary-gesture-detector */
  new GestureDetector(onLongPress: null); /* expect: avoid-unnecessary-gesture-detector */
}
