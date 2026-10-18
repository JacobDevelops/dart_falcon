import 'package:flutter/material.dart';
void f(BuildContext outer) {
 final callback = (BuildContext inner) { final nested = () { use(outer); }; }; /* expect: use-closest-build-context */
}
