import 'package:flutter/material.dart' as f;
class S extends f.State<f.StatefulWidget> {
  int value = 0;
  void update(f.BuildContext other) {
    setState(() {
      if (mounted) value++; /* expect: avoid-mounted-in-setstate */
    });
    this.setState(() {
      if (this.mounted) value++; /* expect: avoid-mounted-in-setstate */
    });
    setState(() {
      if (context.mounted) value++; /* expect: avoid-mounted-in-setstate */
    });
    setState((() {
      if (!other.mounted) return; /* expect: avoid-mounted-in-setstate */
      value++;
    }));
    setState(() => value = mounted ? 1 : 0); /* expect: avoid-mounted-in-setstate */
  }
}
class Child extends S {
  void inherited() {
    setState(() { if (mounted) value++; }); /* expect: avoid-mounted-in-setstate */
  }
}
