import 'package:flutter/widgets.dart';
void callback() {}
void examples(Widget child, void Function()? handler) {
  GestureDetector(onTap: callback, child: child);
  GestureDetector(onLongPress: callback);
  GestureDetector(onDoubleTap: callback, child: TextButton());
  GestureDetector(onTap: handler);
  GestureDetector(onTap: null, onLongPress: callback);
  GestureDetector(behavior: HitTestBehavior.opaque, child: SizedBox.expand());
  GestureDetector(behavior: HitTestBehavior.translucent, child: child);
  GestureDetector(onLongPress: callback, child: TextButton(onPressed: () {}));
  GestureDetector(onDoubleTap: callback, child: TextButton(onPressed: () {}));
  GestureDetector(onTap: callback, child: TextButton(onPressed: null));
  GestureDetector(onTap: callback, child: TextButton(onPressed: handler));
  GestureDetector(onTap: callback, onLongPress: callback, child: TextButton(onPressed: () {}));
  GestureDetector(behavior: HitTestBehavior.opaque, onTap: callback, child: TextButton(onPressed: () {}));
}
