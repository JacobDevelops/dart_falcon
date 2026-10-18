import 'package:flutter/material.dart';
class Other { void setState(void Function() callback) {} Future<void> f() async { await work(); setState(() {}); } }
class S extends State<StatefulWidget> { int x=0; Future<void> f() async { await work(); if (!mounted) { return; } this.setState(() { x++; }); } }
