import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart' as real;

abstract class ConsumerWidget {}
abstract class HookConsumerWidget {}
abstract class WidgetRef implements real.WidgetRef {}

class LocalConsumer extends ConsumerWidget {
  Widget build(BuildContext context, real.WidgetRef ref) => const SizedBox();
}

class LocalHookConsumer extends HookConsumerWidget {
  Widget build(BuildContext context, real.WidgetRef ref) => const SizedBox();
}

// A real superclass is not sufficient when the reference type is a lookalike.
class LocalReference extends real.ConsumerWidget {
  Widget build(BuildContext context, covariant WidgetRef ref) => const SizedBox();
}

class SubclassedConsumer extends real.ConsumerWidget {
  Widget build(BuildContext context, real.WidgetRef ref) => const SizedBox();
}
class UsingSubclass extends SubclassedConsumer {
  Widget build(BuildContext context, real.WidgetRef ref) {
    consume(ref);
    return const SizedBox();
  }
}
void consume(real.WidgetRef ref) {}
