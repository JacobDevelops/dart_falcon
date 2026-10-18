import 'package:flutter/material.dart';
void example(BuildContext outer) {
final callback0 = (BuildContext inner) { use(inner); };
final callback1 = (BuildContext inner) => use(inner);
final callback2 = (BuildContext inner) { return Theme.of(inner); };
void callback3(BuildContext inner) { use(inner); }
final callback4 = (int value) { use(outer); };
}
