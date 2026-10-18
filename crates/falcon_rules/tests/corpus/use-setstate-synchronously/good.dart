import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { int x=0;
Future<void> update0() async { await work(); if (!mounted) return; setState(() { x++; }); }
Future<void> update1() async { await work(); if (mounted) { this.setState(() { x++; }); } }
void update2() { setState(() { x++; }); }
Future<void> update3() async { await work(); if (!this.mounted) return; setState(() { x++; }); }
Future<void> update4() async { await work(); if (!context.mounted) return; setState(() { x++; }); }
}
