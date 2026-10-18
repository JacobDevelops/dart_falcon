import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

typedef LayoutContainer = Container;

void keepContainer(Widget child, Matrix4 value, Matrix4? nullable, Widget? nullableChild, dynamic unknown) {
  Container(transform: value);
  Container(child: child);
  Container(transform: null, child: child);
  Container(transform: nullable, child: child);
  Container(transform: unknown, child: child);
  Container(transform: value, child: null);
  Container(transform: value, child: nullableChild);
  Container(transform: value, width: 12, child: child);
  Container(transform: value, decoration: const BoxDecoration(), child: child);
  Container(transform: value, color: Colors.red, child: child);
}

Object parameterShadow(dynamic Container, Widget child, Matrix4 value) =>
    Container(transform: value, child: child);

Object prefixShadow(dynamic ui, Widget child, Matrix4 value) =>
    ui.Container(transform: value, child: child);

Object aliasShadow(dynamic LayoutContainer, Widget child, Matrix4 value) =>
    LayoutContainer(transform: value, child: child);

void localShadow(dynamic factory, Widget child, Matrix4 value) {
  final Container = factory;
  Container(transform: value, child: child);
}
