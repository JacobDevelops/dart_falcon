import 'package:flutter/material.dart' as f;
import 'base.dart';

class IndirectState extends Intermediate<f.StatefulWidget> {
  @override
  void initState() {
    print('setup');
    super.initState(); /* expect: proper-super-calls */
  }

  @override
  void deactivate() {
    print('cleanup');
    super.deactivate();
  }
}

mixin IndirectCleanup<T extends f.StatefulWidget> on Intermediate<T> {
  @override
  void dispose() {
    super.dispose(); /* expect: proper-super-calls */
    print('cleanup');
  }

  @override
  void activate() {
    print('setup');
    super.activate(); /* expect: proper-super-calls */
  }

  @override
  void deactivate() {
    print('cleanup');
    super.deactivate();
  }

  @override
  void didChangeDependencies() {
    print('setup');
    super.didChangeDependencies();
  }
}
