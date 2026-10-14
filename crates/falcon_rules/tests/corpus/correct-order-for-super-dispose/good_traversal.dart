import 'package:flutter/material.dart';

class NestedDispose extends ChangeNotifier {
  void dispose() {
    void dispose() {
      super.dispose();
      print('callback cleanup');
    }
    if (true) {
      super.dispose();
      print('nested cleanup');
    }
    dispose();
    super.dispose();
  }

  void close() {
    super.dispose();
    print('not a dispose method');
  }
}
