import 'package:flutter/material.dart';
class A extends StatefulWidget { State<A> createState() => AState(); } /* expect: avoid-unnecessary-stateful-widgets */
class AState extends State<A> { Widget build(BuildContext context) => Container(); }
class B extends StatefulWidget { State<B> createState() => BState(); } /* expect: avoid-unnecessary-stateful-widgets */
class BState extends State<B> { Widget build(BuildContext context) => Text('B'); }
class C extends StatefulWidget { State<C> createState() => CState(); } /* expect: avoid-unnecessary-stateful-widgets */
class CState extends State<C> { Widget build(BuildContext context) { return Container(); } }
class D extends StatefulWidget { State<D> createState() => DState(); } /* expect: avoid-unnecessary-stateful-widgets */
class DState extends State<D> { Widget build(BuildContext context) => Row(children: []); }
class E extends StatefulWidget { State<E> createState() => EState(); } /* expect: avoid-unnecessary-stateful-widgets */
class EState extends State<E> { Widget build(BuildContext context) => Column(children: []); }
