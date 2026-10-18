import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

class MemberFactory {
  Widget Container({BoxConstraints? constraints, Widget? child}) => child!;

  Widget build(Widget child, BoxConstraints value) => Container(constraints: value, child: child);
}

class MemberPrefix {
  final dynamic ui;
  MemberPrefix(this.ui);

  Widget build(Widget child, BoxConstraints value) => ui.Container(constraints: value, child: child);
}
