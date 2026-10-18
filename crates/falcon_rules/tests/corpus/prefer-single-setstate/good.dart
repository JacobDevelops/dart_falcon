import 'package:flutter/material.dart';
class S extends State<StatefulWidget> {
  int value = 0;
  void update() {
    setState(() { value++; value++; });
    value++;
    setState(() { value++; });
    if (mounted) setState(() { value++; });
    else setState(() { value--; });
    { setState(() { value++; }); }
    { setState(() { value++; }); }
  }
  void shadow() {
    void setState(void Function() callback) { callback(); }
    setState(() { value++; });
    setState(() { value++; });
  }
}
class Fake {
  void setState(void Function() callback) { callback(); }
  void update() { setState(() {}); setState(() {}); }
}
class Override extends S {
  void setState(void Function() callback) { callback(); }
  void update() { setState(() {}); setState(() {}); }
}
