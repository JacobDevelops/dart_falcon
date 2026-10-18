import 'dart:async';
import 'package:flutter/material.dart' as f;
class Flag { bool get mounted => true; }
class S extends f.State<f.StatefulWidget> {
  int value = 0;
  void subscribe(Stream<int> stream) {
    setState(() {
      stream.listen((event) {
        if (!mounted) return;
        value = event;
      });
      void later() {
        if (!context.mounted) return;
        value++;
      }
      value++;
    });
  }
  void update(f.BuildContext other, Flag flag) {
    if (mounted) setState(() { value++; });
    if (other.mounted) this.setState(() { value++; });
    if (!mounted) return;
    setState(() { value++; });
    setState(() {
      bool mounted = true;
      if (mounted) value++;
      if (flag.mounted) value++;
    });
    void setState(void Function() callback) { callback(); }
    setState(() { if (this.mounted) value++; });
  }
}
class Overridden extends S {
  bool get mounted => true;
  void updateFlag() { setState(() { if (mounted) value++; }); }
}
class Fake {
  bool mounted = true;
  void setState(void Function() callback) { callback(); }
  void update() { setState(() { if (mounted) return; }); }
}
