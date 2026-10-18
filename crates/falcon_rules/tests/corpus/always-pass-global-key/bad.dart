import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;
typedef FormKey = GlobalKey;
class Missing extends State<Widget> {
  final _first = GlobalKey(); /* expect: always-pass-global-key */
  final GlobalKey _second = GlobalKey(); /* expect: always-pass-global-key */
  final _third = f.GlobalKey(); /* expect: always-pass-global-key */
  final _fourth = LabeledGlobalKey('fourth'); /* expect: always-pass-global-key */
  final _fifth = FormKey(); /* expect: always-pass-global-key */
  final _shadowed = GlobalKey(); /* expect: always-pass-global-key */
  final _stateRead = GlobalKey(); /* expect: always-pass-global-key */
  final _contextRead = GlobalKey(); /* expect: always-pass-global-key */
  void inspect() {
    observe(_stateRead.currentState);
    observe(this._contextRead.currentContext);
  }
  Widget build() {
    final _shadowed = GlobalKey();
    return IconButton(key: _shadowed);
  }
}
void observe(Object? value) {}
