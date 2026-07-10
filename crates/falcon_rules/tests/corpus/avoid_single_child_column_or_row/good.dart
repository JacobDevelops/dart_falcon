import 'package:flutter/material.dart';

Widget a() => Column(children: [Text('a'), Text('b')]);

Widget b() => Row(children: []);

Widget c() => Column(children: [...items]);

Widget d() => SizedBox(child: Text('a'));

Widget e() =>
    Column(mainAxisSize: MainAxisSize.min, children: [Text('a'), Icon(Icons.add)]);

Widget f() => Wrap(children: [Text('a')]);

const items = <Widget>[];
