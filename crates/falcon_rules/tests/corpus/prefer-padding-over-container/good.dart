import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void keepContainer(Widget child, EdgeInsetsGeometry value, EdgeInsetsGeometry? nullable, Widget? nullableChild, dynamic unknown) {
  Container(padding: value);
  Container(child: child);
  Container(padding: null, child: child);
  Container(padding: nullable, child: child);
  Container(padding: unknown, child: child);
  Container(padding: value, child: null);
  Container(padding: value, child: nullableChild);
  Container(padding: value, width: 12, child: child);
  Container(padding: value, decoration: const BoxDecoration(), child: child);
  Container(padding: value, color: Colors.red, child: child);
}

Object parameterShadow(dynamic Container, Widget child, EdgeInsetsGeometry value) =>
    Container(padding: value, child: child);

Object prefixShadow(dynamic ui, Widget child, EdgeInsetsGeometry value) =>
    ui.Container(padding: value, child: child);

Object aliasShadow(dynamic LayoutContainer, Widget child, EdgeInsetsGeometry value) =>
    LayoutContainer(padding: value, child: child);

void localShadow(dynamic factory, Widget child, EdgeInsetsGeometry value) {
  final Container = factory;
  Container(padding: value, child: child);
}

void combined(Widget child, EdgeInsetsGeometry value) {
  Container(padding: value, margin: value, child: child);
}

void unusedTransformAlignment(Widget child, EdgeInsetsGeometry value) {
  Container(padding: value, transformAlignment: Alignment.center, child: child);
}
