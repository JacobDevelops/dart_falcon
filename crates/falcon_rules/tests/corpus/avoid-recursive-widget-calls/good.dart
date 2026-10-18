import 'package:flutter/material.dart';
class A extends StatelessWidget {
 Widget build(BuildContext context) => Container();
 A copy() => A();
}
class B extends StatelessWidget { Widget build(BuildContext context) => A(); }
class C extends StatefulWidget { State<C> createState() => CState(); }
class CState extends State<C> { Widget build(BuildContext context) => A(); }
class Other { Object build() => Other(); }
Object create() => A();
class Tree extends StatelessWidget {
 Widget build(BuildContext context) => Builder(builder: (_) => Tree());
}
class Nested extends StatelessWidget {
 final int depth = 0;
 Widget build(BuildContext context) {
   if (depth > 0) return Nested();
   return Container();
 }
}
class Forest extends StatelessWidget {
 final List<int> children = [];
 Widget build(BuildContext context) => Column(children: [
   for (final child in children) Forest(),
 ]);
}
class Shadow extends StatelessWidget {
 Widget build(BuildContext context) {
   final Shadow = () => Container();
   return Shadow();
 }
}
