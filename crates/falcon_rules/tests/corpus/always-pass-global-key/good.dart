import 'package:flutter/material.dart';
class Used extends State<Widget> {
  final _direct = GlobalKey();
  final _helper = GlobalKey();
  final _conditional = GlobalKey();
  final _inherited = GlobalKey();
  final GlobalKey injected;
  Used(this.injected);
  Widget first() => IconButton(key: this._direct);
  void second() => attach(_helper);
  void third(bool choose) => attach(choose ? _conditional : null);
}
class Child extends Used {
  Child(GlobalKey key) : super(key);
  void attachInherited() => attach(_inherited);
}
abstract class Base extends State<Widget> {
  final inherited = GlobalKey();
}
class Controller { final key = GlobalKey(); }
class Wrapper extends Widget { final key = GlobalKey(); }
void attach(Object? key) {}
void locals() { final local = GlobalKey(); }
