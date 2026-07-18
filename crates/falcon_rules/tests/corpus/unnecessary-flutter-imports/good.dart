import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'dart:async';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp();

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'My App',
      home: Scaffold(
        appBar: AppBar(title: const Text('Home')),
        body: const Center(
          child: Text('Hello'),
        ),
      ),
    );
  }
}

Future<void> asyncExample() async {
  await Future.delayed(const Duration(seconds: 1));
  debugPrint('Done');
}

// Uses Stream from dart:async
Stream<int> createStream() {
  return Stream.fromIterable([1, 2, 3]);
}

// Uses widgets
Widget createButton() {
  return ElevatedButton(
    onPressed: () {},
    child: const Text('Click me'),
  );
}

// Uses Color from Flutter
void styleContainer() {
  final container = Container(
    color: Color.fromARGB(255, 100, 150, 200),
  );
}
