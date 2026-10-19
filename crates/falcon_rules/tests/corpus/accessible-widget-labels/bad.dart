import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void actions() {
  GestureDetector(onTap: () {}); /* expect: accessible-widget-labels */
  f.InkWell(onTap: () {}, child: Icon(null)); /* expect: accessible-widget-labels */
  InkResponse(onTap: () {}, child: Text(' ')); /* expect: accessible-widget-labels */
  GestureDetector(onTap: () {}, child: Icon(null)); /* expect: accessible-widget-labels */
  InkWell(onTap: () {}, child: Text('')); /* expect: accessible-widget-labels */
  InkResponse(onTap: () {}); /* expect: accessible-widget-labels */
  Tooltip(message: '', child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Tooltip(richMessage: null, child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Tooltip(richMessage: (null), child: InkResponse(onTap: () {})); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: null, child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: (null), child: InkResponse(onTap: () {})); /* expect: accessible-widget-labels */
}

void shadowed(dynamic Tooltip, dynamic Semantics) {
  Tooltip(richMessage: 'Delete', child: InkWell(onTap: () {})); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: 'Delete', child: InkResponse(onTap: () {})); /* expect: accessible-widget-labels */
}

void siblings(dynamic richMessage, dynamic attributedLabel) {
  Tooltip(richMessage: richMessage, child: InkWell(onTap: () {}));
  InkWell(onTap: () {}); /* expect: accessible-widget-labels */
  Semantics(attributedLabel: attributedLabel, child: InkResponse(onTap: () {}));
  InkResponse(onTap: () {}); /* expect: accessible-widget-labels */
}

void additionalGestures(dynamic callback) {
  GestureDetector(onSecondaryTapCancel: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryTapCancel: callback); /* expect: accessible-widget-labels */
  GestureDetector(onLongPressCancel: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPressDown: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPressCancel: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPress: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPressStart: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPressMoveUpdate: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPressUp: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPressEnd: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryLongPressDown: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryLongPressCancel: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryLongPress: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryLongPressStart: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryLongPressMoveUpdate: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryLongPressUp: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTertiaryLongPressEnd: callback); /* expect: accessible-widget-labels */
  GestureDetector(onVerticalDragDown: callback); /* expect: accessible-widget-labels */
  GestureDetector(onVerticalDragCancel: callback); /* expect: accessible-widget-labels */
  GestureDetector(onHorizontalDragDown: callback); /* expect: accessible-widget-labels */
  GestureDetector(onHorizontalDragCancel: callback); /* expect: accessible-widget-labels */
  GestureDetector(onForcePressStart: callback); /* expect: accessible-widget-labels */
  GestureDetector(onForcePressPeak: callback); /* expect: accessible-widget-labels */
  GestureDetector(onForcePressUpdate: callback); /* expect: accessible-widget-labels */
  GestureDetector(onForcePressEnd: callback); /* expect: accessible-widget-labels */
  GestureDetector(onPanCancel: callback); /* expect: accessible-widget-labels */
  f.GestureDetector(onTapMove: (_) {}, onTap: callback); /* expect: accessible-widget-labels */
  GestureDetector(onSecondaryLongPress: () {}); /* expect: accessible-widget-labels */
  InkWell(onSecondaryTapCancel: () {}, onSecondaryTapDown: callback); /* expect: accessible-widget-labels */
  InkResponse(onSecondaryTapCancel: (callback), onSecondaryTap: () {}); /* expect: accessible-widget-labels */
}

void inkEnablingCallbacks(dynamic callback) {
  InkWell(onTap: callback); /* expect: accessible-widget-labels */
  InkWell(onTapDown: callback); /* expect: accessible-widget-labels */
  InkWell(onTapUp: callback); /* expect: accessible-widget-labels */
  InkWell(onDoubleTap: callback); /* expect: accessible-widget-labels */
  InkWell(onLongPress: callback); /* expect: accessible-widget-labels */
  InkWell(onLongPressUp: callback); /* expect: accessible-widget-labels */
  InkWell(onSecondaryTap: callback); /* expect: accessible-widget-labels */
  InkWell(onSecondaryTapDown: callback); /* expect: accessible-widget-labels */
  InkWell(onSecondaryTapUp: callback); /* expect: accessible-widget-labels */
  InkResponse(onTap: callback); /* expect: accessible-widget-labels */
  InkResponse(onTapDown: callback); /* expect: accessible-widget-labels */
  InkResponse(onTapUp: callback); /* expect: accessible-widget-labels */
  InkResponse(onDoubleTap: callback); /* expect: accessible-widget-labels */
  InkResponse(onLongPress: callback); /* expect: accessible-widget-labels */
  InkResponse(onLongPressUp: callback); /* expect: accessible-widget-labels */
  InkResponse(onSecondaryTap: callback); /* expect: accessible-widget-labels */
  InkResponse(onSecondaryTapDown: callback); /* expect: accessible-widget-labels */
  InkResponse(onSecondaryTapUp: callback); /* expect: accessible-widget-labels */
  GestureDetector(onTapCancel: callback); /* expect: accessible-widget-labels */
  InkWell(onTapCancel: callback, onTap: () {}); /* expect: accessible-widget-labels */
  InkResponse(onTapCancel: () {}, onTapUp: callback); /* expect: accessible-widget-labels */
}
