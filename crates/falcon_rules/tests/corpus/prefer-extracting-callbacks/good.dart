import 'package:flutter/material.dart';

// Non-widget class: callbacks are never flagged (dcl only visits Widget/State).
class NotAWidget {
  void run(List<int> items) {
    items.forEach((item) {
      final doubled = item * 2;
      print(doubled);
    });
  }
}

class MyWidget extends StatelessWidget {
  const MyWidget({super.key});

  final int mode = 0;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // Arrow callback: not a block body, never flagged.
        ElevatedButton(
          onPressed: () => doSomething(),
          child: const Text('go'),
        ),
        // Single-line block callback: within allowed_line_count, not flagged.
        ElevatedButton(
          onPressed: () { doSomething(); },
          child: const Text('short'),
        ),
        // Builder callback (first parameter is BuildContext) is excluded.
        Builder(
          builder: (BuildContext context) {
            final theme = Theme.of(context);
            return Text('$theme');
          },
        ),
        // Untyped builder parameter: unresolved, still treated as a builder.
        Builder(
          builder: (context) {
            final label = 'value';
            return Text(label);
          },
        ),
        // Widget returned only from nested branches still counts as a builder.
        Builder(
          builder: (BuildContext context) {
            switch (mode) {
              case 0:
                return const Text('a');
              default:
                return const Text('b');
            }
          },
        ),
      ],
    );
  }
}

void doSomething() {}
