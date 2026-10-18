import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget column() => Column(children: [
  ListView(shrinkWrap: true), /* expect: avoid-shrink-wrap-in-lists */
]);
Widget row() => Row(children: [
  new ListView(shrinkWrap: true), /* expect: avoid-shrink-wrap-in-lists */
]);
Widget prefixed() => m.Column(children: [
  m.ListView(shrinkWrap: (true)), /* expect: avoid-shrink-wrap-in-lists */
]);
Widget nested() => ListView(children: [
  ListView.builder(shrinkWrap: true, itemBuilder: (context, index) => Text('item')), /* expect: avoid-shrink-wrap-in-lists */
]);
Widget wrapped() => Column(children: [
  Expanded(child: ListView(shrinkWrap: true)), /* expect: avoid-shrink-wrap-in-lists */
]);
