import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as ui;

class MemberFactory {
  Widget Container({AlignmentGeometry? alignment, Widget? child}) => child!;

  Widget build(Widget child, AlignmentGeometry value) => Container(alignment: value, child: child);
}

class MemberPrefix {
  final dynamic ui;
  MemberPrefix(this.ui);

  Widget build(Widget child, AlignmentGeometry value) => ui.Container(alignment: value, child: child);
}
