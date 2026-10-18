import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget loop(List<String> labels) => Column(children: [for (final label in labels) Text(label)]);
Widget block(List<String> labels) => Column(children: labels.map((label) { return Text(label); }).toList());
Widget typed(List<String> labels) => Column(children: labels.map((String label) => Text(label)).toList());
Widget tearOff(List<String> labels) => Column(children: labels.map(Text.new).toList());
Widget growable(List<String> labels) => Column(children: labels.map((label) => Text(label)).toList(growable: false));
Widget unknown(dynamic labels) => Column(children: labels.map((label) => Text(label)).toList());
Widget shadow(dynamic Column, List<String> labels) => Column(children: labels.map((label) => Text(label)).toList());
Widget prefixShadow(dynamic m, List<String> labels) => m.Column(children: labels.map((label) => Text(label)).toList());
Widget nullable(List<String>? labels) => Column(children: labels?.map((label) => Text(label)).toList() ?? []);
Widget custom(Custom labels) => Column(children: labels.map((label) => Text(label)).toList());
class Custom {
  Custom map(Widget Function(String) callback) => this;
  List<Widget> toList() => [];
}
Widget spread(List<String> labels) => Column(children: [
  ...labels.map((label) => Text(label)).toList(),
]);
