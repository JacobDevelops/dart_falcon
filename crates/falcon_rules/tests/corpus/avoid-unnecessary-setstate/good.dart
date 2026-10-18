import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { int x=0;
void update() { setState(() { x++; }); }
void didChangeDependencies() { setState(() { x++; }); }
Widget build(BuildContext context) { return ElevatedButton(onPressed: () { setState(() { x++; }); }, child: Text('')); }
void initState() { Future.delayed(Duration.zero, () { setState(() { x++; }); }); }
}
class Other { void setState(Function fn) {} void initState() { setState(() {}); } }
