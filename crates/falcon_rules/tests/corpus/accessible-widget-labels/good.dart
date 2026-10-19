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
