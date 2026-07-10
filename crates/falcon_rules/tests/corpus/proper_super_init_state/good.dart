import 'package:flutter/material.dart';

class A extends State<StatefulWidget> {
  @override
  void initState() {
    super.initState();
    doStuff();
  }
}

class B extends State<StatefulWidget> {
  @override
  void initState() {
    super.initState();
  }
}

class C extends ConsumerState<StatefulWidget> {
  @override
  void initState() {
    super.initState();
    final a = 1;
    print(a);
  }
}

class D {
  void initState() {
    doStuff();
  }
}

class E extends StatelessWidget {
  void initState() {}
}
