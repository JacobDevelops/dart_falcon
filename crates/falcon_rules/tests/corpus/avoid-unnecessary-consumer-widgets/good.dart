import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;

final provider = Object();

class WatchesProvider extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) {
    ref.watch(provider);
    return const SizedBox();
  }
}

class CapturesReference extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) {
    void callback() { ref.read(provider); }
    callback();
    return const SizedBox();
  }
}

class PassesReference extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) => render(ref);
  Widget render(WidgetRef reference) {
    reference.watch(provider);
    return const SizedBox();
  }
}

class HelperConsumesReference extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
  Widget render(WidgetRef reference) {
    reference.watch(provider);
    return const SizedBox();
  }
}

class HookReadsProvider extends hooks.HookConsumerWidget {
  Widget build(BuildContext context, WidgetRef reference) {
    reference.read(provider);
    return const SizedBox();
  }
}

mixin Behavior {}
class MixedConsumer extends ConsumerWidget with Behavior {
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}

abstract class AbstractConsumer extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}

class InferredReference extends ConsumerWidget {
  Widget build(BuildContext context, ref) => const SizedBox();
}

class UncertainHelper extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
  void helper(ref) { print(ref); }
}
