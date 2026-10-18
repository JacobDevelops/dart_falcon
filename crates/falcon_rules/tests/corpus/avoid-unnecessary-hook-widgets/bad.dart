import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;
import 'package:hooks_riverpod/hooks_riverpod.dart';

class EmptyBlock extends HookWidget { /* expect: avoid-unnecessary-hook-widgets */
  @override
  Widget build(BuildContext context) { return const SizedBox(); }
}

class EmptyArrow extends hooks.HookWidget { /* expect: avoid-unnecessary-hook-widgets */
  @override
  Widget build(BuildContext context) => const SizedBox();
}

class EmptyConsumer extends HookConsumerWidget { /* expect: avoid-unnecessary-hook-widgets */
  @override
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}

final emptyArrow = HookBuilder(builder: (context) => const SizedBox()); /* expect: avoid-unnecessary-hook-widgets */
final emptyBlock = hooks.HookBuilder(builder: (context) { return const SizedBox(); }); /* expect: avoid-unnecessary-hook-widgets */
final explicitNew = new HookBuilder(builder: (context) => const SizedBox()); /* expect: avoid-unnecessary-hook-widgets */

class OuterWithoutHooks extends HookWidget { /* expect: avoid-unnecessary-hook-widgets */
  @override
  Widget build(BuildContext context) => HookBuilder(builder: (context) {
    useState(0);
    return const SizedBox();
  });
}

class CallbackOnly extends HookWidget { /* expect: avoid-unnecessary-hook-widgets */
  @override
  Widget build(BuildContext context) => Clickable(onPressed: () { useState(0); });
}

final nestedCallback = hooks.HookBuilder(builder: (context) { /* expect: avoid-unnecessary-hook-widgets */
  return Clickable(onPressed: () { hooks.useState(0); });
});

final prefixedNew = new hooks.HookBuilder(builder: (context) => const SizedBox()); /* expect: avoid-unnecessary-hook-widgets */
final parenthesized = (hooks.HookBuilder(builder: (context) => const SizedBox())); /* expect: avoid-unnecessary-hook-widgets */

class Clickable extends StatelessWidget {
  const Clickable({required this.onPressed});
  final void Function() onPressed;
  @override
  Widget build(BuildContext context) => const SizedBox();
}

class PlainFieldRead extends HookWidget { /* expect: avoid-unnecessary-hook-widgets */
  const PlainFieldRead(this.child);
  final Widget child;
  @override
  Widget build(BuildContext context) => child;
}
