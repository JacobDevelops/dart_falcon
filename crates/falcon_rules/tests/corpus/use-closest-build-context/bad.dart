import 'package:flutter/material.dart';
void example(BuildContext outer) {
final callback0 = (BuildContext inner) { use(outer); }; /* expect: use-closest-build-context */
final callback1 = (BuildContext inner) => use(outer); /* expect: use-closest-build-context */
final callback2 = (BuildContext inner) { return Theme.of(outer); }; /* expect: use-closest-build-context */
void callback3(BuildContext inner) { use(outer); } /* expect: use-closest-build-context */
final callback4 = (BuildContext inner) { final nested = (BuildContext closest) { use(inner); }; }; /* expect: use-closest-build-context */
}
