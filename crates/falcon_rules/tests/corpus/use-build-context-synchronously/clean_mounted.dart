import 'package:flutter/widgets.dart';

Future<void> work() async {}

Future<void> beforeGap(BuildContext context) async {
  context.toString();
  await work();
}

Future<void> guardedContext(BuildContext context) async {
  await work();
  if (!context.mounted) return;
  context.toString();
}

Future<void> positiveGuard(BuildContext context) async {
  await work();
  if (context.mounted) {
    context.toString();
  }
}

abstract class MountedState extends State<StatefulWidget> {
  Future<void> guardedStateContext() async {
    await work();
    if (!mounted) return;
    context.toString();
  }
}
