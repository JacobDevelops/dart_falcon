import 'package:flutter/material.dart';

final topLevel = Container(); /* expect: use-design-system-item */

@Example(const Container()) /* expect: use-design-system-item */
class Initializers {
  final field = Text('field'); /* expect: use-design-system-item */
  final Widget value;

  Initializers() : value = Scaffold(); /* expect: use-design-system-item */

  Widget operator +(Object other) => Container(); /* expect: use-design-system-item */
}

void defaults([Widget value = const Container()]) {} /* expect: use-design-system-item */

void statements() {
  for (var widget = Container(); ready; widget = Container()) {} /* expect: use-design-system-item */ /* expect: use-design-system-item */
  for (final widget in [Container()]) {} /* expect: use-design-system-item */
  assert(ready, Text('message')); /* expect: use-design-system-item */
  throw Container(); /* expect: use-design-system-item */
}

Iterable<Widget> widgets() sync* {
  yield Container(); /* expect: use-design-system-item */
}
