import 'package:flutter/material.dart';

final topLevel = Expanded(child: SizedBox()); /* expect: use-spacer-as-expanded-child */

@Example(const Expanded(child: SizedBox())) /* expect: use-spacer-as-expanded-child */
class Initializers {
  final Widget value;

  Initializers() : value = Expanded(child: Container()); /* expect: use-spacer-as-expanded-child */

  Widget operator +(Object other) => Expanded(child: SizedBox()); /* expect: use-spacer-as-expanded-child */
}

void defaults([Widget value = const Expanded(child: SizedBox())]) {} /* expect: use-spacer-as-expanded-child */

void statements() {
  if (accept(Expanded(child: SizedBox()))) {} /* expect: use-spacer-as-expanded-child */
  while (accept(Expanded(child: Container()))) {} /* expect: use-spacer-as-expanded-child */
  for (var widget = Expanded(child: SizedBox()); ready; widget = Expanded(child: Container())) {} /* expect: use-spacer-as-expanded-child */ /* expect: use-spacer-as-expanded-child */
  for (final widget in [Expanded(child: SizedBox())]) {} /* expect: use-spacer-as-expanded-child */
  try {} catch (error) {
    consume(Expanded(child: SizedBox())); /* expect: use-spacer-as-expanded-child */
  } finally {
    consume(Expanded(child: Container())); /* expect: use-spacer-as-expanded-child */
  }
  switch (value) {
    case 0:
      consume(Expanded(child: SizedBox())); /* expect: use-spacer-as-expanded-child */
  }
  assert(ready, Expanded(child: SizedBox())); /* expect: use-spacer-as-expanded-child */
  Widget local() => Expanded(child: SizedBox()); /* expect: use-spacer-as-expanded-child */
  final callback = () => Expanded(child: Container()); /* expect: use-spacer-as-expanded-child */
  final record = (Expanded(child: SizedBox()),); /* expect: use-spacer-as-expanded-child */
}

Future<Widget> awaited() async => await wrap(Expanded(child: SizedBox())); /* expect: use-spacer-as-expanded-child */
