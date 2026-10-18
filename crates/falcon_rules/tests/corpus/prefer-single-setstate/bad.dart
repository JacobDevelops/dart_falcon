import 'package:flutter/material.dart' as f;
class S extends f.State<f.StatefulWidget> {
  int value = 0;
  void update() {
    setState(() { value++; });
    setState(() { value++; }); /* expect: prefer-single-setstate */
    this.setState(() { value++; }); /* expect: prefer-single-setstate */
    value++;
    this.setState(() { value++; });
    (setState(() { value++; })); /* expect: prefer-single-setstate */
    if (mounted) {
      setState(() => value++);
      setState(() => value++); /* expect: prefer-single-setstate */
    }
    void nested() {
      setState(() { value++; });
      setState(() { value++; }); /* expect: prefer-single-setstate */
    }
  }
}
class Child extends S {
  void inherited() {
    setState(() { value++; });
    setState(() { value++; }); /* expect: prefer-single-setstate */
  }
}
