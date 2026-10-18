import 'package:flutter/material.dart';
class A extends StatefulWidget { State<A> createState() => AState(); }
class AState extends State<A> { int count = 0; Widget build(BuildContext context) => Text('$count'); }
class B extends StatefulWidget { State<B> createState() => BState(); }
class BState extends State<B> { void initState() { super.initState(); } Widget build(BuildContext context) => Container(); }
class C extends StatefulWidget { State<C> createState() => CState(); }
class CState extends State<C> { void dispose() { super.dispose(); } Widget build(BuildContext context) => Container(); }
class D extends StatefulWidget { State<D> createState() => DState(); }
class DState extends State<D> { Widget build(BuildContext context) => Text('$mounted'); }
class E extends StatelessWidget { Widget build(BuildContext context) => Container(); }
class Unknown extends StatefulWidget { State<Unknown> createState() => externalState(); }
