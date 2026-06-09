// Good examples for avoid-returning-widgets rule
// Using separate widget classes instead of methods that return widgets

import 'package:flutter/material.dart';

class MyScreen extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: MyCard(),
      appBar: AppBar(
        title: MyHeader(title: 'Title'),
      ),
    );
  }
}

class MyCard extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Card(
      child: Text('Hello'),
    );
  }
}

class MyButton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: () {},
      child: Text('Click me'),
    );
  }
}

class MyHeader extends StatelessWidget {
  final String title;

  MyHeader({required this.title});

  @override
  Widget build(BuildContext context) {
    return Container(
      child: Text(title),
    );
  }
}

class WidgetHelper {
  static Card createCard() {
    return Card(child: SizedBox());
  }

  static List<Widget> getChildren() {
    return [Text('a'), Text('b')];
  }
}

void processData(String data) {
  print(data);
}

String buildString() {
  return "not a widget";
}
