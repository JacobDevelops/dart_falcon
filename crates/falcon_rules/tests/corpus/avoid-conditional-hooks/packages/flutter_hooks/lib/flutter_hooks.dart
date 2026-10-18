import 'package:flutter/widgets.dart';

abstract class HookWidget extends StatelessWidget {
  const HookWidget({super.key});
}

class HookBuilder extends HookWidget {
  const HookBuilder({super.key, required this.builder});
  final Widget Function(BuildContext) builder;
  Widget build(BuildContext context) => builder(context);
}

T useState<T>(T initial) => initial;
T useMemoized<T>(T Function() callback) => callback();
void useEffect(void Function() callback) {}
