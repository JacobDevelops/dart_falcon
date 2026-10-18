import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { int x=0;
void initState() { setState(() { x++; }); } /* expect: avoid-unnecessary-setstate */
Widget build(BuildContext context) { setState(() { x++; }); return Text(''); } /* expect: avoid-unnecessary-setstate */
void didUpdateWidget(StatefulWidget old) { setState(() { x++; }); } /* expect: avoid-unnecessary-setstate */
}
class Explicit extends State<StatefulWidget> { void initState() { this.setState(() {}); } } /* expect: avoid-unnecessary-setstate */
class Helpers extends State<StatefulWidget> { void update() { setState(() {}); } void initState() { update(); } } /* expect: avoid-unnecessary-setstate */
