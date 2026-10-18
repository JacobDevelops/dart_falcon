import 'package:flutter/material.dart';
class S0 extends State<StatefulWidget> { void dispose() { super.dispose(); } }
class S1 extends State<StatefulWidget> { void dispose() => super.dispose(); }
class S2 extends State<StatefulWidget> { void dispose() { cleanup(); super.dispose(); } }
class S3 extends State<StatefulWidget> { void dispose() { try { cleanup(); } finally { super.dispose(); } } }
class Other { void dispose() {} }
