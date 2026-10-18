import 'package:flutter/material.dart';
class A extends StatelessWidget { Widget build(BuildContext context) => A(); /* expect: avoid-recursive-widget-calls */ }
class B extends StatelessWidget { Widget build(BuildContext context) => new B(); /* expect: avoid-recursive-widget-calls */ }
class C extends StatelessWidget { Widget build(BuildContext context) => Container(child: C()); /* expect: avoid-recursive-widget-calls */ }
class D extends StatefulWidget { State<D> createState() => DState(); }
class DState extends State<D> { Widget build(BuildContext context) => D(); /* expect: avoid-recursive-widget-calls */ }
class E extends StatelessWidget { E.named(); Widget build(BuildContext context) => E.named(); /* expect: avoid-recursive-widget-calls */ }
