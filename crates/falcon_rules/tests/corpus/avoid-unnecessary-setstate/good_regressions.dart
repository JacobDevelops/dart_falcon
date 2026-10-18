import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 int x=0;
 Widget build(BuildContext context) { void onTap() { setState(() { x++; }); } return ElevatedButton(onPressed: onTap, child: Text('')); }
}
