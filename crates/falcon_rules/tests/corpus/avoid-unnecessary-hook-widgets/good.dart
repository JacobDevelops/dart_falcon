import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;
import 'package:hooks_riverpod/hooks_riverpod.dart';

class StateUser extends HookWidget {
  @override
  Widget build(BuildContext context) {
    useState(0);
    return const SizedBox();
  }
}
class ConsumerUser extends HookConsumerWidget {
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    hooks.useEffect(() {});
    return const SizedBox();
  }
}
Widget useContent() { useState(0); return const SizedBox(); }
Widget _useContent() => useContent();
class CustomHookUser extends HookWidget {
  @override
  Widget build(BuildContext context) => useContent();
}
class PrivateHookUser extends HookWidget {
  @override
  Widget build(BuildContext context) => _useContent();
}
Widget indirectlyUsesHooks() { useState(0); return const SizedBox(); }
class IndirectUser extends HookWidget {
  @override
  Widget build(BuildContext context) => indirectlyUsesHooks();
}
abstract class AbstractWidget extends HookWidget {
  @override
  Widget build(BuildContext context) => const SizedBox();
}
class InheritedBuild extends StateUser {}
final withHook = HookBuilder(builder: (context) {
  useState(0);
  return const SizedBox();
});
final withCustomHook = hooks.HookBuilder(builder: (context) => useContent());
final withIndirectHook = HookBuilder(builder: indirectlyUsesHooksWithContext);
Widget indirectlyUsesHooksWithContext(BuildContext context) => indirectlyUsesHooks();

class MethodHookUser extends HookWidget {
  Widget content() { useState(0); return const SizedBox(); }
  @override
  Widget build(BuildContext context) => content();
}

final prefixedHook = hooks.HookBuilder(builder: (context) {
  hooks.useState(0);
  return const SizedBox();
});
final prefixedNewHook = new hooks.HookBuilder(builder: (context) {
  hooks.useEffect(() {});
  return const SizedBox();
});

class GeneratedHooks extends HookWidget {
  @override
  Widget build(BuildContext context) {
    final states = List.generate(3, (_) => useState(0));
    return const SizedBox();
  }
}

class HookBundle {
  final state = useState(0);
  HookBundle();
  factory HookBundle.create() {
    useState(0);
    return HookBundle();
  }
}
class FactoryHookUser extends HookWidget {
  @override
  Widget build(BuildContext context) {
    new HookBundle.create();
    return const SizedBox();
  }
}
class FieldInitializerHookUser extends HookWidget {
  @override
  Widget build(BuildContext context) {
    HookBundle();
    return const SizedBox();
  }
}

Widget get hookContent { useState(0); return const SizedBox(); }
class TopLevelGetterUser extends HookWidget {
  @override
  Widget build(BuildContext context) => hookContent;
}
class MemberGetterUser extends HookWidget {
  Widget get content { useState(0); return const SizedBox(); }
  @override
  Widget build(BuildContext context) => content;
}
class ExplicitGetterUser extends HookWidget {
  Widget get content { useState(0); return const SizedBox(); }
  @override
  Widget build(BuildContext context) => this.content;
}

class HookActions extends StatelessWidget {
  void init() { useState(0); }
  @override
  Widget build(BuildContext context) => const SizedBox();
}
class CascadeHookUser extends HookWidget {
  @override
  Widget build(BuildContext context) => HookActions()..init();
}

class UnknownCallUser extends HookWidget {
  @override
  Widget build(BuildContext context) {
    Unknown();
    return const SizedBox();
  }
}
class ExplicitUnknownCallUser extends HookWidget {
  @override
  Widget build(BuildContext context) {
    new Unknown();
    return const SizedBox();
  }
}

class HookBase extends HookWidget {
  @override
  Widget build(BuildContext context) => const SizedBox();
}
class HookChild extends HookBase {
  @override
  Widget build(BuildContext context) {
    useState(0);
    return super.build(context);
  }
}

class StaticHooks {
  static Widget content() { useState(0); return const SizedBox(); }
}
class StaticHookUser extends HookWidget {
  @override
  Widget build(BuildContext context) => StaticHooks.content();
}
extension ContextHooks on BuildContext {
  Widget hookContent() { useState(0); return const SizedBox(); }
}
class ExtensionHookUser extends HookWidget {
  @override
  Widget build(BuildContext context) => context.hookContent();
}

class Panel extends StatelessWidget {
  const Panel();
  static Widget withCounter() {
    useState(0);
    return const Panel();
  }
  factory Panel.counted() {
    useState(0);
    return const Panel();
  }
  @override
  Widget build(BuildContext context) => const SizedBox();
}
class StaticWidgetMethodUser extends HookWidget {
  @override
  Widget build(BuildContext context) => Panel.withCounter();
}
class WidgetFactoryUser extends HookWidget {
  @override
  Widget build(BuildContext context) => Panel.counted();
}
class ExplicitWidgetFactoryUser extends HookWidget {
  @override
  Widget build(BuildContext context) => new Panel.counted();
}

class ShadowedWidgetConstructor extends HookWidget {
  Widget SizedBox() {
    useState(0);
    return const Panel();
  }
  @override
  Widget build(BuildContext context) => SizedBox();
}
