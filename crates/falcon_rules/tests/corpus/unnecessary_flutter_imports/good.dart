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
