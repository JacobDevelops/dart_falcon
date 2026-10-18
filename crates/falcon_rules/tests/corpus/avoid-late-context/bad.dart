import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
late final value0 = context; /* expect: avoid-late-context */
late BuildContext value1; /* expect: avoid-late-context */
late final value2 = this.context; /* expect: avoid-late-context */
late final value3 = Theme.of(context); /* expect: avoid-late-context */
late final BuildContext value4 = context; /* expect: avoid-late-context */
}
