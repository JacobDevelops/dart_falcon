import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void keepContainer(Widget child, BoxConstraints value, BoxConstraints? nullable, Widget? nullableChild, dynamic unknown) {
  Container(constraints: value);
  Container(child: child);
  Container(constraints: null, child: child);
  Container(constraints: nullable, child: child);
  Container(constraints: unknown, child: child);
  Container(constraints: value, child: null);
  Container(constraints: value, child: nullableChild);
  Container(constraints: value, width: 12, child: child);
  Container(constraints: value, decoration: const BoxDecoration(), child: child);
  Container(constraints: value, color: Colors.red, child: child);
}

Object parameterShadow(dynamic Container, Widget child, BoxConstraints value) =>
    Container(constraints: value, child: child);

Object prefixShadow(dynamic ui, Widget child, BoxConstraints value) =>
    ui.Container(constraints: value, child: child);

Object aliasShadow(dynamic LayoutContainer, Widget child, BoxConstraints value) =>
    LayoutContainer(constraints: value, child: child);

void localShadow(dynamic factory, Widget child, BoxConstraints value) {
  final Container = factory;
  Container(constraints: value, child: child);
}

void unusedTransformAlignment(Widget child, BoxConstraints value) {
  Container(constraints: value, transformAlignment: Alignment.center, child: child);
}
