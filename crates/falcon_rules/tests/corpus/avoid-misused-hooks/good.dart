import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';

int useBusiness() => 2;
void ordinary() { useState(0); } // Outside-build rule owns this.
class Screen extends HookWidget {
  Widget build(BuildContext context) {
    useState<int>(0);
    return const SizedBox();
  }
}
class RiverScreen extends HookConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) {
    useMemoized(() => 1);
    return const SizedBox();
  }
}
class NonHookScreen extends StatelessWidget {
  Widget build(BuildContext context) {
    useBusiness(); // A custom useX name alone is not a hook.
    final callback = () { useState(0); }; // Outside-build owns callbacks.
    return HookBuilder(builder: (context) {
      useState(1);
      return const SizedBox();
    });
  }
}
class Service {
  int build() => useState(0); // Not a Flutter widget.
}
class ShadowedScreen extends StatelessWidget {
  final int Function(int) useState;
  ShadowedScreen(this.useState);
  Widget build(BuildContext context) {
    useState(0);
    return const SizedBox();
  }
}
class StatefulScreen extends StatefulWidget {}
class ScreenState extends State<StatefulScreen> {
  void event() { useState(0); } // Not a build method.
}

class StatefulHooks extends StatefulHookWidget {}
class HooksState extends State<StatefulHooks> {
  Widget build(BuildContext context) {
    useState(0);
    return const SizedBox();
  }
}
class StatefulRiverHooks extends StatefulHookConsumerWidget {}
class RiverHooksState extends ConsumerState<StatefulRiverHooks> {
  Widget build(BuildContext context) {
    useState(0);
    return HookConsumer(builder: (context, ref, child) {
      useState(1);
      return const SizedBox();
    });
  }
}
class UnknownScreen extends MissingHookBase {
  Widget build(BuildContext context) {
    useState(0);
    return const SizedBox();
  }
}
class UnknownState extends State<dynamic> {
  Widget build(BuildContext context) {
    useState(0);
    return const SizedBox();
  }
}
