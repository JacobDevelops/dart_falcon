import 'package:flutter/material.dart';
class Other { void dispose() {} }
class S extends State<StatefulWidget> { void dispose() { cleanup(); super.dispose(); } }
class T extends State<StatefulWidget> { void dispose() => super.dispose(); }
