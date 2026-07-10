import 'package:flutter/material.dart';

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

void doSomething(Object? x) {}
