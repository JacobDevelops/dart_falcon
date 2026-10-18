import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

class MemberFactory {
  Widget Container({Matrix4? transform, Widget? child}) => child!;

  Widget build(Widget child, Matrix4 value) => Container(transform: value, child: child);
}

class MemberPrefix {
  final dynamic ui;
  MemberPrefix(this.ui);

  Widget build(Widget child, Matrix4 value) => ui.Container(transform: value, child: child);
}
