import 'package:flutter/material.dart';
import 'widget.dart';

class Host extends StatelessWidget {
  @override
  Widget build(BuildContext context) => CustomWidget.named(
    onTap: () { /* expect: prefer-extracting-callbacks */
      final now = DateTime.now();
      print(now);
    },
  );
}
