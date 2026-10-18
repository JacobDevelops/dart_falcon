import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 int x=0;
 Future<void> update() async { await work(); if (!context.mounted) return; setState(() { x++; }); }
 void listen(Stream<int> stream) { stream.listen((value) { setState(() { x=value; }); }); }
}
