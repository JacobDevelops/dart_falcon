import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as ui;

class MemberFactory {
  Widget Align({Widget? child}) => child!;

  Widget build(Widget child) => Align(child: child);
}

class MemberAlignment {
  final dynamic Alignment;
  MemberAlignment(this.Alignment);

  Widget build(Widget child) => Align(alignment: Alignment.center, child: child);
}

class MemberPrefix {
  final dynamic ui;
  MemberPrefix(this.ui);

  Widget build(Widget child) => ui.Align(child: child);
}

class AlignmentPrefix {
  final dynamic ui;
  AlignmentPrefix(this.ui);

  Widget build(Widget child) => Align(alignment: ui.Alignment.center, child: child);
}
