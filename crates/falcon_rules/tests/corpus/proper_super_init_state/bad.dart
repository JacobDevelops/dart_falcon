import 'package:flutter/material.dart';

class A extends State<StatefulWidget> {
  @override
  void initState() {
    doStuff();
    super.initState(); /* expect: proper_super_init_state */
  }
}

class B extends State<StatefulWidget> {
  int x = 0;

  @override
  void initState() { /* expect: proper_super_init_state */
    x = 1;
  }
}

class C extends State<StatefulWidget> {
  @override
  void initState() {} /* expect: proper_super_init_state */
}

class D extends ConsumerState<StatefulWidget> {
  @override
  void initState() {
    final a = 1;
    super.initState(); /* expect: proper_super_init_state */
    print(a);
  }
}

class E extends State<StatefulWidget> {
  @override
  void initState() {
    setup();
    super.initState(); /* expect: proper_super_init_state */
  }
}
