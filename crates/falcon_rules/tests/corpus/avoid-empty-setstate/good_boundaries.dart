import 'package:flutter/material.dart';
class Other { void setState(void Function() f) {} void f() { setState(() {}); } }
class S extends State<StatefulWidget> { void f() { setState(() { print('updated'); }); } }
