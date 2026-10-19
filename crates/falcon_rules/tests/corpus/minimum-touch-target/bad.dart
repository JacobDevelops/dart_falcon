import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void targets() {
  SizedBox(width: 47, child: IconButton()); /* expect: minimum-touch-target */
  SizedBox(height: 24.0, child: FloatingActionButton()); /* expect: minimum-touch-target */
  f.Container(width: 0, child: GestureDetector(onTap: () {})); /* expect: minimum-touch-target */
  new SizedBox(width: 32, height: 32, child: InkWell(onTap: () {})); /* expect: minimum-touch-target */
  const SizedBox(height: (47.5), child: TextButton()); /* expect: minimum-touch-target */
  Container(width: -1, child: ElevatedButton()); /* expect: minimum-touch-target */
  SizedBox(width: 43, child: CupertinoButton(onPressed: () {})); /* expect: minimum-touch-target */
}

void callbacks(dynamic callback) {
  SizedBox(width: 1, child: IconButton(onPressed: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: IconButton.filled(onPressed: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: FloatingActionButton.small(onPressed: (callback))); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: TextButton(onPressed: null, onLongPress: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: ElevatedButton(onPressed: (null), onLongPress: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: OutlinedButton(onPressed: () {}, onLongPress: null)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: RawMaterialButton(onPressed: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 43, child: CupertinoButton(onPressed: (callback))); /* expect: minimum-touch-target */
  SizedBox(width: 43, child: CupertinoButton(onPressed: null, onLongPress: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: TextButton(onLongPress: null)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTap: (callback))); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onTap: null, onLongPress: () {})); /* expect: minimum-touch-target */
}

void additionalGestures(dynamic callback) {
  SizedBox(width: 1, child: GestureDetector(onSecondaryTapCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryTapCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onLongPressCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPress: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressStart: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressMoveUpdate: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPressEnd: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPress: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressStart: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressMoveUpdate: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTertiaryLongPressEnd: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onVerticalDragDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onVerticalDragCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onHorizontalDragDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onHorizontalDragCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onForcePressStart: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onForcePressPeak: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onForcePressUpdate: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onForcePressEnd: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onPanCancel: callback)); /* expect: minimum-touch-target */
  f.SizedBox(width: 1, child: f.GestureDetector(onTapMove: (_) {}, onTap: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onSecondaryLongPress: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onSecondaryTapCancel: () {}, onSecondaryTapDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onSecondaryTapCancel: (callback), onSecondaryTap: () {})); /* expect: minimum-touch-target */
}

void inkEnablingCallbacks(dynamic callback) {
  SizedBox(width: 1, child: InkWell(onTap: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onTapDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onTapUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onDoubleTap: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onLongPress: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onLongPressUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onSecondaryTap: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onSecondaryTapDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onSecondaryTapUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onTap: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onTapDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onTapUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onDoubleTap: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onLongPress: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onLongPressUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onSecondaryTap: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onSecondaryTapDown: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onSecondaryTapUp: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTapCancel: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onTapCancel: callback, onTap: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkResponse(onTapCancel: () {}, onTapUp: callback)); /* expect: minimum-touch-target */
}
