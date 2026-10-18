import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 int x=0;
 Future<void> update() async { await work(); setState(() { x++; }); }
 void initState() { update(); final callback = () { setState(() { x++; }); }; }
 Widget build(BuildContext context) => ElevatedButton(onPressed: () { setState(() { x++; }); }, child: Text(''));
}
class Other { void setState(void Function() f) {} void initState() { setState(() {}); } }
