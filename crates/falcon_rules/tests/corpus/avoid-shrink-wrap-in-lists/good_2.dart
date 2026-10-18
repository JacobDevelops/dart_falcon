import 'package:flutter/material.dart' as m;

class ListView extends m.StatelessWidget {
  const ListView({required this.shrinkWrap, super.key});
  final bool shrinkWrap;
  m.Widget build(m.BuildContext context) => const m.SizedBox();
}
m.Widget custom() => m.Column(children: [const ListView(shrinkWrap: true)]);
m.Widget constantFalse() => m.Column(children: [m.ListView(shrinkWrap: disabled)]);
const disabled = false;
m.Widget shadowedParent(dynamic Column) => Column(children: [m.ListView(shrinkWrap: true)]);
