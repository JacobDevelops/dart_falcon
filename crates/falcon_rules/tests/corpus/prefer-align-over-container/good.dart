import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void keepContainer(Widget child, AlignmentGeometry value, AlignmentGeometry? nullable, Widget? nullableChild, dynamic unknown) {
  Container(alignment: value);
  Container(child: child);
  Container(alignment: null, child: child);
  Container(alignment: nullable, child: child);
  Container(alignment: unknown, child: child);
  Container(alignment: value, child: null);
  Container(alignment: value, child: nullableChild);
  Container(alignment: value, width: 12, child: child);
  Container(alignment: value, decoration: const BoxDecoration(), child: child);
  Container(alignment: value, color: Colors.red, child: child);
}

Object parameterShadow(dynamic Container, Widget child, AlignmentGeometry value) =>
    Container(alignment: value, child: child);

Object prefixShadow(dynamic ui, Widget child, AlignmentGeometry value) =>
    ui.Container(alignment: value, child: child);

Object aliasShadow(dynamic LayoutContainer, Widget child, AlignmentGeometry value) =>
    LayoutContainer(alignment: value, child: child);

void localShadow(dynamic factory, Widget child, AlignmentGeometry value) {
  final Container = factory;
  Container(alignment: value, child: child);
}

void unusedTransformAlignment(Widget child, AlignmentGeometry value) {
  Container(alignment: value, transformAlignment: Alignment.center, child: child);
}
