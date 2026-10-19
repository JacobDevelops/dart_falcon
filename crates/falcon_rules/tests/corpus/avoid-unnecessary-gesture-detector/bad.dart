import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef Detector = GestureDetector;
void examples(Widget child) {
  GestureDetector(); /* expect: avoid-unnecessary-gesture-detector */
  GestureDetector(child: child); /* expect: avoid-unnecessary-gesture-detector */
  f.GestureDetector(child: TextButton()); /* expect: avoid-unnecessary-gesture-detector */
  Detector(onTap: null, child: child); /* expect: avoid-unnecessary-gesture-detector */
  new GestureDetector(onLongPress: null); /* expect: avoid-unnecessary-gesture-detector */
  GestureDetector(onTap: () {}, child: TextButton(onPressed: () {})); /* expect: avoid-unnecessary-gesture-detector, msg: "Handle taps in the button's onPressed callback instead of wrapping it in GestureDetector." */
  GestureDetector(onTap: () {}, child: f.ElevatedButton(onPressed: () {})); /* expect: avoid-unnecessary-gesture-detector */
  Detector(onTap: () {}, child: OutlinedButton(onPressed: () {})); /* expect: avoid-unnecessary-gesture-detector */
  GestureDetector(onTap: () {}, child: IconButton(onPressed: () {})); /* expect: avoid-unnecessary-gesture-detector */
  GestureDetector(behavior: null, child: child); /* expect: avoid-unnecessary-gesture-detector */
}
