import 'package:flutter/material.dart' as f;
import 'base.dart';

class Host extends Intermediate<f.StatefulWidget> {
  @override
  void initState() {
    final value = 1;
    super.initState(); /* expect: proper-super-init-state */
    print(value);
  }
}

class SuffixOnlyState {
  void initState() {
    print('not Flutter State');
  }
}
