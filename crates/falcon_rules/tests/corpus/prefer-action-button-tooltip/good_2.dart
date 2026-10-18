import 'package:flutter/material.dart' as m;

class FloatingActionButton extends m.StatelessWidget {
  const FloatingActionButton({super.key});
  m.Widget build(m.BuildContext context) => const m.SizedBox();
}
class IconButton extends m.StatelessWidget {
  const IconButton({super.key});
  m.Widget build(m.BuildContext context) => const m.SizedBox();
}
m.Widget custom() => const FloatingActionButton();
m.Widget customNested() => m.Tooltip(message: 'Action', child: const IconButton());
m.Widget actual() => const m.FloatingActionButton(tooltip: 'Add', onPressed: null);
