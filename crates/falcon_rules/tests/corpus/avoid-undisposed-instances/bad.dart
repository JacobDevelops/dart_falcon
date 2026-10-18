import 'package:flutter/material.dart';
class Resource { void dispose() {} }
void bad() {
 TextEditingController(); /* expect: avoid-undisposed-instances */
 ScrollController(); /* expect: avoid-undisposed-instances */
 FocusNode(); /* expect: avoid-undisposed-instances */
 Resource(); /* expect: avoid-undisposed-instances */
 new Resource(); /* expect: avoid-undisposed-instances */
}
void consume(Object value) {}
void discardedInsideCallback() {
 consume(() {
  Resource(); /* expect: avoid-undisposed-instances */
 });
}
void discardedArgumentReceivers() {
 print(ScrollController().initialScrollOffset); /* expect: avoid-undisposed-instances */
 consume(FocusNode().hasFocus); /* expect: avoid-undisposed-instances */
 Text('${Resource()}'); /* expect: avoid-undisposed-instances */
}
