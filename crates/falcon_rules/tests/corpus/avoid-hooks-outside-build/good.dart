import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;
import 'package:hooks_riverpod/hooks_riverpod.dart';

int useFeature() => useState(1);
int _usePrivate() => hooks.useMemoized(() => 2);
int useBusiness() => 3;
void ordinary() { useBusiness(); }
void shadowed(int Function(int) useState) { useState(0); }
void shadowedPrefix(dynamic hooks) { hooks.useState(0); }
class Screen extends HookWidget {
  Widget build(BuildContext context) {
    useState<int>(0);
    useFeature();
    _usePrivate();
    int _useLocal() => hooks.useState<int>(1);
    _useLocal();
    return HookBuilder(builder: (context) {
      hooks.useEffect(() {});
      return const SizedBox();
    });
  }
  int _useMethod() => useState(2);
}
class RiverScreen extends HookConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) {
    useState(0);
    return const SizedBox();
  }
}
class NonHookScreen extends StatelessWidget {
  Widget build(BuildContext context) {
    useState(0); // Owned by avoid-misused-hooks, not this rule.
    return const SizedBox();
  }
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
    useState(0); // Unresolved ancestry is not proof of an invalid context.
    return const SizedBox();
  }
}
mixin CounterHooks {
  int useCounter() => useState(0);
}
extension ContextHooks on BuildContext {
  int useCounter() => useState(0);
}

mixin HookScreenBuild on HookWidget {
  Widget build(BuildContext context) {
    useState(0);
    return const SizedBox();
  }
}
mixin UnconstrainedBuild {
  Widget build(BuildContext context) {
    useState(0);
    return const SizedBox();
  }
}
