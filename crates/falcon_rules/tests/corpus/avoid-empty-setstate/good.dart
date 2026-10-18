import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { int x = 0; void update() {
setState(() { x++; });
setState(() => x++);
this.setState(() { x = 2; });
setState(() { update(); });
setState(() { if (x == 0) x = 1; });
}}
