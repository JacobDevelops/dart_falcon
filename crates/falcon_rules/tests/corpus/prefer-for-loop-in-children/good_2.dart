import 'package:flutter/material.dart' as m;

class Column extends m.StatelessWidget {
  const Column({required this.children, super.key});
  final List<m.Widget> children;
  m.Widget build(m.BuildContext context) => const m.SizedBox();
}
m.Widget custom(List<String> labels) => Column(children: labels.map((label) => m.Text(label)).toList());
