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
      ],
    );
  }
}

void doSomething() {}
