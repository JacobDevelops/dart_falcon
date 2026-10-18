import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 int x=0;
 Future<void> update() async {
  if (mounted) { await work(); setState(() { x++; }); } /* expect: use-setstate-synchronously */
 }
 Future<void> guarded() async { await work(); if (mounted) setState(() { x++; }); }
 Future<void> before() async { setState(() { x++; }); await work(); }
}
