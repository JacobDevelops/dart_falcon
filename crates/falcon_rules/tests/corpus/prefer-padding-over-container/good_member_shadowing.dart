import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

class MemberFactory {
  Widget Container({EdgeInsetsGeometry? padding, Widget? child}) => child!;

  Widget build(Widget child, EdgeInsetsGeometry value) => Container(padding: value, child: child);
}

class MemberPrefix {
  final dynamic ui;
  MemberPrefix(this.ui);

  Widget build(Widget child, EdgeInsetsGeometry value) => ui.Container(padding: value, child: child);
}
