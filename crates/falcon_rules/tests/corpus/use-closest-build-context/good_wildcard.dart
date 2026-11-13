import 'package:flutter/material.dart';

void example(BuildContext outer) {
  final explicit = (int _) => Theme.of(outer);
  void Function(int) contextual = (_) => Theme.of(outer);
  void local(int _) { Theme.of(outer); }
  void Function(BuildContext, BuildContext) own = (_, inner) => Theme.of(inner);
}
