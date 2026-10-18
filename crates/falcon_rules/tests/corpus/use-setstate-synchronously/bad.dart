import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { int x=0;
Future<void> update0() async { await work(); setState(() { x++; }); } /* expect: use-setstate-synchronously */
Future<void> update1() async { await work(); this.setState(() { x++; }); } /* expect: use-setstate-synchronously */
Future<void> update2() async { if (mounted) { await work(); setState(() { x++; }); } } /* expect: use-setstate-synchronously */
Future<void> update3() async { await work(); if (!mounted) return; await work(); setState(() { x++; }); } /* expect: use-setstate-synchronously */
Future<void> update4() async { try { await work(); } finally { setState(() { x++; }); } } /* expect: use-setstate-synchronously */
}
