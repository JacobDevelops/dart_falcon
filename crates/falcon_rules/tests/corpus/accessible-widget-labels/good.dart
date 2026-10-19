import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void actions(String label) {
  IconButton(tooltip: 'Add');
  f.IconButton.filled(tooltip: label);
  FloatingActionButton(child: Text('Add'));
  GestureDetector(onTap: () {}, child: Text('Open'));
  InkWell(onTap: () {}, child: Icon(null, semanticLabel: 'Open'));
  Semantics(label: 'Open', child: InkResponse(onTap: () {}));
  Tooltip(message: 'Add', child: IconButton());
  GestureDetector(excludeFromSemantics: true);
  TextButton(child: Text('Save'));
  GestureDetector();
  InkWell(onTap: null);
  InkResponse();
  IconButton(icon: customLabel());
  GestureDetector(onTap: () {}, child: customLabel());
}
void shadow(dynamic IconButton) { IconButton(); }
Widget customLabel() => Text('Custom label');

void alternatives(dynamic richMessage, dynamic attributedLabel) {
  Tooltip(richMessage: TextSpan(text: 'Delete'), child: InkWell(onTap: () {}, child: Icon(null)));
  Semantics(attributedLabel: AttributedString('Delete'), child: InkResponse(onTap: () {}, child: Icon(null)));
  f.Tooltip(richMessage: (richMessage), child: f.GestureDetector(onTap: () {}, child: f.Icon(null)));
  f.Semantics(attributedLabel: (attributedLabel), child: f.InkWell(onTap: () {}, child: f.Icon(null)));
  Tooltip(message: 'Delete', richMessage: null, child: InkWell(onTap: () {}));
  Semantics(label: 'Delete', attributedLabel: null, child: InkResponse(onTap: () {}));
}

void inactiveAdditionalGestures() {
  GestureDetector(onTapMove: (null));
  GestureDetector(onSecondaryTapCancel: null);
  GestureDetector(onTertiaryTapCancel: null);
  GestureDetector(onLongPressCancel: null);
  GestureDetector(onSecondaryLongPressDown: null);
  GestureDetector(onSecondaryLongPressCancel: null);
  GestureDetector(onSecondaryLongPress: ((null)));
  GestureDetector(onSecondaryLongPressStart: null);
  GestureDetector(onSecondaryLongPressMoveUpdate: null);
  GestureDetector(onSecondaryLongPressUp: null);
  GestureDetector(onSecondaryLongPressEnd: null);
  GestureDetector(onTertiaryLongPressDown: null);
  GestureDetector(onTertiaryLongPressCancel: null);
  GestureDetector(onTertiaryLongPress: null);
  GestureDetector(onTertiaryLongPressStart: null);
  GestureDetector(onTertiaryLongPressMoveUpdate: null);
  GestureDetector(onTertiaryLongPressUp: null);
  GestureDetector(onTertiaryLongPressEnd: null);
  GestureDetector(onVerticalDragDown: null);
  GestureDetector(onVerticalDragCancel: null);
  GestureDetector(onHorizontalDragDown: null);
  GestureDetector(onHorizontalDragCancel: null);
  GestureDetector(onForcePressStart: null);
  GestureDetector(onForcePressPeak: null);
  GestureDetector(onForcePressUpdate: null);
  GestureDetector(onForcePressEnd: null);
  GestureDetector(onPanCancel: null);
  InkWell(onSecondaryTapCancel: (null));
  InkResponse(onSecondaryTapCancel: null);
  InkWell(onHover: (_) {}, onFocusChange: (_) {}, onHighlightChanged: (_) {});
  InkResponse(onHover: (_) {}, onFocusChange: (_) {}, onHighlightChanged: (_) {});
  GestureDetector(onSecondaryLongPress: () {}, child: Text('Open'));
  Tooltip(message: 'Open', child: GestureDetector(onTapMove: (_) {}, onTap: () {}));
}

void inertRuntimeCallbacks(dynamic callback) {
  GestureDetector(onTapMove: callback);
  f.GestureDetector(onTapMove: (_) {});
  GestureDetector(onTapMove: callback, onTap: (null));
  InkWell(onSecondaryTapCancel: () {});
  InkResponse(onSecondaryTapCancel: (callback));
  InkWell(onTapCancel: callback);
  InkResponse(onTapCancel: () {});
  InkWell(onTapCancel: callback, onSecondaryTapCancel: callback);
  InkResponse(onTapCancel: callback, onSecondaryTapCancel: callback);
  InkWell(onTapCancel: callback, onSecondaryTapCancel: callback,
    onTap: null, onTapDown: null, onTapUp: null, onDoubleTap: null,
    onLongPress: null, onLongPressUp: null,
    onSecondaryTap: null, onSecondaryTapDown: (null), onSecondaryTapUp: null);
  InkResponse(onTapCancel: callback, onSecondaryTapCancel: callback,
    onTap: null, onTapDown: null, onTapUp: null, onDoubleTap: null,
    onLongPress: null, onLongPressUp: null,
    onSecondaryTap: null, onSecondaryTapDown: null, onSecondaryTapUp: (null));
}
