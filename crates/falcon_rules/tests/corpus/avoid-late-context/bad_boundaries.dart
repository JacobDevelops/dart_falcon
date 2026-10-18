import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 late final value = Theme.of(this.context); /* expect: avoid-late-context */
 late BuildContext saved; /* expect: avoid-late-context */
}
