import 'package:flutter/widgets.dart';
abstract class HookWidget extends StatelessWidget {
  const HookWidget({super.key});
}
abstract class StatefulHookWidget extends StatefulWidget {
  const StatefulHookWidget({super.key});
}
class HookBuilder extends HookWidget {
  const HookBuilder({super.key, required this.builder});
  final Widget Function(BuildContext) builder;
  Widget build(BuildContext context) => builder(context);
}
T useState<T>(T value) => value;
T useMemoized<T>(T Function() create) => create();
void useEffect(void Function() effect) {}
