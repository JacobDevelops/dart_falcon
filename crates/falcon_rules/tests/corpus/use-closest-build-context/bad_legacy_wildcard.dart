// @dart = 3.6
import 'package:flutter/material.dart';

void example(BuildContext outer) {
  final explicit = (BuildContext _) => Theme.of(outer); /* expect: use-closest-build-context */
  void Function(BuildContext) contextual = (_) => Theme.of(outer); /* expect: use-closest-build-context */
  void local(BuildContext _) { Theme.of(outer); } /* expect: use-closest-build-context */
}
