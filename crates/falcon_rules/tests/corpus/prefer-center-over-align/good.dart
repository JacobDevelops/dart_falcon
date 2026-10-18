import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutAlign = Align;

void examples(Widget child, AlignmentGeometry value) {
  Align(alignment: Alignment.topLeft, child: child);
  Align(alignment: Alignment(0, 1), child: child);
  Align(alignment: Alignment(1, 0), child: child);
  Align(alignment: value, child: child);
  Center(child: child);
}

Object parameterShadow(dynamic Align, Widget child) => Align(child: child);
Object prefixShadow(dynamic ui, Widget child) => ui.Align(child: child);
Object aliasShadow(dynamic LayoutAlign, Widget child) => LayoutAlign(child: child);

void localShadow(dynamic factory, Widget child) {
  final Align = factory;
  Align(child: child);
}

void alignmentShadow(dynamic Alignment, Widget child) {
  Align(alignment: Alignment.center, child: child);
  Align(alignment: Alignment(0, 0), child: child);
}

void alignmentPrefixShadow(dynamic ui, Widget child) {
  Align(alignment: ui.Alignment.center, child: child);
  Align(alignment: ui.Alignment(0, 0), child: child);
}
