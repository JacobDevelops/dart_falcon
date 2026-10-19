import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void targets(double size) {
  SizedBox(width: 48, height: 48, child: IconButton());
  Container(width: 60, child: GestureDetector(onTap: () {}));
  SizedBox(width: size, child: InkWell());
  SizedBox(width: 16, child: Text('Decoration'));
  SizedBox(child: FloatingActionButton());
  IconButton();
  SizedBox(width: 16);
  SizedBox(width: 44, height: 44, child: CupertinoButton(onPressed: () {}));
  SizedBox(width: 1, child: GestureDetector());
  SizedBox(width: 1, child: InkWell(onTap: null));
  SizedBox(width: 1, child: InkResponse());
  SizedBox(width: 1, child: IconButton(onPressed: null));
  f.Container(height: 1, child: f.IconButton.filled(onPressed: (null)));
  SizedBox(width: 1, child: FloatingActionButton(onPressed: null));
  SizedBox(width: 1, child: FloatingActionButton.small(onPressed: (null)));
  SizedBox(width: 1, child: TextButton(onPressed: null));
  SizedBox(width: 1, child: ElevatedButton(onPressed: (null), onLongPress: null));
  SizedBox(width: 1, child: OutlinedButton(onPressed: null, onLongPress: (null)));
  SizedBox(width: 1, child: RawMaterialButton(onPressed: null));
  SizedBox(width: 1, child: CupertinoButton(onPressed: ((null))));
}
void shadow(dynamic SizedBox) { SizedBox(width: 1, child: IconButton()); }

void inactiveAdditionalGestures() {
  SizedBox(width: 1, child: GestureDetector(onTapMove: (null)));
  SizedBox(width: 1, child: GestureDetector(onSecondaryTapCancel: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryTapCancel: null));
  SizedBox(width: 1, child: GestureDetector(onLongPressCancel: null));
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressDown: null));
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressCancel: null));
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPress: ((null))));
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressStart: null));
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressMoveUpdate: null));
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressUp: null));
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressEnd: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressDown: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressCancel: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPress: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressStart: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressMoveUpdate: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressUp: null));
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressEnd: null));
  SizedBox(width: 1, child: GestureDetector(onVerticalDragDown: null));
  SizedBox(width: 1, child: GestureDetector(onVerticalDragCancel: null));
  SizedBox(width: 1, child: GestureDetector(onHorizontalDragDown: null));
  SizedBox(width: 1, child: GestureDetector(onHorizontalDragCancel: null));
  SizedBox(width: 1, child: GestureDetector(onForcePressStart: null));
  SizedBox(width: 1, child: GestureDetector(onForcePressPeak: null));
  SizedBox(width: 1, child: GestureDetector(onForcePressUpdate: null));
  SizedBox(width: 1, child: GestureDetector(onForcePressEnd: null));
  SizedBox(width: 1, child: GestureDetector(onPanCancel: null));
  SizedBox(width: 1, child: InkWell(onSecondaryTapCancel: (null)));
  SizedBox(width: 1, child: InkResponse(onSecondaryTapCancel: null));
  SizedBox(width: 1, child: InkWell(onHover: (_) {}, onFocusChange: (_) {}, onHighlightChanged: (_) {}));
  SizedBox(width: 1, child: InkResponse(onHover: (_) {}, onFocusChange: (_) {}, onHighlightChanged: (_) {}));
  SizedBox(width: 48, child: GestureDetector(onSecondaryLongPress: () {}));
  SizedBox(height: 48, child: GestureDetector(onTapMove: (_) {}, onTap: () {}));
}

void inertRuntimeCallbacks(dynamic callback) {
  SizedBox(width: 1, child: GestureDetector(onTapMove: callback));
  f.SizedBox(width: 1, child: f.GestureDetector(onTapMove: (_) {}));
  SizedBox(width: 1, child: GestureDetector(onTapMove: callback, onTap: (null)));
  SizedBox(width: 1, child: InkWell(onSecondaryTapCancel: () {}));
  SizedBox(width: 1, child: InkResponse(onSecondaryTapCancel: (callback)));
  SizedBox(width: 1, child: InkWell(onTapCancel: callback));
  SizedBox(width: 1, child: InkResponse(onTapCancel: () {}));
  SizedBox(width: 1, child: InkWell(onTapCancel: callback, onSecondaryTapCancel: callback));
  SizedBox(width: 1, child: InkResponse(onTapCancel: callback, onSecondaryTapCancel: callback));
  SizedBox(width: 1, child: InkWell(onTapCancel: callback, onSecondaryTapCancel: callback,
    onTap: null, onTapDown: null, onTapUp: null, onDoubleTap: null,
    onLongPress: null, onLongPressUp: null,
    onSecondaryTap: null, onSecondaryTapDown: (null), onSecondaryTapUp: null));
  SizedBox(width: 1, child: InkResponse(onTapCancel: callback, onSecondaryTapCancel: callback,
    onTap: null, onTapDown: null, onTapUp: null, onDoubleTap: null,
    onLongPress: null, onLongPressUp: null,
    onSecondaryTap: null, onSecondaryTapDown: null, onSecondaryTapUp: (null)));
}
