// Test cases for avoid-returning-widgets rule
// Flags methods that return Widget type outside of build methods

import 'package:flutter/material.dart';

class MyScreen extends StatelessWidget {
  Widget _buildCard() { /* expect: avoid-returning-widgets */
    return Card(
      child: Text('Hello'),
    );
  }

  Widget _buildButton() { /* expect: avoid-returning-widgets */
    return ElevatedButton(
      onPressed: () {},
      child: Text('Click me'),
    );
  }

  Widget buildHeader(String title) { /* expect: avoid-returning-widgets */
    return Container(
      child: Text(title),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: _buildCard(),
    );
  }
}

class WidgetHelper {
  static Widget createCard() { /* expect: avoid-returning-widgets */
    return Card(child: SizedBox());
  }

  static Widget createRow(List<Widget> children) { /* expect: avoid-returning-widgets */
    return Row(children: children);
  }
}

Widget globalWidgetBuilder() { /* expect: avoid-returning-widgets */
  return Container();
}

Future<Widget> asyncWidgetBuilder() { /* expect: avoid-returning-widgets */
  return Future.value(Text('async'));
}
