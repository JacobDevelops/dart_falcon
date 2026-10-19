import 'package:flutter/widgets.dart' as f;
class GestureDetector { GestureDetector({dynamic onTap, dynamic child}); }
class TextButton { TextButton({dynamic onPressed}); }
void example() {
  GestureDetector(onTap: () {}, child: f.TextButton());
  f.GestureDetector(onTap: () {}, child: TextButton(onPressed: () {}));
  void localChild(dynamic TextButton) {
    f.GestureDetector(onTap: () {}, child: TextButton(onPressed: () {}));
  }
}
