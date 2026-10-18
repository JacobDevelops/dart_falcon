import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;

final provider = Object();

class PlainConsumer extends ConsumerWidget { /* expect: avoid-unnecessary-consumer-widgets */
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}

class HookConsumer extends hooks.HookConsumerWidget { /* expect: avoid-unnecessary-consumer-widgets */
  Widget build(BuildContext context, WidgetRef ref) {
    return const SizedBox();
  }
}

class NestedConsumer extends ConsumerWidget { /* expect: avoid-unnecessary-consumer-widgets */
  Widget build(BuildContext context, WidgetRef ref) {
    return Consumer(builder: (context, ref, child) {
      ref.watch(provider);
      return const SizedBox();
    });
  }
}

class LocalShadow extends ConsumerWidget { /* expect: avoid-unnecessary-consumer-widgets */
  Widget build(BuildContext context, WidgetRef ref) {
    {
      final ref = 'a local string';
      print(ref);
    }
    return const SizedBox();
  }
}

class RenamedReference extends ConsumerWidget { /* expect: avoid-unnecessary-consumer-widgets */
  Widget build(BuildContext context, WidgetRef unused) => const SizedBox();
}

class ClosureShadow extends ConsumerWidget { /* expect: avoid-unnecessary-consumer-widgets */
  Widget build(BuildContext context, WidgetRef ref) {
    void visit(String ref) { print(ref); }
    visit('shadow');
    return const SizedBox();
  }
}

class PrefixedConsumer extends hooks.ConsumerWidget { /* expect: avoid-unnecessary-consumer-widgets */
  Widget build(BuildContext context, hooks.WidgetRef reference) => const SizedBox();
}
