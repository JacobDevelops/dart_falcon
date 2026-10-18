import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 int x=0;
 void update() { setState(() { x++; }); }
 void indirect() { update(); }
 void initState() { indirect(); } /* expect: avoid-unnecessary-setstate */
 void didUpdateWidget(StatefulWidget old) { this.setState(() { x++; }); } /* expect: avoid-unnecessary-setstate */
 Widget build(BuildContext context) { update(); return Text(''); } /* expect: avoid-unnecessary-setstate */
}
