import 'package:flutter/material.dart';
class Nested extends State<StatefulWidget> {
  int value = 0;
  void update() {
    setState(() {
      void later() { if (!mounted) return; }
      final callback = () { if (!context.mounted) return; };
      if (mounted) value++; /* expect: avoid-mounted-in-setstate */
      final updateLater = () {
        if (!mounted) return;
        setState(() {
          if (mounted) value++; /* expect: avoid-mounted-in-setstate */
        });
      };
    });
  }
}
