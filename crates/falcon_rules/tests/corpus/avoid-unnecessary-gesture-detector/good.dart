import 'package:flutter/widgets.dart';
void callback() {}
void examples(Widget child, void Function()? handler) {
  GestureDetector(onTap: callback, child: child);
  GestureDetector(onLongPress: callback);
  GestureDetector(onDoubleTap: callback, child: TextButton());
  GestureDetector(onTap: handler);
  GestureDetector(onTap: null, onLongPress: callback);
}
