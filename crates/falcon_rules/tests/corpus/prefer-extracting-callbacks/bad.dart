import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

class MyWidget extends StatelessWidget {
  const MyWidget({super.key});

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: () { /* expect: prefer-extracting-callbacks */
        final now = DateTime.now();
        doSomething(now);
      },
      child: const Text('go'),
    );
  }
}

class MyState extends State<MyWidget> {
  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: () { /* expect: prefer-extracting-callbacks */
        setState(() {});
        doSomething(null);
      },
      child: const Text('tap'),
    );
  }
}

class PrefixedWidget extends f.StatelessWidget {
  @override
  f.Widget build(f.BuildContext context) => f.ElevatedButton(
    onPressed: () { /* expect: prefer-extracting-callbacks */
      final value = DateTime.now();
      doSomething(value);
    },
  );
}

void doSomething(Object? x) {}
