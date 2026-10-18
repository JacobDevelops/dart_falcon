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
