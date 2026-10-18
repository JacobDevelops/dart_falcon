import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;

abstract class HookWidget extends StatelessWidget {
  const HookWidget();
}
abstract class HookConsumerWidget extends StatelessWidget {
  const HookConsumerWidget();
}
class HookBuilder {
  HookBuilder({required Widget Function(BuildContext) builder});
}
class LocalWidget extends HookWidget {
  @override
  Widget build(BuildContext context) => const SizedBox();
}
class LocalConsumer extends HookConsumerWidget {
  @override
  Widget build(BuildContext context) => const SizedBox();
}
final localBuilder = HookBuilder(builder: (context) => const SizedBox());
