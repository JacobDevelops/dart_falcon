import 'package:flutter/material.dart';
class S extends State<StatefulWidget> { void update() {
setState(() { ; }); /* expect: avoid-empty-setstate */
setState(() { /* comment */ }); /* expect: avoid-empty-setstate */
this.setState(() {}); /* expect: avoid-empty-setstate */
setState((() {})); /* expect: avoid-empty-setstate */
setState(() { ; ; }); /* expect: avoid-empty-setstate */
}}
