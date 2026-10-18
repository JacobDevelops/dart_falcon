import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
 void update(Future<void> future) {
 future.then((_) { setState(() {}); }); /* expect: use-setstate-synchronously */
 Future.delayed(Duration.zero, () { setState(() {}); }); /* expect: use-setstate-synchronously */
 }
}
