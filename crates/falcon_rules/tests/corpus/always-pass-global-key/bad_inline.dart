import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;
typedef FormKey = GlobalKey;
class ConstCapableKey extends GlobalKey {
  const ConstCapableKey() : super.constructor();
}
class InlineKeys extends State<Widget> {
  Widget build() => IconButton(key: GlobalKey()); /* expect: always-pass-global-key */
  Widget prefixed() => f.IconButton.filled(key: f.GlobalKey()); /* expect: always-pass-global-key */
  Widget explicitNew() => IconButton(key: new GlobalKey()); /* expect: always-pass-global-key */
  Widget alias() => IconButton(key: (FormKey())); /* expect: always-pass-global-key */
  Widget subtype() => IconButton(key: LabeledGlobalKey('inline')); /* expect: always-pass-global-key */
  Widget form(Widget child) => Form(key: GlobalKey<FormState>(), child: child); /* expect: always-pass-global-key */
  Widget named() => (IconButton(key: GlobalKey.debug(debugLabel: 'inline'))); /* expect: always-pass-global-key */
  Widget nonconstant() => IconButton(key: ConstCapableKey()); /* expect: always-pass-global-key */
}
Widget outsideState() => IconButton(key: GlobalKey()); /* expect: always-pass-global-key */
Widget localWidget() {
  final app = IconButton(key: GlobalKey()); /* expect: always-pass-global-key */
  return app;
}
class CallbackWidget extends State<Widget> {
  final builder = () => IconButton(key: GlobalKey()); /* expect: always-pass-global-key */
}
final topLevelBuilder = () => IconButton(key: GlobalKey()); /* expect: always-pass-global-key */
